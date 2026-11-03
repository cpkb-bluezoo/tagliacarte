//! Command arguments, collected as rjsonparser's events arrive (through
//! `tagliacarte_core::json`). Arguments are one flat object of scalars and
//! arrays of strings; nesting is refused rather than built into a tree.
//! Anything large travels as a raw HTTP body, not as JSON arguments.

use tagliacarte_core::json::{ContentHandler, Number, ParseResult, parse_str_complete};

#[derive(Debug, Clone, PartialEq)]
enum Arg {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Strings(Vec<String>),
}

#[derive(Debug, Default)]
pub struct Args {
    members: Vec<(String, Arg)>,
}

#[derive(Default)]
struct Collector {
    args: Args,
    key: Option<String>,
    /// 0 outside the document, 1 in the argument object, 2 in an array.
    depth: u8,
    array: Vec<String>,
    error: Option<String>,
}

impl Collector {
    fn fail(&mut self, what: &str) {
        self.error.get_or_insert_with(|| what.to_owned());
    }

    fn put(&mut self, v: Arg) {
        if self.depth != 1 {
            return self.fail("arguments must be one flat object");
        }
        let key = self.key.take().unwrap_or_default();
        self.args.members.push((key, v));
    }
}

impl ContentHandler for Collector {
    fn start_object(&mut self) -> ParseResult<()> {
        if self.depth == 0 {
            self.depth = 1;
        } else {
            self.fail("nested objects are not supported in arguments");
        }
        Ok(())
    }

    fn end_object(&mut self) -> ParseResult<()> {
        if self.depth == 1 {
            self.depth = 0;
        }
        Ok(())
    }

    fn start_array(&mut self) -> ParseResult<()> {
        if self.depth == 1 {
            self.depth = 2;
            self.array.clear();
        } else {
            self.fail("arrays are only supported as members of the argument object");
        }
        Ok(())
    }

    fn end_array(&mut self) -> ParseResult<()> {
        if self.depth == 2 {
            self.depth = 1;
            let items = std::mem::take(&mut self.array);
            self.put(Arg::Strings(items));
        }
        Ok(())
    }

    fn key(&mut self, key: &str) -> ParseResult<()> {
        if self.depth == 1 {
            self.key = Some(key.to_owned());
        }
        Ok(())
    }

    fn string_value(&mut self, value: &str) -> ParseResult<()> {
        match self.depth {
            2 => self.array.push(value.to_owned()),
            _ => self.put(Arg::Str(value.to_owned())),
        }
        Ok(())
    }

    fn number_value(&mut self, value: &Number) -> ParseResult<()> {
        match value.as_i64() {
            Some(i) if self.depth == 1 => self.put(Arg::Int(i)),
            _ => self.fail("arguments take integers only"),
        }
        Ok(())
    }

    fn boolean_value(&mut self, value: bool) -> ParseResult<()> {
        self.put(Arg::Bool(value));
        Ok(())
    }

    fn null_value(&mut self) -> ParseResult<()> {
        self.put(Arg::Null);
        Ok(())
    }
}

impl Args {
    /// An empty body is no arguments, so commands without any need no special case.
    pub fn parse(body: &str) -> Result<Args, String> {
        if body.trim().is_empty() {
            return Ok(Args::default());
        }
        let mut c = Collector::default();
        parse_str_complete(body, &mut c).map_err(|e| format!("invalid JSON: {e}"))?;
        match c.error {
            Some(e) => Err(e),
            None => Ok(c.args),
        }
    }

    fn get(&self, key: &str) -> Option<&Arg> {
        self.members.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// A string member; `null` and absent are both `None`.
    pub fn opt_str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Arg::Str(s)) => Some(s),
            _ => None,
        }
    }

    pub fn str(&self, key: &str) -> Result<String, String> {
        self.opt_str(key)
            .map(str::to_owned)
            .ok_or_else(|| format!("missing string argument '{key}'"))
    }

    pub fn opt_i64(&self, key: &str) -> Option<i64> {
        match self.get(key) {
            Some(Arg::Int(i)) => Some(*i),
            _ => None,
        }
    }

    pub fn i64(&self, key: &str) -> Result<i64, String> {
        self.opt_i64(key).ok_or_else(|| format!("missing integer argument '{key}'"))
    }

    pub fn opt_u64(&self, key: &str) -> Option<u64> {
        match self.get(key) {
            Some(Arg::Int(i)) => u64::try_from(*i).ok(),
            _ => None,
        }
    }

    pub fn u64(&self, key: &str) -> Result<u64, String> {
        self.opt_u64(key)
            .ok_or_else(|| format!("missing non-negative integer argument '{key}'"))
    }

    pub fn opt_bool(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(Arg::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    pub fn bool_or(&self, key: &str, default: bool) -> bool {
        match self.get(key) {
            Some(Arg::Bool(b)) => *b,
            _ => default,
        }
    }

    pub fn strings(&self, key: &str) -> Result<Vec<String>, String> {
        match self.get(key) {
            Some(Arg::Strings(items)) => Ok(items.clone()),
            _ => Err(format!("missing array argument '{key}'")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_a_flat_object() {
        let a = Args::parse(r#"{"a":"x","n":42,"t":true,"z":null,"l":["p","q"],"e":[]}"#).unwrap();
        assert_eq!(a.str("a").unwrap(), "x");
        assert_eq!(a.u64("n").unwrap(), 42);
        assert!(a.bool_or("t", false));
        assert_eq!(a.opt_str("z"), None);
        assert_eq!(a.strings("l").unwrap(), vec!["p", "q"]);
        assert!(a.strings("e").unwrap().is_empty());
    }

    #[test]
    fn integers_may_be_negative_where_a_date_can_be_before_1970() {
        let a = Args::parse(r#"{"t":-86400000,"n":5}"#).unwrap();
        assert_eq!(a.i64("t").unwrap(), -86_400_000);
        assert_eq!(a.i64("n").unwrap(), 5);
        assert!(a.i64("missing").unwrap_err().contains("'missing'"));
        assert_eq!(a.opt_i64("missing"), None);
        assert!(Args::parse(r#"{"t":"x"}"#).unwrap().i64("t").is_err());
    }

    #[test]
    fn empty_body_means_no_arguments() {
        let a = Args::parse("  ").unwrap();
        assert!(a.str("x").unwrap_err().contains("'x'"));
    }

    #[test]
    fn nesting_floats_and_bad_json_are_refused() {
        assert!(Args::parse(r#"{"o":{"k":1}}"#).unwrap_err().contains("nested"));
        assert!(Args::parse(r#"{"l":[["a"]]}"#).is_err());
        assert!(Args::parse(r#"[1,2]"#).is_err());
        assert!(Args::parse(r#"{"f":1.5}"#).unwrap_err().contains("integers"));
        assert!(Args::parse("{\"a\":").is_err());
        assert!(Args::parse(r#"{"n":-1}"#).unwrap().u64("n").is_err());
    }
}
