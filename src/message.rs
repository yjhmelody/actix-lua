use ::actix::dev::{MessageResponse, ResponseChannel};
use ::actix::prelude::*;
use regex::Regex;
use rlua::Result as LuaResult;
use rlua::{Context, FromLua, ToLua, Value};

use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub enum LuaMessage {
    String(String),
    Integer(i64),
    Number(f64),
    Boolean(bool),
    Nil,
    Table(HashMap<String, LuaMessage>),
    ThreadYield(String),
}

impl<A, M> MessageResponse<A, M> for LuaMessage
where
    A: Actor,
    M: Message<Result = LuaMessage>,
{
    fn handle<R: ResponseChannel<M>>(self, _: &mut A::Context, tx: Option<R>) {
        if let Some(tx) = tx {
            tx.send(self);
        }
    }
}

impl Message for LuaMessage {
    type Result = LuaMessage;
}

// macro_rules! lua_message_convert_boolean {
//     ( $($ty:ty),+ ) => {
//         $(
//             impl From<$ty> for LuaMessage {
//                 fn from(s: $ty) -> Self {
//                     LuaMessage::Boolean(s != 0)
//                 }
//             }
//         )+
//     };
// }
//
// lua_message_convert_boolean!(i8, u8, i16, u16, i32, u32, i64);

impl From<bool> for LuaMessage {
    fn from(s: bool) -> Self {
        LuaMessage::Boolean(s)
    }
}


impl<'l> From<&'l str> for LuaMessage {
    fn from(s: &'l str) -> Self {
        LuaMessage::String(s.to_string())
    }
}

impl From<String> for LuaMessage {
    fn from(s: String) -> Self {
        LuaMessage::String(s)
    }
}

macro_rules! lua_message_convert_int {
    ( $($ty:ty),+ ) => {
        $(
            impl From<$ty> for LuaMessage {
                fn from(s: $ty) -> Self {
                    LuaMessage::Integer(i64::from(s))
                }
            }
        )+
    };
}

lua_message_convert_int!(i8, u8, i16, u16, i32, u32, i64);

impl From<usize> for LuaMessage {
    fn from(s: usize) -> Self {
        LuaMessage::Integer(s as i64)
    }
}

impl From<isize> for LuaMessage {
    fn from(s: isize) -> Self {
        LuaMessage::Integer(s as i64)
    }
}

impl From<HashMap<String, LuaMessage>> for LuaMessage {
    fn from(s: HashMap<String, LuaMessage>) -> Self {
        LuaMessage::Table(s)
    }
}

macro_rules! lua_message_convert_float {
    ( $($ty:ty),+ ) => {
        $(
            impl From<$ty> for LuaMessage {
                fn from(s: $ty) -> Self {
                    LuaMessage::Number(f64::from(s))
                }
            }
        )+
    };
}

lua_message_convert_float!(f32, f64);

impl<'lua> FromLua<'lua> for LuaMessage {
    fn from_lua(v: Value<'lua>, ctx: Context<'lua>) -> LuaResult<LuaMessage> {
        match v {
            Value::String(x) => {
                let re = Regex::new(r"__suspended__(.+)").unwrap();
                let s = Value::String(x);
                if let Some(cap) = re.captures(&String::from_lua(s.clone(), ctx)?) {
                    let tid = cap.get(1).unwrap().as_str();
                    Ok(LuaMessage::ThreadYield(tid.to_string()))
                } else {
                    Ok(LuaMessage::String(String::from_lua(s.clone(), ctx)?))
                }
            }
            Value::Integer(n) => Ok(LuaMessage::Integer(n as i64)),
            Value::Number(n) => Ok(LuaMessage::Number(n as f64)),
            Value::Boolean(b) => Ok(LuaMessage::Boolean(b)),
            Value::Nil => Ok(LuaMessage::Nil),
            Value::Table(t) => Ok(LuaMessage::Table(HashMap::from_lua(Value::Table(t), ctx)?)),
            Value::Error(err) => {
                panic!("Lua error: {:?}", err);
            }
            _ => unimplemented!(),
        }
    }
}

impl<'lua> ToLua<'lua> for LuaMessage {
    fn to_lua(self, ctx: Context<'lua>) -> LuaResult<Value<'lua>> {
        match self {
            LuaMessage::String(x) => Ok(Value::String(ctx.create_string(&x)?)),
            LuaMessage::Integer(x) => Ok(Value::Integer(x)),
            LuaMessage::Number(x) => Ok(Value::Number(x)),
            LuaMessage::Boolean(x) => Ok(Value::Boolean(x)),
            LuaMessage::Nil => Ok(Value::Nil),
            LuaMessage::Table(x) => Ok(Value::Table(ctx.create_table_from(x)?)),

            // TODO: passing rust error to lua error?
            _ => unimplemented!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlua::Lua;
    use std::mem::discriminant;

    #[test]
    fn constructors() {
        assert_eq!(LuaMessage::from(42), LuaMessage::Integer(42));
        assert_eq!(LuaMessage::from(0), LuaMessage::Integer(0));
        assert_eq!(
            LuaMessage::from("foo"),
            LuaMessage::String("foo".to_string())
        );
        assert_eq!(LuaMessage::from(42.5), LuaMessage::Number(42.5));
        assert_eq!(LuaMessage::from(true), LuaMessage::Boolean(true));

        let mut t = HashMap::new();
        t.insert("bar".to_string(), LuaMessage::from("abc"));
        let mut t2 = HashMap::new();
        t2.insert("bar".to_string(), LuaMessage::from("abc"));
        assert_eq!(LuaMessage::from(t), LuaMessage::Table(t2));
    }

    #[test]
    fn to_lua<'lua>() {
        // we only check if they have the correct variant
        let lua = Lua::new();
        lua.context(|ctx| {
            assert_eq!(
                LuaMessage::Integer(42).to_lua(ctx),
                Ok(Value::<'lua>::Integer(42)),
            );
            assert_eq!(
                discriminant(&LuaMessage::String("foo".to_string()).to_lua(ctx).unwrap()),
                discriminant(&Value::String(ctx.create_string("foo").unwrap()))
            );
            assert_eq!(
                discriminant(&LuaMessage::Number(42.5).to_lua(ctx).unwrap()),
                discriminant(&Value::Number(42.5))
            );
            assert_eq!(
                discriminant(&LuaMessage::Boolean(true).to_lua(ctx).unwrap()),
                discriminant(&Value::Boolean(true))
            );
            assert_eq!(
                discriminant(&LuaMessage::Nil.to_lua(ctx).unwrap()),
                discriminant(&Value::Nil)
            );

            let mut t = HashMap::new();
            t.insert("bar".to_string(), LuaMessage::from("abc"));
            assert_eq!(
                discriminant(&LuaMessage::Table(t).to_lua(ctx).unwrap()),
                discriminant(&Value::Table(ctx.create_table().unwrap()))
            );
        })
    }

    #[test]
    fn from_lua() {
        // we only check if they have the correct variant
        let lua = Lua::new();
        lua.context(|ctx| {
            assert_eq!(
                discriminant(&LuaMessage::from_lua(Value::Integer(42), ctx).unwrap()),
                discriminant(&LuaMessage::Integer(42))
            );
            assert_eq!(
                discriminant(&LuaMessage::from_lua(Value::Number(42.5), ctx).unwrap()),
                discriminant(&LuaMessage::Number(42.5))
            );
            assert_eq!(
                discriminant(
                    &LuaMessage::from_lua(Value::String(ctx.create_string("foo").unwrap()), ctx)
                        .unwrap()
                ),
                discriminant(&LuaMessage::String("foo".to_string()))
            );
            assert_eq!(
                discriminant(&LuaMessage::from_lua(Value::Boolean(true), ctx).unwrap()),
                discriminant(&LuaMessage::Boolean(true))
            );
            assert_eq!(
                discriminant(&LuaMessage::from_lua(Value::Nil, ctx).unwrap()),
                discriminant(&LuaMessage::Nil)
            );

            let mut t = HashMap::new();
            t.insert("bar".to_string(), LuaMessage::from("abc"));
            assert_eq!(
                discriminant(
                    &LuaMessage::from_lua(Value::Table(ctx.create_table().unwrap()), ctx).unwrap()
                ),
                discriminant(&LuaMessage::Table(t))
            );
        })
    }

    #[should_panic]
    #[test]
    fn from_lua_error() {
        use rlua::Error;

        let lua = Lua::new();
        lua.context(|ctx| {
            &LuaMessage::from_lua(Value::Error(Error::RuntimeError("foo".to_string())), ctx)
                .unwrap();
        })
    }
}
