//! Test support only: a JSON tree built from rjsonparser's events, so tests
//! can inspect the nested documents the production code streams out. The
//! running app never builds one (see `args.rs`).

use tagliacarte_core::json::{ContentHandler, Number, ParseResult, parse_str_complete};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// An open container and the key it was opened under in its parent, which
/// its own members' keys would otherwise overwrite.
struct Frame {
    parent_key: Option<String>,
    body: Body,
}

enum Body {
    Object(Vec<(String, Value)>),
    Array(Vec<Value>),
}

#[derive(Default)]
struct Builder {
    stack: Vec<Frame>,
    key: Option<String>,
    root: Option<Value>,
}

impl Builder {
    fn put(&mut self, v: Value) {
        match self.stack.last_mut() {
            Some(Frame { body: Body::Object(members), .. }) => {
                members.push((self.key.take().unwrap_or_default(), v))
            }
            Some(Frame { body: Body::Array(items), .. }) => items.push(v),
            None => self.root = Some(v),
        }
    }
}

impl ContentHandler for Builder {
    fn start_object(&mut self) -> ParseResult<()> {
        let parent_key = self.key.take();
        self.stack.push(Frame { parent_key, body: Body::Object(Vec::new()) });
        Ok(())
    }

    fn end_object(&mut self) -> ParseResult<()> {
        if let Some(Frame { parent_key, body: Body::Object(members) }) = self.stack.pop() {
            self.key = parent_key;
            self.put(Value::Object(members));
        }
        Ok(())
    }

    fn start_array(&mut self) -> ParseResult<()> {
        let parent_key = self.key.take();
        self.stack.push(Frame { parent_key, body: Body::Array(Vec::new()) });
        Ok(())
    }

    fn end_array(&mut self) -> ParseResult<()> {
        if let Some(Frame { parent_key, body: Body::Array(items) }) = self.stack.pop() {
            self.key = parent_key;
            self.put(Value::Array(items));
        }
        Ok(())
    }

    fn key(&mut self, key: &str) -> ParseResult<()> {
        self.key = Some(key.to_owned());
        Ok(())
    }

    fn string_value(&mut self, value: &str) -> ParseResult<()> {
        self.put(Value::Str(value.to_owned()));
        Ok(())
    }

    fn number_value(&mut self, value: &Number) -> ParseResult<()> {
        let v = match value.as_i64() {
            Some(i) => Value::Int(i),
            None => Value::Float(value.as_f64().unwrap_or(f64::NAN)),
        };
        self.put(v);
        Ok(())
    }

    fn boolean_value(&mut self, value: bool) -> ParseResult<()> {
        self.put(Value::Bool(value));
        Ok(())
    }

    fn null_value(&mut self) -> ParseResult<()> {
        self.put(Value::Null);
        Ok(())
    }
}

impl Value {
    /// Parses a complete JSON document. An empty body is an empty object, so
    /// commands without arguments need no special case.
    pub fn parse(body: &str) -> Result<Value, String> {
        if body.trim().is_empty() {
            return Ok(Value::Object(Vec::new()));
        }
        let mut b = Builder::default();
        parse_str_complete(body, &mut b).map_err(|e| format!("invalid JSON: {e}"))?;
        b.root.ok_or_else(|| "invalid JSON: empty document".to_owned())
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// A string member; `null` and absent are both `None`.
    pub fn opt_str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Value::Str(s)) => Some(s),
            _ => None,
        }
    }

    pub fn str(&self, key: &str) -> Result<String, String> {
        self.opt_str(key)
            .map(str::to_owned)
            .ok_or_else(|| format!("missing string argument '{key}'"))
    }

    pub fn opt_u64(&self, key: &str) -> Option<u64> {
        match self.get(key) {
            Some(Value::Int(i)) => u64::try_from(*i).ok(),
            _ => None,
        }
    }

    pub fn u64(&self, key: &str) -> Result<u64, String> {
        self.opt_u64(key)
            .ok_or_else(|| format!("missing non-negative integer argument '{key}'"))
    }

    pub fn i64(&self, key: &str) -> Result<i64, String> {
        match self.get(key) {
            Some(Value::Int(i)) => Ok(*i),
            _ => Err(format!("missing integer argument '{key}'")),
        }
    }

    pub fn bool_or(&self, key: &str, default: bool) -> bool {
        match self.get(key) {
            Some(Value::Bool(b)) => *b,
            _ => default,
        }
    }

    pub fn strings(&self, key: &str) -> Result<Vec<String>, String> {
        match self.get(key) {
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| match v {
                    Value::Str(s) => Ok(s.clone()),
                    _ => Err(format!("'{key}' must be an array of strings")),
                })
                .collect(),
            _ => Err(format!("missing array argument '{key}'")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_documents() {
        let v = Value::parse(r#"{"a":"x","n":42,"f":1.5,"t":true,"z":null,"l":["p","q"],"o":{"k":"v"}}"#)
            .unwrap();
        assert_eq!(v.str("a").unwrap(), "x");
        assert_eq!(v.u64("n").unwrap(), 42);
        assert_eq!(v.get("f"), Some(&Value::Float(1.5)));
        assert!(v.bool_or("t", false));
        assert_eq!(v.opt_str("z"), None);
        assert_eq!(v.strings("l").unwrap(), vec!["p", "q"]);
        assert_eq!(v.get("o").unwrap().str("k").unwrap(), "v");
    }

    #[test]
    fn empty_body_is_an_empty_object_and_errors_are_reported() {
        assert_eq!(Value::parse("").unwrap(), Value::Object(Vec::new()));
        assert!(Value::parse("{\"a\":").is_err());
        let v = Value::parse("{}").unwrap();
        assert!(v.str("missing").unwrap_err().contains("missing"));
        assert!(Value::parse(r#"{"n":-1}"#).unwrap().u64("n").is_err());
    }
}
