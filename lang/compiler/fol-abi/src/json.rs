//! A small, strict JSON reader.
//!
//! This crate writes canonical JSON by hand and may depend only on
//! `fol-types`, so it reads it by hand too. The reader is deliberately narrow:
//! it accepts the subset this crate emits and refuses everything else, which
//! is the right posture for a file the compiler trusts enough to build a
//! namespace from.
//!
//! Numbers are read as `i64` because every number in an ABI manifest is an
//! index, a width, a status code, or a version. There is no float in the
//! schema, so admitting one would only create a way to lose precision.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Integer(i64),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    /// Parse one complete JSON document.
    pub fn parse(text: &str) -> Result<Self, JsonError> {
        let bytes = text.as_bytes();
        let mut reader = Reader { bytes, offset: 0 };
        reader.skip_whitespace();
        let value = reader.read_value()?;
        reader.skip_whitespace();
        if reader.offset != bytes.len() {
            return Err(JsonError::TrailingContent {
                offset: reader.offset,
            });
        }
        Ok(value)
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, JsonValue>> {
        match self {
            Self::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    /// A required field, with the field name in the error.
    pub fn field(&self, name: &str) -> Result<&JsonValue, JsonError> {
        self.as_object()
            .ok_or_else(|| JsonError::ExpectedObject {
                field: name.to_string(),
            })?
            .get(name)
            .ok_or_else(|| JsonError::MissingField {
                field: name.to_string(),
            })
    }

    pub fn string_field(&self, name: &str) -> Result<&str, JsonError> {
        self.field(name)?.as_str().ok_or_else(|| JsonError::WrongType {
            field: name.to_string(),
            expected: "string",
        })
    }

    pub fn integer_field(&self, name: &str) -> Result<i64, JsonError> {
        self.field(name)?.as_i64().ok_or_else(|| JsonError::WrongType {
            field: name.to_string(),
            expected: "integer",
        })
    }

    pub fn array_field(&self, name: &str) -> Result<&[JsonValue], JsonError> {
        self.field(name)?
            .as_array()
            .ok_or_else(|| JsonError::WrongType {
                field: name.to_string(),
                expected: "array",
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonError {
    UnexpectedEnd,
    UnexpectedByte { offset: usize, found: char },
    TrailingContent { offset: usize },
    InvalidNumber { offset: usize },
    InvalidEscape { offset: usize },
    DuplicateKey { key: String },
    MissingField { field: String },
    ExpectedObject { field: String },
    WrongType { field: String, expected: &'static str },
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEnd => write!(f, "the document ended in the middle of a value"),
            Self::UnexpectedByte { offset, found } => {
                write!(f, "byte {offset}: unexpected '{found}'")
            }
            Self::TrailingContent { offset } => {
                write!(f, "byte {offset}: content after the end of the document")
            }
            Self::InvalidNumber { offset } => write!(
                f,
                "byte {offset}: an ABI manifest carries only integers, and this is not one"
            ),
            Self::InvalidEscape { offset } => write!(f, "byte {offset}: unsupported string escape"),
            Self::DuplicateKey { key } => write!(f, "the key '{key}' appears twice"),
            Self::MissingField { field } => write!(f, "missing required field '{field}'"),
            Self::ExpectedObject { field } => {
                write!(f, "expected an object while reading field '{field}'")
            }
            Self::WrongType { field, expected } => {
                write!(f, "field '{field}' must be {expected}")
            }
        }
    }
}

impl std::error::Error for JsonError {}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Reader<'_> {
    fn skip_whitespace(&mut self) {
        while matches!(
            self.bytes.get(self.offset),
            Some(b' ' | b'\t' | b'\n' | b'\r')
        ) {
            self.offset += 1;
        }
    }

    fn peek(&self) -> Result<u8, JsonError> {
        self.bytes.get(self.offset).copied().ok_or(JsonError::UnexpectedEnd)
    }

    fn expect(&mut self, byte: u8) -> Result<(), JsonError> {
        if self.peek()? != byte {
            return Err(self.unexpected());
        }
        self.offset += 1;
        Ok(())
    }

    fn unexpected(&self) -> JsonError {
        JsonError::UnexpectedByte {
            offset: self.offset,
            found: self.bytes.get(self.offset).map_or('?', |b| *b as char),
        }
    }

    fn read_value(&mut self) -> Result<JsonValue, JsonError> {
        match self.peek()? {
            b'{' => self.read_object(),
            b'[' => self.read_array(),
            b'"' => Ok(JsonValue::String(self.read_string()?)),
            b't' => self.read_literal("true").map(|()| JsonValue::Bool(true)),
            b'f' => self.read_literal("false").map(|()| JsonValue::Bool(false)),
            b'n' => self.read_literal("null").map(|()| JsonValue::Null),
            b'-' | b'0'..=b'9' => self.read_integer(),
            _ => Err(self.unexpected()),
        }
    }

    fn read_literal(&mut self, literal: &str) -> Result<(), JsonError> {
        if self.bytes[self.offset..].starts_with(literal.as_bytes()) {
            self.offset += literal.len();
            return Ok(());
        }
        Err(self.unexpected())
    }

    fn read_integer(&mut self) -> Result<JsonValue, JsonError> {
        let start = self.offset;
        if self.peek()? == b'-' {
            self.offset += 1;
        }
        while matches!(self.bytes.get(self.offset), Some(b'0'..=b'9')) {
            self.offset += 1;
        }
        // A fraction or exponent is not a mistake to round; the schema has no
        // place to put one, so reading it would silently change the value.
        if matches!(self.bytes.get(self.offset), Some(b'.' | b'e' | b'E')) {
            return Err(JsonError::InvalidNumber { offset: start });
        }
        std::str::from_utf8(&self.bytes[start..self.offset])
            .ok()
            .and_then(|text| text.parse::<i64>().ok())
            .map(JsonValue::Integer)
            .ok_or(JsonError::InvalidNumber { offset: start })
    }

    fn read_string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut text = String::new();
        loop {
            let byte = self.peek()?;
            self.offset += 1;
            match byte {
                b'"' => return Ok(text),
                b'\\' => {
                    let escape = self.peek()?;
                    self.offset += 1;
                    text.push(match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'u' => self.read_unicode_escape()?,
                        _ => {
                            return Err(JsonError::InvalidEscape {
                                offset: self.offset - 1,
                            })
                        }
                    });
                }
                _ => {
                    // Multi-byte UTF-8 arrives one byte at a time here, so the
                    // whole sequence is collected before it is decoded.
                    let start = self.offset - 1;
                    while self
                        .bytes
                        .get(self.offset)
                        .is_some_and(|byte| (0x80..0xC0).contains(byte))
                    {
                        self.offset += 1;
                    }
                    let slice = &self.bytes[start..self.offset];
                    let decoded = std::str::from_utf8(slice)
                        .map_err(|_| JsonError::UnexpectedByte {
                            offset: start,
                            found: '?',
                        })?;
                    text.push_str(decoded);
                }
            }
        }
    }

    fn read_unicode_escape(&mut self) -> Result<char, JsonError> {
        let start = self.offset;
        let end = start + 4;
        let digits = self
            .bytes
            .get(start..end)
            .ok_or(JsonError::UnexpectedEnd)?;
        let text = std::str::from_utf8(digits).map_err(|_| JsonError::InvalidEscape { offset: start })?;
        let code = u32::from_str_radix(text, 16)
            .map_err(|_| JsonError::InvalidEscape { offset: start })?;
        self.offset = end;
        char::from_u32(code).ok_or(JsonError::InvalidEscape { offset: start })
    }

    fn read_array(&mut self) -> Result<JsonValue, JsonError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek()? == b']' {
            self.offset += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            self.skip_whitespace();
            items.push(self.read_value()?);
            self.skip_whitespace();
            match self.peek()? {
                b',' => self.offset += 1,
                b']' => {
                    self.offset += 1;
                    return Ok(JsonValue::Array(items));
                }
                _ => return Err(self.unexpected()),
            }
        }
    }

    fn read_object(&mut self) -> Result<JsonValue, JsonError> {
        self.expect(b'{')?;
        let mut map = BTreeMap::new();
        self.skip_whitespace();
        if self.peek()? == b'}' {
            self.offset += 1;
            return Ok(JsonValue::Object(map));
        }
        loop {
            self.skip_whitespace();
            let key = self.read_string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            let value = self.read_value()?;
            // A duplicate key means two different readers could disagree about
            // what the document says, which a trusted manifest cannot afford.
            if map.insert(key.clone(), value).is_some() {
                return Err(JsonError::DuplicateKey { key });
            }
            self.skip_whitespace();
            match self.peek()? {
                b',' => self.offset += 1,
                b'}' => {
                    self.offset += 1;
                    return Ok(JsonValue::Object(map));
                }
                _ => return Err(self.unexpected()),
            }
        }
    }
}

/// Escape one string for the canonical writer.
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            c if (c as u32) < 0x20 => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_shapes_the_canonical_writer_emits() {
        let value = JsonValue::parse(
            r#"{"alias":"c_math","routines":[{"symbol":"add","status":[0,-1]}],"count":2}"#,
        )
        .expect("canonical output should read back");

        assert_eq!(value.string_field("alias").expect("alias"), "c_math");
        assert_eq!(value.integer_field("count").expect("count"), 2);
        let routines = value.array_field("routines").expect("routines");
        assert_eq!(routines.len(), 1);
        assert_eq!(routines[0].string_field("symbol").expect("symbol"), "add");
        assert_eq!(
            routines[0]
                .array_field("status")
                .expect("status")
                .iter()
                .map(|item| item.as_i64().expect("integer"))
                .collect::<Vec<_>>(),
            vec![0, -1]
        );
    }

    #[test]
    fn a_string_survives_escaping_and_reading_back() {
        let original = "quote:\" backslash:\\ newline:\n tab:\t unicode:\u{2713}";
        let document = format!("{{\"value\":\"{}\"}}", escape(original));

        let value = JsonValue::parse(&document).expect("escaped output should read back");
        assert_eq!(value.string_field("value").expect("value"), original);
    }

    #[test]
    fn multi_byte_utf8_reads_back_unchanged() {
        let value = JsonValue::parse("{\"value\":\"naïve — 日本語\"}").expect("utf-8 should read");
        assert_eq!(value.string_field("value").expect("value"), "naïve — 日本語");
    }

    #[test]
    fn empty_containers_read_as_empty() {
        let value = JsonValue::parse(r#"{"items":[],"nested":{}}"#).expect("empty should read");
        assert!(value.array_field("items").expect("items").is_empty());
        assert!(value
            .field("nested")
            .expect("nested")
            .as_object()
            .expect("object")
            .is_empty());
    }

    #[test]
    fn a_fractional_number_is_refused_rather_than_rounded() {
        assert_eq!(
            JsonValue::parse(r#"{"width":32.5}"#),
            Err(JsonError::InvalidNumber { offset: 9 })
        );
        assert!(matches!(
            JsonValue::parse(r#"{"width":1e9}"#),
            Err(JsonError::InvalidNumber { .. })
        ));
    }

    #[test]
    fn a_duplicate_key_is_refused_rather_than_resolved_by_order() {
        assert_eq!(
            JsonValue::parse(r#"{"alias":"a","alias":"b"}"#),
            Err(JsonError::DuplicateKey {
                key: "alias".to_string()
            })
        );
    }

    #[test]
    fn trailing_content_is_refused() {
        assert!(matches!(
            JsonValue::parse(r#"{"a":1} {"b":2}"#),
            Err(JsonError::TrailingContent { .. })
        ));
    }

    #[test]
    fn a_truncated_document_is_refused() {
        for text in [r#"{"a":"#, r#"{"a":1"#, r#"["#, r#""unterminated"#] {
            assert!(
                JsonValue::parse(text).is_err(),
                "'{text}' should not parse"
            );
        }
    }

    #[test]
    fn a_missing_or_mistyped_field_names_itself() {
        let value = JsonValue::parse(r#"{"alias":"c_math"}"#).expect("should read");

        assert_eq!(
            value.string_field("target"),
            Err(JsonError::MissingField {
                field: "target".to_string()
            })
        );
        assert_eq!(
            value.integer_field("alias"),
            Err(JsonError::WrongType {
                field: "alias".to_string(),
                expected: "integer",
            })
        );
    }

    #[test]
    fn negative_and_boundary_integers_round_trip() {
        let document = format!(
            r#"{{"min":{},"max":{},"zero":0,"neg":-1}}"#,
            i64::MIN,
            i64::MAX
        );
        let value = JsonValue::parse(&document).expect("boundaries should read");

        assert_eq!(value.integer_field("min").expect("min"), i64::MIN);
        assert_eq!(value.integer_field("max").expect("max"), i64::MAX);
        assert_eq!(value.integer_field("zero").expect("zero"), 0);
        assert_eq!(value.integer_field("neg").expect("neg"), -1);
    }
}
