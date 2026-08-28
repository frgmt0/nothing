use std::fmt;

#[derive(Clone, PartialEq, Debug)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JsonError(pub String);

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for JsonError {}

impl Json {
    pub fn str(s: impl Into<String>) -> Json {
        Json::Str(s.into())
    }

    pub fn obj(fields: Vec<(&str, Json)>) -> Json {
        Json::Obj(
            fields
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    pub fn arr(items: Vec<Json>) -> Json {
        Json::Arr(items)
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(n) => Some(*n),
            Json::Float(f) if f.fract() == 0.0 => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.as_i64().and_then(|n| u64::try_from(n).ok())
    }

    pub fn as_usize(&self) -> Option<usize> {
        self.as_i64().and_then(|n| usize::try_from(n).ok())
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Json::Null => f.write_str("null"),
            Json::Bool(true) => f.write_str("true"),
            Json::Bool(false) => f.write_str("false"),
            Json::Int(n) => write!(f, "{n}"),
            Json::Float(x) => {
                if x.is_finite() {
                    write!(f, "{x}")
                } else {
                    f.write_str("null")
                }
            }
            Json::Str(s) => f.write_str(&escape(s)),
            Json::Arr(items) => {
                f.write_str("[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Json::Obj(fields) => {
                f.write_str("{")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{}:{v}", escape(k))?;
                }
                f.write_str("}")
            }
        }
    }
}

struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    source: &'a str,
}

pub fn parse(text: &str) -> Result<Json, JsonError> {
    let mut p = Parser {
        chars: text.chars().collect(),
        pos: 0,
        source: text,
    };
    p.skip_ws();
    let value = p.value()?;
    p.skip_ws();
    if p.pos != p.chars.len() {
        return Err(JsonError(format!(
            "trailing input at character {} of `{}`",
            p.pos, p.source
        )));
    }
    Ok(value)
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t') | Some('\n') | Some('\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, c: char) -> Result<(), JsonError> {
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(JsonError(format!(
                "expected `{c}` at character {}",
                self.pos
            )))
        }
    }

    fn literal(&mut self, word: &str) -> Result<(), JsonError> {
        for expected in word.chars() {
            if self.bump() != Some(expected) {
                return Err(JsonError(format!("expected `{word}`")));
            }
        }
        Ok(())
    }

    fn value(&mut self) -> Result<Json, JsonError> {
        match self.peek() {
            None => Err(JsonError("unexpected end of input".to_string())),
            Some('n') => {
                self.literal("null")?;
                Ok(Json::Null)
            }
            Some('t') => {
                self.literal("true")?;
                Ok(Json::Bool(true))
            }
            Some('f') => {
                self.literal("false")?;
                Ok(Json::Bool(false))
            }
            Some('"') => self.string().map(Json::Str),
            Some('[') => self.array(),
            Some('{') => self.object(),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(JsonError(format!(
                "unexpected character `{c}` at position {}",
                self.pos
            ))),
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err(JsonError("unterminated string".to_string())),
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('b') => out.push('\u{8}'),
                    Some('f') => out.push('\u{c}'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('u') => {
                        let code = self.hex4()?;
                        if (0xd800..0xdc00).contains(&code) {
                            self.expect('\\')?;
                            self.expect('u')?;
                            let low = self.hex4()?;
                            if !(0xdc00..0xe000).contains(&low) {
                                return Err(JsonError("bad surrogate pair".to_string()));
                            }
                            let combined =
                                0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00);
                            match char::from_u32(combined) {
                                Some(c) => out.push(c),
                                None => return Err(JsonError("bad surrogate pair".to_string())),
                            }
                        } else {
                            match char::from_u32(code) {
                                Some(c) => out.push(c),
                                None => return Err(JsonError("bad \\u escape".to_string())),
                            }
                        }
                    }
                    other => {
                        return Err(JsonError(format!("bad escape `\\{}`", other.unwrap_or(' '))));
                    }
                },
                Some(c) => out.push(c),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let mut value = 0u32;
        for _ in 0..4 {
            let c = self.bump().ok_or_else(|| JsonError("short \\u escape".to_string()))?;
            let digit = c
                .to_digit(16)
                .ok_or_else(|| JsonError(format!("`{c}` is not a hex digit")))?;
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
        }
        let mut floating = false;
        if self.peek() == Some('.') {
            floating = true;
            self.pos += 1;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            floating = true;
            self.pos += 1;
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.pos += 1;
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        if text.is_empty() || text == "-" {
            return Err(JsonError("expected a number".to_string()));
        }
        if floating {
            text.parse::<f64>()
                .map(Json::Float)
                .map_err(|e| JsonError(format!("bad number `{text}`: {e}")))
        } else {
            match text.parse::<i64>() {
                Ok(n) => Ok(Json::Int(n)),
                Err(_) => text
                    .parse::<f64>()
                    .map(Json::Float)
                    .map_err(|e| JsonError(format!("bad number `{text}`: {e}"))),
            }
        }
    }

    fn array(&mut self) -> Result<Json, JsonError> {
        self.expect('[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(',') => {}
                Some(']') => return Ok(Json::Arr(items)),
                _ => return Err(JsonError("expected `,` or `]`".to_string())),
            }
        }
    }

    fn object(&mut self) -> Result<Json, JsonError> {
        self.expect('{')?;
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(Json::Obj(fields));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(':')?;
            self.skip_ws();
            let value = self.value()?;
            fields.push((key, value));
            self.skip_ws();
            match self.bump() {
                Some(',') => {}
                Some('}') => return Ok(Json::Obj(fields)),
                _ => return Err(JsonError("expected `,` or `}`".to_string())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_round_trip() {
        for text in ["null", "true", "false", "0", "-12", "1.5", "\"hi\""] {
            let parsed = parse(text).unwrap();
            assert_eq!(parse(&parsed.to_string()).unwrap(), parsed, "{text}");
        }
    }

    #[test]
    fn objects_and_arrays_round_trip() {
        let text = r#"{"method":"apply","params":{"step":"construct-lam"},"xs":[1,2,3],"ok":true}"#;
        let parsed = parse(text).unwrap();
        assert_eq!(parsed.get("method").unwrap().as_str(), Some("apply"));
        assert_eq!(
            parsed
                .get("params")
                .and_then(|p| p.get("step"))
                .and_then(Json::as_str),
            Some("construct-lam")
        );
        assert_eq!(parsed.get("xs").unwrap().as_arr().unwrap().len(), 3);
        assert_eq!(parse(&parsed.to_string()).unwrap(), parsed);
    }

    #[test]
    fn escapes_survive_the_round_trip() {
        let awkward = "λx0:Num. ⦇⦈\n\"quoted\"\ttabbed\\";
        let encoded = Json::str(awkward).to_string();
        assert_eq!(parse(&encoded).unwrap().as_str(), Some(awkward));
    }

    #[test]
    fn surrogate_pairs_decode() {
        assert_eq!(parse("\"\\ud83d\\ude00\"").unwrap().as_str(), Some("😀"));
    }

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        for text in [
            "", "{", "}", "[1,", "\"unterminated", "{\"a\"}", "tru", "-", "01x", "{} {}",
        ] {
            assert!(parse(text).is_err(), "`{text}` should not parse");
        }
    }

    #[test]
    fn a_response_line_never_contains_a_newline() {
        let value = Json::obj(vec![("render", Json::str("a\nb"))]);
        assert!(!value.to_string().contains('\n'));
    }
}
