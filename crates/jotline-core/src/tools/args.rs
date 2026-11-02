//! Lenient, typed access to tool-call arguments.
//!
//! Small models often quote numbers (`"10"`) and booleans (`"true"`); these accessors accept
//! both forms but reject values of the wrong kind with a message naming the argument.

use super::{fail, Failure};
use serde_json::{Map, Value};

pub(crate) struct Args<'a> {
    map: &'a Map<String, Value>,
}

impl<'a> Args<'a> {
    pub(crate) fn new(value: &'a Value) -> Result<Self, Failure> {
        match value.as_object() {
            Some(map) => Ok(Self { map }),
            None => fail("args must be a JSON object"),
        }
    }

    pub(crate) fn raw(&self) -> &'a Map<String, Value> {
        self.map
    }

    pub(crate) fn get(&self, key: &str) -> Option<&'a Value> {
        self.map.get(key).filter(|value| !value.is_null())
    }

    pub(crate) fn req_str(&self, key: &str) -> Result<&'a str, Failure> {
        match self.opt_str(key)? {
            Some(value) => Ok(value),
            None => fail(format!("missing required argument '{key}'")),
        }
    }

    pub(crate) fn opt_str(&self, key: &str) -> Result<Option<&'a str>, Failure> {
        match self.get(key) {
            None => Ok(None),
            Some(Value::String(value)) => Ok(Some(value.as_str())),
            Some(_) => fail(format!("argument '{key}' must be a string")),
        }
    }

    pub(crate) fn usize_or(&self, key: &str, default: usize) -> Result<usize, Failure> {
        match self.get(key) {
            None => Ok(default),
            Some(value) => match as_usize(value) {
                Some(number) => Ok(number),
                None => fail(format!("argument '{key}' must be a non-negative integer")),
            },
        }
    }

    pub(crate) fn bool_or(&self, key: &str, default: bool) -> Result<bool, Failure> {
        match self.get(key) {
            None => Ok(default),
            Some(Value::Bool(value)) => Ok(*value),
            Some(Value::String(value)) if value.eq_ignore_ascii_case("true") => Ok(true),
            Some(Value::String(value)) if value.eq_ignore_ascii_case("false") => Ok(false),
            Some(_) => fail(format!("argument '{key}' must be true or false")),
        }
    }
}

/// Reads a non-negative integer from a JSON number or numeric string.
pub(crate) fn as_usize(value: &Value) -> Option<usize> {
    match value {
        Value::Number(number) => number.as_u64().and_then(|n| usize::try_from(n).ok()),
        Value::String(text) => text.trim().parse::<usize>().ok(),
        _ => None,
    }
}
