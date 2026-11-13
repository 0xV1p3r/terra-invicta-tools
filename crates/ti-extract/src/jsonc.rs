//! Lenient JSON reader for the game's template files (port of `jsonc.py`).
//!
//! The templates are mostly JSON, but some contain `//` and `/* */` comments and
//! trailing commas. Comments are blanked out (not removed) so that line/column
//! numbers in parse errors still point at the original file.
//! Missing commas are inserted only between complete tokens in containers.
//! Duplicate object keys are reported and reject the whole file, like FullSerializer.
//!
//! [`Value`] follows Python's `json` module: integers and floats stay distinct, `NaN` and
//! `±Infinity` are accepted, objects keep their key order, and equality is Python's
//! (`1 == 1.0 == true`). Parse errors carry Python's messages and character positions.

use std::fmt;
use std::path::Path;

/// A JSON value with Python `json` semantics.
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i128),
    /// An integer outside `i128`, as its decimal digits.
    BigInt(String),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    /// Keys in document order.
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Self::Object(pairs) => Some(pairs),
            _ => None,
        }
    }

    pub fn is_container(&self) -> bool {
        matches!(self, Self::Array(_) | Self::Object(_))
    }

    /// Python's `type(v).__name__`.
    pub fn py_type_name(&self) -> &'static str {
        match self {
            Self::Null => "NoneType",
            Self::Bool(_) => "bool",
            Self::Int(_) | Self::BigInt(_) => "int",
            Self::Float(_) => "float",
            Self::Str(_) => "str",
            Self::Array(_) => "list",
            Self::Object(_) => "dict",
        }
    }

    /// Python's numeric value of a `bool`/`int`/`float`, for cross-type comparison.
    fn number(&self) -> Option<Number> {
        match *self {
            Self::Bool(b) => Some(Number::Int(b as i128)),
            Self::Int(i) => Some(Number::Int(i)),
            Self::Float(f) => Some(Number::Float(f)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Number {
    Int(i128),
    Float(f64),
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        match (*self, *other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Int(i), Self::Float(f)) | (Self::Float(f), Self::Int(i)) => {
                // Python compares int and float exactly.
                f.is_finite() && f.fract() == 0.0 && f.abs() < 1.7e38 && f as i128 == i
            }
        }
    }
}

/// Python equality: numbers compare by value across `bool`/`int`/`float`, objects ignore key order.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(a), Some(b)) = (self.number(), other.number()) {
            return a == b;
        }
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::BigInt(a), Self::BigInt(b)) => a == b,
            (Self::Str(a), Self::Str(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a.len() == b.len() && a.iter().all(|(k, v)| other.get(k) == Some(v)),
            _ => false,
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Str(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::Str(s)
    }
}

impl From<&String> for Value {
    fn from(s: &String) -> Self {
        Self::Str(s.clone())
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

macro_rules! from_int {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(i: $t) -> Self {
                Self::Int(i as i128)
            }
        }
    )*};
}
from_int!(i32, i64, u32, u64, usize);

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        v.map_or(Self::Null, Into::into)
    }
}

impl From<Vec<Value>> for Value {
    fn from(items: Vec<Value>) -> Self {
        Self::Array(items)
    }
}

/// An object from `(key, value)` pairs, in order.
pub fn object<K: Into<String>>(pairs: impl IntoIterator<Item = (K, Value)>) -> Value {
    Value::Object(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

/// A `json.JSONDecodeError` (or other `ValueError`), displayed like Python's `str(e)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

/// `str.isspace()`, also what `\s` matches in Python's `re` for `str` patterns.
pub fn py_isspace(c: char) -> bool {
    matches!(
        c,
        '\t'..='\r' | '\x1c'..='\x1f' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

pub fn strip_comments(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let n = chars.len();
    let mut i = 0;
    let mut in_string = false;
    while i < n {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < n {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
        } else if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            let end = chars[i..].iter().position(|&c| c == '\n').map_or(n, |p| i + p);
            out.extend(std::iter::repeat_n(' ', end - i));
            i = end;
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            let end = (i + 2..n.saturating_sub(1))
                .find(|&j| chars[j] == '*' && chars[j + 1] == '/')
                .map_or(n, |j| j + 2);
            // keep newlines so line numbers stay stable
            out.extend(chars[i..end].iter().map(|&c| if c == '\n' { '\n' } else { ' ' }));
            i = end;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// End of the regex `"(?:\\.|[^"\\])*"` matched at `start` (a `"`), if it matches.
fn match_string(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start + 1;
    while i < chars.len() {
        match chars[i] {
            '"' => return Some(i + 1),
            // `.` does not match a newline
            '\\' if i + 1 < chars.len() && chars[i + 1] != '\n' => i += 2,
            '\\' => return None,
            _ => i += 1,
        }
    }
    None
}

/// Python's `re.sub(r'("(?:\\.|[^"\\])*")|,(\s*[\]}])', ...)`: drop commas before `]`/`}`.
pub fn strip_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"'
            && let Some(end) = match_string(&chars, i)
        {
            out.extend(&chars[i..end]);
            i = end;
            continue;
        }
        if chars[i] == ',' {
            let mut j = i + 1;
            while j < chars.len() && py_isspace(chars[j]) {
                j += 1;
            }
            if j < chars.len() && matches!(chars[j], ']' | '}') {
                out.extend(&chars[i + 1..=j]);
                i = j + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_number_token(token: &[char]) -> bool {
    // -?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?, full match
    let mut i = 0;
    let digits = |i: &mut usize| {
        let start = *i;
        while *i < token.len() && token[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > start
    };
    if token.first() == Some(&'-') {
        i += 1;
    }
    match token.get(i) {
        Some('0') => i += 1,
        Some('1'..='9') => {
            digits(&mut i);
        }
        _ => return false,
    }
    if token.get(i) == Some(&'.') {
        i += 1;
        if !digits(&mut i) {
            return false;
        }
    }
    if matches!(token.get(i), Some('e' | 'E')) {
        i += 1;
        if matches!(token.get(i), Some('+' | '-')) {
            i += 1;
        }
        if !digits(&mut i) {
            return false;
        }
    }
    i == token.len()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Expect {
    Key,
    Colon,
    Value,
    End,
}

/// Track container grammar; never repair colons, invalid tokens or root values.
///
/// Comments must already be blanked. Numeric tokens stay intact, so malformed
/// numbers cannot accidentally become several valid array elements.
pub fn insert_missing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut stack: Vec<(char, Expect)> = Vec::new();
    let mut out = String::with_capacity(text.len() + 16);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // _TOKEN = "(?:\\.|[^"\\])*" | [{}\[\],:] | [^\s{}\[\],:"]+
        let end = if c == '"' {
            match match_string(&chars, i) {
                Some(end) => end,
                None => {
                    out.push(c);
                    i += 1;
                    continue;
                }
            }
        } else if matches!(c, '{' | '}' | '[' | ']' | ',' | ':') {
            i + 1
        } else if py_isspace(c) {
            out.push(c);
            i += 1;
            continue;
        } else {
            let mut j = i;
            while j < chars.len() && !py_isspace(chars[j]) && !matches!(chars[j], '{' | '}' | '[' | ']' | ',' | ':' | '"') {
                j += 1;
            }
            j
        };
        let token = &chars[i..end];
        let starts_value = matches!(token[0], '"' | '[' | '{')
            || matches!(
                token.iter().collect::<String>().as_str(),
                "true" | "false" | "null" | "NaN" | "Infinity" | "-Infinity"
            )
            || is_number_token(token);
        if let Some(top) = stack.last_mut()
            && top.1 == Expect::End
            && starts_value
            && (top.0 == '[' || token[0] == '"')
        {
            out.push(',');
            top.1 = if top.0 == '[' { Expect::Value } else { Expect::Key };
        }
        match token {
            ['[' | '{'] => {
                if let Some(top) = stack.last_mut() {
                    top.1 = Expect::End;
                }
                stack.push((token[0], if token[0] == '[' { Expect::Value } else { Expect::Key }));
            }
            [']' | '}'] => {
                stack.pop();
            }
            _ => {
                if let Some(top) = stack.last_mut() {
                    if token == [','] {
                        top.1 = if top.0 == '[' { Expect::Value } else { Expect::Key };
                    } else if token == [':'] {
                        top.1 = Expect::Value;
                    } else if top.1 == Expect::Key && token[0] == '"' {
                        top.1 = Expect::Colon;
                    } else if top.1 == Expect::Value && starts_value {
                        top.1 = Expect::End;
                    }
                }
            }
        }
        out.extend(token);
        i = end;
    }
    out
}

/// A parsed object before duplicate keys are resolved.
enum Raw {
    Scalar(Value),
    Array(Vec<Raw>),
    Object(Vec<(String, Raw)>),
}

/// Python's C JSON scanner (`json.loads`) over characters, with its error messages.
struct Parser<'a> {
    s: &'a [char],
}

fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

enum Fail {
    /// `StopIteration`: no value starts here; reported as "Expecting value".
    Stop(usize),
    Error(&'static str, usize),
}

impl Parser<'_> {
    fn error(&self, fail: Fail) -> ParseError {
        let (msg, pos) = match fail {
            Fail::Stop(pos) => ("Expecting value", pos),
            Fail::Error(msg, pos) => (msg, pos),
        };
        decode_error(self.s, msg, pos)
    }

    fn skip_ws(&self, mut i: usize) -> usize {
        while i < self.s.len() && is_ws(self.s[i]) {
            i += 1;
        }
        i
    }

    fn starts(&self, i: usize, word: &str) -> bool {
        let w: Vec<char> = word.chars().collect();
        self.s.get(i..i + w.len()) == Some(&w[..])
    }

    fn scan(&self, i: usize) -> Result<(Raw, usize), Fail> {
        let Some(&c) = self.s.get(i) else {
            return Err(Fail::Stop(i));
        };
        match c {
            '"' => self.string(i + 1).map(|(s, end)| (Raw::Scalar(Value::Str(s)), end)),
            '{' => self.object(i + 1),
            '[' => self.array(i + 1),
            'n' if self.starts(i, "null") => Ok((Raw::Scalar(Value::Null), i + 4)),
            't' if self.starts(i, "true") => Ok((Raw::Scalar(Value::Bool(true)), i + 4)),
            'f' if self.starts(i, "false") => Ok((Raw::Scalar(Value::Bool(false)), i + 5)),
            'N' if self.starts(i, "NaN") => Ok((Raw::Scalar(Value::Float(f64::NAN)), i + 3)),
            'I' if self.starts(i, "Infinity") => Ok((Raw::Scalar(Value::Float(f64::INFINITY)), i + 8)),
            '-' if self.starts(i, "-Infinity") => Ok((Raw::Scalar(Value::Float(f64::NEG_INFINITY)), i + 9)),
            _ => self.number(i).map(|(v, end)| (Raw::Scalar(v), end)),
        }
    }

    fn number(&self, start: usize) -> Result<(Value, usize), Fail> {
        let s = self.s;
        let digit = |i: usize| s.get(i).is_some_and(char::is_ascii_digit);
        let mut i = start;
        if s.get(i) == Some(&'-') {
            i += 1;
        }
        match s.get(i) {
            Some('1'..='9') => {
                while digit(i) {
                    i += 1;
                }
            }
            Some('0') => i += 1,
            _ => return Err(Fail::Stop(start)),
        }
        let mut is_float = false;
        if s.get(i) == Some(&'.') && digit(i + 1) {
            is_float = true;
            i += 1;
            while digit(i) {
                i += 1;
            }
        }
        if matches!(s.get(i), Some('e' | 'E')) {
            let e_start = i;
            i += 1;
            if matches!(s.get(i), Some('+' | '-')) {
                i += 1;
            }
            let digits = i;
            while digit(i) {
                i += 1;
            }
            if i > digits {
                is_float = true;
            } else {
                i = e_start;
            }
        }
        let text: String = s[start..i].iter().collect();
        let value = if is_float {
            Value::Float(text.parse().expect("valid float literal"))
        } else {
            text.parse().map_or_else(|_| Value::BigInt(text.clone()), Value::Int)
        };
        Ok((value, i))
    }

    /// Python's `scanstring`; `end` is just past the opening quote.
    fn string(&self, mut end: usize) -> Result<(String, usize), Fail> {
        let s = self.s;
        let begin = end - 1;
        let mut out = String::new();
        loop {
            let mut next = end;
            let mut c = '\0';
            while next < s.len() {
                c = s[next];
                if c == '"' || c == '\\' {
                    break;
                } else if c <= '\x1f' {
                    return Err(Fail::Error("Invalid control character at", next));
                }
                next += 1;
            }
            if next >= s.len() {
                return Err(Fail::Error("Unterminated string starting at", begin));
            }
            out.extend(&s[end..next]);
            next += 1;
            if c == '"' {
                return Ok((out, next));
            }
            if next == s.len() {
                return Err(Fail::Error("Unterminated string starting at", begin));
            }
            let e = s[next];
            if e != 'u' {
                end = next + 1;
                out.push(match e {
                    '"' | '\\' | '/' => e,
                    'b' => '\x08',
                    'f' => '\x0c',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    _ => return Err(Fail::Error("Invalid \\escape", end - 2)),
                });
                continue;
            }
            next += 1;
            end = next + 4;
            if end >= s.len() {
                return Err(Fail::Error("Invalid \\uXXXX escape", next - 1));
            }
            let hex = |from: usize, to: usize| -> Result<u32, Fail> {
                s[from..to].iter().try_fold(0u32, |acc, d| {
                    d.to_digit(16).map(|d| acc << 4 | d).ok_or(Fail::Error("Invalid \\uXXXX escape", to - 5))
                })
            };
            let mut code = hex(next, end)?;
            if (0xd800..0xdc00).contains(&code) && end + 6 < s.len() && s[end] == '\\' && s[end + 1] == 'u' {
                let low = hex(end + 2, end + 6)?;
                if (0xdc00..0xe000).contains(&low) {
                    code = 0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00);
                    end += 6;
                }
            }
            // A lone surrogate cannot be stored in a Rust string.
            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
        }
    }

    fn object(&self, start: usize) -> Result<(Raw, usize), Fail> {
        let s = self.s;
        let mut pairs = Vec::new();
        let mut i = self.skip_ws(start);
        if s.get(i) != Some(&'}') {
            loop {
                if s.get(i) != Some(&'"') {
                    return Err(Fail::Error("Expecting property name enclosed in double quotes", i));
                }
                let (key, next) = self.string(i + 1)?;
                i = self.skip_ws(next);
                if s.get(i) != Some(&':') {
                    return Err(Fail::Error("Expecting ':' delimiter", i));
                }
                i = self.skip_ws(i + 1);
                let (value, next) = self.scan(i)?;
                pairs.push((key, value));
                i = self.skip_ws(next);
                if s.get(i) == Some(&'}') {
                    break;
                }
                if s.get(i) != Some(&',') {
                    return Err(Fail::Error("Expecting ',' delimiter", i));
                }
                i = self.skip_ws(i + 1);
            }
        }
        Ok((Raw::Object(pairs), i + 1))
    }

    fn array(&self, start: usize) -> Result<(Raw, usize), Fail> {
        let s = self.s;
        let mut items = Vec::new();
        let mut i = self.skip_ws(start);
        if s.get(i) != Some(&']') {
            loop {
                let (value, next) = self.scan(i)?;
                items.push(value);
                i = self.skip_ws(next);
                if s.get(i) == Some(&']') {
                    break;
                }
                if s.get(i) != Some(&',') {
                    return Err(Fail::Error("Expecting ',' delimiter", i));
                }
                i = self.skip_ws(i + 1);
            }
        }
        Ok((Raw::Array(items), i + 1))
    }
}

/// `json.JSONDecodeError` text: `msg: line L column C (char P)`, positions in characters.
fn decode_error(s: &[char], msg: &str, pos: usize) -> ParseError {
    let before = &s[..pos.min(s.len())];
    let line = before.iter().filter(|&&c| c == '\n').count() + 1;
    let column = before.iter().rposition(|&c| c == '\n').map_or(pos + 1, |nl| pos - nl);
    ParseError(format!("{msg}: line {line} column {column} (char {pos})"))
}

fn parse_raw(text: &str) -> Result<Raw, ParseError> {
    let chars: Vec<char> = text.chars().collect();
    if chars.first() == Some(&'\u{feff}') {
        return Err(decode_error(&chars, "Unexpected UTF-8 BOM (decode using utf-8-sig)", 0));
    }
    let p = Parser { s: &chars };
    let start = p.skip_ws(0);
    let (value, end) = p.scan(start).map_err(|f| p.error(f))?;
    let end = p.skip_ws(end);
    if end != chars.len() {
        return Err(decode_error(&chars, "Extra data", end));
    }
    Ok(value)
}

/// Strict `json.loads` (plus `NaN`/`Infinity`); duplicate keys keep the last value.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    fn plain(raw: Raw) -> Value {
        match raw {
            Raw::Scalar(v) => v,
            Raw::Array(items) => Value::Array(items.into_iter().map(plain).collect()),
            Raw::Object(pairs) => {
                let mut out: Vec<(String, Value)> = Vec::with_capacity(pairs.len());
                for (k, v) in pairs {
                    let v = plain(v);
                    match out.iter_mut().find(|(key, _)| *key == k) {
                        Some(slot) => slot.1 = v,
                        None => out.push((k, v)),
                    }
                }
                Value::Object(out)
            }
        }
    }
    parse_raw(text).map(plain)
}

/// Context for issues: the template file and layer being read.
#[derive(Debug, Clone, Copy, Default)]
pub struct Context<'a> {
    pub template: Option<&'a str>,
    pub layer: Option<&'a str>,
}

pub const DUPLICATE_KEY_ERROR: &str = "duplicate JSON key (whole file rejected by FullSerializer)";

/// Parse a template file's text; duplicate keys are appended to `issues` and fail the file.
pub fn loads(text: &str, issues: &mut Vec<Value>, ctx: Context<'_>) -> Result<Value, ParseError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let raw = parse_raw(&strip_trailing_commas(&insert_missing_commas(&strip_comments(text))))?;

    struct Walk<'a, 'b> {
        issues: &'a mut Vec<Value>,
        ctx: Context<'b>,
        duplicate: bool,
    }
    impl Walk<'_, '_> {
        fn materialize(&mut self, raw: Raw, path: &str, data_name: Option<&str>) -> Value {
            match raw {
                Raw::Scalar(v) => v,
                Raw::Array(items) => Value::Array(
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(i, v)| self.materialize(v, &format!("{path}[{i}]"), data_name))
                        .collect(),
                ),
                Raw::Object(pairs) => {
                    // dict(pairs) keeps the last value of a repeated key
                    let own = pairs.iter().rev().find(|(k, _)| k == "dataName").and_then(|(_, v)| match v {
                        Raw::Scalar(Value::Str(s)) => Some(s.clone()),
                        _ => None,
                    });
                    let data_name = own.as_deref().or(data_name);
                    let mut out: Vec<(String, Value)> = Vec::with_capacity(pairs.len());
                    for (key, child) in pairs {
                        let existing = out.iter().position(|(k, _)| *k == key);
                        if existing.is_some() {
                            self.duplicate = true;
                            let mut issue = vec![("kind", Value::from("duplicate_json_key")), ("severity", "error".into())];
                            issue.extend(self.ctx.template.map(|t| ("template", t.into())));
                            issue.extend(self.ctx.layer.map(|l| ("layer", l.into())));
                            issue.push(("key", key.as_str().into()));
                            issue.push(("path", path.into()));
                            issue.extend(data_name.map(|d| ("dataName", d.into())));
                            self.issues.push(object(issue));
                        }
                        let value = self.materialize(child, &format!("{path}.{key}"), data_name);
                        match existing {
                            Some(i) => out[i].1 = value,
                            None => out.push((key, value)),
                        }
                    }
                    Value::Object(out)
                }
            }
        }
    }

    let mut walk = Walk { issues, ctx, duplicate: false };
    let value = walk.materialize(raw, "$", None);
    // fsJsonParser.Dictionary.Add throws before StringSerializationAPI can
    // deserialize any records: the entire source file fails, not just a record.
    if walk.duplicate {
        return Err(ParseError(DUPLICATE_KEY_ERROR.into()));
    }
    Ok(value)
}

/// Python's `bytes.decode("utf-8")` error text.
fn utf8_error(bytes: &[u8], e: std::str::Utf8Error) -> ParseError {
    let pos = e.valid_up_to();
    let byte = bytes[pos];
    let reason = match e.error_len() {
        None => "unexpected end of data",
        Some(_) if (0x80..0xc2).contains(&byte) || byte >= 0xf5 => "invalid start byte",
        Some(_) => "invalid continuation byte",
    };
    ParseError(format!("'utf-8' codec can't decode byte 0x{byte:02x} in position {pos}: {reason}"))
}

/// `bytes` as Python's `read_text(encoding="utf-8-sig")`: strict UTF-8, one BOM dropped,
/// universal newlines (`\r\n` and `\r` become `\n`).
pub fn decode_text(bytes: &[u8]) -> Result<String, ParseError> {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|e| utf8_error(bytes, e))?;
    Ok(universal_newlines(text))
}

pub fn universal_newlines(text: &str) -> String {
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.to_owned()
    }
}

/// [`loads`] over raw file bytes.
pub fn load_bytes(bytes: &[u8], issues: &mut Vec<Value>, ctx: Context<'_>) -> Result<Value, ParseError> {
    loads(&decode_text(bytes)?, issues, ctx)
}

/// Read and parse a template file. IO errors are `Err(Ok(_))`.
pub fn load(path: &Path, issues: &mut Vec<Value>, ctx: Context<'_>) -> std::io::Result<Result<Value, ParseError>> {
    Ok(load_bytes(&std::fs::read(path)?, issues, ctx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loads_plain(text: &str) -> Result<Value, ParseError> {
        loads(text, &mut Vec::new(), Context::default())
    }

    fn json(text: &str) -> Value {
        parse(text).unwrap()
    }

    #[test]
    fn comments_and_trailing_commas() {
        let text = "\u{feff}[\n      {\"dataName\": \"a\", // line comment\n       \"url\": \"http://x//y\", /* block\n       comment */ \"list\": [1, 2, ],\n       \"s\": \"has \\\" quote, ]\",},\n    ]";
        assert_eq!(
            loads_plain(text).unwrap(),
            json(r#"[{"dataName": "a", "url": "http://x//y", "list": [1, 2], "s": "has \" quote, ]"}]"#)
        );
    }

    #[test]
    fn error_positions_are_preserved() {
        let e = loads_plain("/* a\nb */\n{\"x\": }").unwrap_err();
        assert!(e.0.contains("line 3"), "{e}");
        assert_eq!(e.0, "Expecting value: line 3 column 7 (char 16)");
    }

    #[test]
    fn duplicates_even_in_replaced_objects() {
        let mut issues = Vec::new();
        let e = loads(r#"{"a": {"x": 1, "x": 2}, "a": 3}"#, &mut issues, Context::default()).unwrap_err();
        assert!(e.0.contains("duplicate JSON key"));
        let keys: Vec<_> = issues.iter().map(|i| i.get("key").unwrap().as_str().unwrap()).collect();
        assert_eq!(keys, ["x", "a"]);
        assert!(issues.iter().all(|i| i.get("dataName").is_none()));
    }

    #[test]
    fn nonfinite_numbers_and_case_sensitive_keys() {
        let data = loads_plain(r#"{"n": NaN, "i": Infinity, "I": -Infinity}"#).unwrap();
        assert!(matches!(data.get("n"), Some(Value::Float(f)) if f.is_nan()));
        assert_eq!(data.get("i"), Some(&Value::Float(f64::INFINITY)));
        assert_eq!(data.get("I"), Some(&Value::Float(f64::NEG_INFINITY)));
    }

    #[test]
    fn missing_commas() {
        for (text, expected) in [
            ("[1 2]", "[1, 2]"),
            (r#"{"x": 1 "y": 2}"#, r#"{"x": 1, "y": 2}"#),
            (r#"[{}{}[]"a" true false null]"#, r#"[{}, {}, [], "a", true, false, null]"#),
            (
                r#"{"a": [1 /* comment */ 2] "b": {"x": "a,b" "y": "//"}}"#,
                r#"{"a": [1, 2], "b": {"x": "a,b", "y": "//"}}"#,
            ),
            ("[\"a\\\"b\" // comment\n \"c\"]", r#"["a\"b", "c"]"#),
        ] {
            assert_eq!(loads_plain(text).unwrap(), json(expected), "{text}");
        }
    }

    #[test]
    fn missing_comma_repair_does_not_hide_other_errors() {
        for text in ["[1e 2]", "[1.2.3]", r#"{"x" 1}"#, "[1,,2]", "1 2", r#"{"x": }"#] {
            assert!(loads_plain(text).is_err(), "{text}");
        }
    }

    #[test]
    fn python_error_messages() {
        for (text, expected) in [
            ("[1,,2]", "Expecting value: line 1 column 4 (char 3)"),
            ("1 2", "Extra data: line 1 column 3 (char 2)"),
            (r#"{"x" 1}"#, "Expecting ':' delimiter: line 1 column 6 (char 5)"),
            ("[\"abc", "Unterminated string starting at: line 1 column 2 (char 1)"),
            ("[\"a\\qb\"]", "Invalid \\escape: line 1 column 4 (char 3)"),
            ("{1: 2}", "Expecting property name enclosed in double quotes: line 1 column 2 (char 1)"),
            ("", "Expecting value: line 1 column 1 (char 0)"),
        ] {
            assert_eq!(loads_plain(text).unwrap_err().0, expected, "{text}");
        }
    }

    #[test]
    fn numbers_and_strings() {
        assert!(matches!(json("1"), Value::Int(1)));
        assert!(matches!(json("-0"), Value::Int(0)));
        assert!(matches!(json("1.0"), Value::Float(_)));
        assert!(matches!(json("1e999"), Value::Float(f) if f == f64::INFINITY));
        assert!(matches!(json("123456789012345678901234567890123456789012"), Value::BigInt(_)));
        assert_eq!(json(r#""\u00e9\ud83d\ude00\n""#), Value::from("é😀\n"));
        assert_eq!(json("[1, true]"), json("[1.0, 1]"));
        assert_ne!(json(r#"{"a": 1}"#), json(r#"{"a": 1, "b": 2}"#));
        assert_eq!(json(r#"{"a": 1, "b": 2}"#), json(r#"{"b": 2, "a": 1}"#));
    }
}
