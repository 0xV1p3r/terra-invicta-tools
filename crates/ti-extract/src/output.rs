//! Deterministic file output so that re-extracting an unchanged game yields identical bytes
//! (port of `output.py`). Serialization matches Python's `json.dumps(ensure_ascii=False)`.

use std::fmt::Write as _;
use std::io::Read as _;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::jsonc::{Value, object};
use crate::{Error, Result};

/// Where a record comes from, for `non_finite_number` issues.
#[derive(Debug, Clone, Copy, Default)]
pub struct RecordContext<'a> {
    pub template: Option<&'a str>,
    pub layer: Option<&'a str>,
}

/// Sort keys with `dataName` first, replacing and reporting non-finite numbers.
pub fn canonical_record(value: Value, issues: &mut Vec<Value>, ctx: RecordContext<'_>) -> Value {
    fn walk(value: Value, issues: &mut Vec<Value>, ctx: RecordContext<'_>, data_name: &Value, path: &mut String) -> Value {
        match value {
            Value::Float(f) if !f.is_finite() => {
                issues.push(object([
                    ("kind", Value::from("non_finite_number")),
                    ("template", ctx.template.into()),
                    ("layer", ctx.layer.into()),
                    ("dataName", data_name.clone()),
                    ("path", path.as_str().into()),
                ]));
                Value::Null
            }
            Value::Object(mut pairs) => {
                let data_name = pairs
                    .iter()
                    .find(|(k, _)| k == "dataName")
                    .map_or_else(|| data_name.clone(), |(_, v)| v.clone());
                pairs.sort_by(|(a, _), (b, _)| (a != "dataName", a).cmp(&(b != "dataName", b)));
                let len = path.len();
                let pairs = pairs
                    .into_iter()
                    .map(|(k, v)| {
                        path.push('.');
                        path.push_str(&k);
                        let v = walk(v, issues, ctx, &data_name, path);
                        path.truncate(len);
                        (k, v)
                    })
                    .collect();
                Value::Object(pairs)
            }
            Value::Array(items) => {
                let len = path.len();
                let items = items
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let _ = write!(path, "[{i}]");
                        let v = walk(v, issues, ctx, data_name, path);
                        path.truncate(len);
                        v
                    })
                    .collect();
                Value::Array(items)
            }
            v => v,
        }
    }
    walk(value, issues, ctx, &Value::Null, &mut String::from("$"))
}

/// Python's `repr(float)`: shortest round-trip digits, exponent form outside `1e-4 ..< 1e16`.
pub fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let sci = format!("{:e}", f.abs());
    let (mantissa, exp) = sci.split_once('e').expect("LowerExp has an exponent");
    let exp: i32 = exp.parse().expect("integer exponent");
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let sign = if f.is_sign_negative() { "-" } else { "" };
    // Python's decpt: value = 0.<digits> * 10^decpt
    let decpt = exp + 1;
    if !(-4 < decpt && decpt <= 16) {
        let (first, rest) = digits.split_at(1);
        let dot = if rest.is_empty() { String::new() } else { format!(".{rest}") };
        let esign = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{first}{dot}e{esign}{:02}", exp.abs());
    }
    let n = digits.len() as i32;
    if decpt <= 0 {
        format!("{sign}0.{}{digits}", "0".repeat(-decpt as usize))
    } else if decpt >= n {
        format!("{sign}{digits}{}.0", "0".repeat((decpt - n) as usize))
    } else {
        let (int, frac) = digits.split_at(decpt as usize);
        format!("{sign}{int}.{frac}")
    }
}

/// Python's `round(x, 4)`.
pub fn round4(x: f64) -> f64 {
    format!("{x:.4}").parse().expect("formatted float parses")
}

/// A JSON string literal; `ascii` escapes all non-ASCII like `ensure_ascii=True`.
pub fn write_str(out: &mut String, s: &str, ascii: bool) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            c if c < ' ' || (ascii && !(' '..='~').contains(&c)) => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn non_finite() -> Error {
    Error::Other("Out of range float values are not JSON compliant".into())
}

/// A scalar as `json.dumps(v, ensure_ascii=False)` writes it; containers are not supported.
pub fn scalar(out: &mut String, v: &Value) -> Result<()> {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Value::BigInt(digits) => out.push_str(digits),
        Value::Float(f) if !f.is_finite() => return Err(non_finite()),
        Value::Float(f) => out.push_str(&float_repr(*f)),
        Value::Str(s) => write_str(out, s, false),
        Value::Array(_) | Value::Object(_) => unreachable!("scalar() called with a container"),
    }
    Ok(())
}

fn write_value(out: &mut String, v: &Value, compact: bool, level: usize) -> Result<()> {
    let newline = |out: &mut String, level: usize| {
        if !compact {
            out.push('\n');
            out.extend(std::iter::repeat_n(' ', level));
        }
    };
    match v {
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Object(pairs) if pairs.is_empty() => out.push_str("{}"),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, level + 1);
                write_value(out, item, compact, level + 1)?;
            }
            newline(out, level);
            out.push(']');
        }
        Value::Object(pairs) => {
            out.push('{');
            for (i, (k, item)) in pairs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, level + 1);
                write_str(out, k, false);
                out.push_str(if compact { ":" } else { ": " });
                write_value(out, item, compact, level + 1)?;
            }
            newline(out, level);
            out.push('}');
        }
        v => scalar(out, v)?,
    }
    Ok(())
}

/// `json.dumps(value, allow_nan=False, ensure_ascii=False, indent=1)` (or compact separators)
/// plus a trailing newline.
pub fn dumps(value: &Value, compact: bool) -> Result<String> {
    let mut out = String::new();
    write_value(&mut out, value, compact, 0)?;
    out.push('\n');
    Ok(out)
}

pub fn write_json(path: &Path, value: &Value, compact: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
    }
    std::fs::write(path, dumps(value, compact)?).map_err(|e| io_error(path, e))
}

pub(crate) fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        context: path.display().to_string(),
        source,
    }
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| io_error(path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1 << 20];
    loop {
        let n = file.read(&mut buf).map_err(|e| io_error(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonc::parse;

    #[test]
    fn canonical_record_order() {
        let rec = canonical_record(
            parse(r#"{"z": 1, "dataName": "n", "a": {"y": 1, "b": 2}}"#).unwrap(),
            &mut Vec::new(),
            RecordContext::default(),
        );
        let keys = |v: &Value| v.as_object().unwrap().iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
        assert_eq!(keys(&rec), ["dataName", "a", "z"]);
        assert_eq!(keys(rec.get("a").unwrap()), ["b", "y"]);
    }

    #[test]
    fn non_finite_numbers_become_null() {
        let mut issues = Vec::new();
        let ctx = RecordContext {
            template: Some("T"),
            layer: Some("base"),
        };
        let rec = canonical_record(parse(r#"{"dataName": "x", "n": [NaN, 1e999]}"#).unwrap(), &mut issues, ctx);
        assert_eq!(rec, parse(r#"{"dataName": "x", "n": [null, null]}"#).unwrap());
        assert_eq!(
            dumps(&Value::Array(issues), true).unwrap(),
            concat!(
                r#"[{"kind":"non_finite_number","template":"T","layer":"base","dataName":"x","path":"$.n[0]"},"#,
                r#"{"kind":"non_finite_number","template":"T","layer":"base","dataName":"x","path":"$.n[1]"}]"#,
                "\n"
            )
        );
        for compact in [false, true] {
            assert!(dumps(&Value::Float(f64::NAN), compact).is_err());
        }
    }

    #[test]
    fn python_float_repr() {
        for (f, expected) in [
            (1.0, "1.0"),
            (0.1, "0.1"),
            (-0.0, "-0.0"),
            (1e16, "1e+16"),
            (1.5e16, "1.5e+16"),
            (1e15, "1000000000000000.0"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (123.456, "123.456"),
            (1e100, "1e+100"),
            (5e-324, "5e-324"),
            (1.0 / 3.0, "0.3333333333333333"),
        ] {
            assert_eq!(float_repr(f), expected);
        }
        assert_eq!(round4(0.03125), 0.0312);
        assert_eq!(round4(0.09375), 0.0938);
        assert_eq!(round4(2.0 / 3.0), 0.6667);
    }

    #[test]
    fn python_layout() {
        let v = parse(r#"{"a": [1, {"b": "é\n\u0001"}], "e": [], "o": {}}"#).unwrap();
        assert_eq!(
            dumps(&v, false).unwrap(),
            "{\n \"a\": [\n  1,\n  {\n   \"b\": \"é\\n\\u0001\"\n  }\n ],\n \"e\": [],\n \"o\": {}\n}\n"
        );
        let mut s = String::new();
        write_str(&mut s, "é😀\x7f", true);
        assert_eq!(s, r#""\u00e9\ud83d\ude00\u007f""#);
    }
}
