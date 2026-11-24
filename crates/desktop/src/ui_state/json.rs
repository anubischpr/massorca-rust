//! A JSON reader for the UI state file. The desktop has no serde, so this reads the whole document strictly and
//! refuses anything it does not understand, which gives the defaults rather than a wrong setting.

/// Nesting deeper than this is refused, so a damaged file cannot exhaust the stack.
const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The member `key` of an object; the first one when a key repeats.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(members) => members
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(number) => Some(*number),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
}

/// Parses one JSON document; `None` when the text is not exactly one document.
pub fn parse(text: &str) -> Option<Value> {
    let mut reader = Reader {
        text,
        at: 0,
        depth: 0,
    };
    let value = reader.value()?;
    reader.skip_space();
    (reader.at == text.len()).then_some(value)
}

struct Reader<'a> {
    text: &'a str,
    at: usize,
    depth: usize,
}

impl Reader<'_> {
    fn value(&mut self) -> Option<Value> {
        self.skip_space();
        match self.peek()? {
            b'{' => self.nested(Self::object),
            b'[' => self.nested(Self::array),
            b'"' => self.string().map(Value::String),
            b't' => self.word("true", Value::Bool(true)),
            b'f' => self.word("false", Value::Bool(false)),
            b'n' => self.word("null", Value::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => None,
        }
    }

    fn nested(&mut self, parse: fn(&mut Self) -> Option<Value>) -> Option<Value> {
        if self.depth == MAX_DEPTH {
            return None;
        }
        self.depth += 1;
        let value = parse(self);
        self.depth -= 1;
        value
    }

    fn object(&mut self) -> Option<Value> {
        self.at += 1;
        let mut members = Vec::new();
        self.skip_space();
        if self.eat(b'}') {
            return Some(Value::Object(members));
        }
        loop {
            self.skip_space();
            let name = self.string()?;
            self.skip_space();
            if !self.eat(b':') {
                return None;
            }
            members.push((name, self.value()?));
            self.skip_space();
            if self.eat(b'}') {
                return Some(Value::Object(members));
            }
            if !self.eat(b',') {
                return None;
            }
        }
    }

    fn array(&mut self) -> Option<Value> {
        self.at += 1;
        let mut items = Vec::new();
        self.skip_space();
        if self.eat(b']') {
            return Some(Value::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_space();
            if self.eat(b']') {
                return Some(Value::Array(items));
            }
            if !self.eat(b',') {
                return None;
            }
        }
    }

    fn string(&mut self) -> Option<String> {
        if !self.eat(b'"') {
            return None;
        }
        let mut out = String::new();
        loop {
            match self.next_char()? {
                '"' => return Some(out),
                '\\' => out.push(self.escape()?),
                control if control < ' ' => return None,
                plain => out.push(plain),
            }
        }
    }

    fn escape(&mut self) -> Option<char> {
        Some(match self.next_char()? {
            '"' => '"',
            '\\' => '\\',
            '/' => '/',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'u' => char::from_u32(self.hex4()?)?,
            _ => return None,
        })
    }

    fn hex4(&mut self) -> Option<u32> {
        let digits = self.text.get(self.at..self.at + 4)?;
        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        self.at += 4;
        u32::from_str_radix(digits, 16).ok()
    }

    fn number(&mut self) -> Option<Value> {
        let start = self.at;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.at += 1;
        }
        self.text[start..self.at]
            .parse::<f64>()
            .ok()
            .map(Value::Number)
    }

    fn word(&mut self, word: &str, value: Value) -> Option<Value> {
        if !self.text[self.at..].starts_with(word) {
            return None;
        }
        self.at += word.len();
        Some(value)
    }

    fn next_char(&mut self) -> Option<char> {
        let next = self.text[self.at..].chars().next()?;
        self.at += next.len_utf8();
        Some(next)
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    fn eat(&mut self, byte: u8) -> bool {
        let found = self.peek() == Some(byte);
        if found {
            self.at += 1;
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::{Value, parse};

    #[test]
    fn reads_objects_arrays_strings_numbers_and_literals() {
        let value = parse(r#" { "a": [1, -2.5e1, true, null], "b": "x\ty\u00e9" } "#)
            .expect("a valid document");

        assert_eq!(
            value.get("a").and_then(Value::as_array).map(<[Value]>::len),
            Some(4)
        );
        assert_eq!(value.get("b").and_then(Value::as_str), Some("x\tyé"));
        assert_eq!(value.get("missing"), None);
    }

    #[test]
    fn refuses_text_that_is_not_exactly_one_document() {
        for text in [
            "",
            "{",
            "[1,]",
            "{\"a\":1,}",
            "{\"a\" 1}",
            "\"open",
            "tru",
            "{} {}",
            "1 2",
            "\"\\x\"",
            "\"\\ud800\"",
            "\"tab\there\"",
            "\"\\u12G4\"",
        ] {
            assert!(parse(text).is_none(), "{text:?}");
        }
    }

    #[test]
    fn nesting_deeper_than_the_limit_is_refused() {
        let deep = format!("{}{}", "[".repeat(40), "]".repeat(40));
        let shallow = format!("{}{}", "[".repeat(8), "]".repeat(8));

        assert!(parse(&deep).is_none());
        assert!(parse(&shallow).is_some());
    }
}
