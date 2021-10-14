use std::collections::HashMap;

#[derive(Debug)]
pub struct Route {
    path: &'static str,
}

#[derive(Debug)]
pub struct Invocation {
    target: &'static str,
    arguments: Argument,
}

#[derive(Debug)]
pub enum Argument {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<Argument>),
    Object(HashMap<String, Argument>),
}

impl Invocation {
    pub fn target(&self) -> &'static str {
        self.target
    }

    pub fn arguments(&self) -> &Argument {
        &self.arguments
    }
}

/*
/chat <-- Route
/chat
{
    target: "Dupa",
    arguments : [1, "dwa", true]
} <-- json or messagepack

ws -> bytes -> des {json, msgpack} -> extract target -> extract arguments -> invoke

FnMut(&mut Hub, arg1, arg2) -> ??

*/
