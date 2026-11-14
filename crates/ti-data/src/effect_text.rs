//! Offline effect descriptions, following `docs/research/game-semantics.md` §§4–5.
//!
//! Arguments are already strings, as in the game: composite format specifiers do
//! not reformat them. State names are supplied without invented faction colors.
//! Missing runtime state uses explicit neutral placeholders; missing localization
//! and invalid composite formats can fall back to the caller's raw key.

use serde_json::Value;

pub trait EffectEnv {
    /// Localized string for a key; the caller applies language fallback.
    fn loc(&self, key: &str) -> Option<String>;
    /// Display name of any template with this dataName, or None if unknown.
    fn template_display_name(&self, data_name: &str) -> Option<String>;
    /// Display names of traits whose grouping equals this value.
    fn trait_names_in_grouping(&self, grouping: i64) -> Vec<String>;
    /// UI label of a councilor attribute, such as Persuasion.
    fn councilor_attribute_label(&self, attribute: &str) -> Option<String>;
}

/// Offline stand-ins for runtime states. Tech/project tooltips use the acting
/// faction as state1, and usually leave state2 empty.
#[derive(Debug, Clone, Default)]
pub struct EffectContext {
    pub state1_name: Option<String>,
    pub state2_name: Option<String>,
    pub state1_faction: Option<String>,
    pub state2_faction: Option<String>,
    pub state1_nation_with_article: Option<String>,
    pub state2_nation_with_article: Option<String>,
    pub state1_councilor_trait: Option<String>,
    pub state1_executive_nations: Vec<String>,
    /// Shared population multiplier for slots 23/24; None means 1.0.
    pub pop_scaling: Option<f64>,
}

/// The 30 positional arguments for a raw effect record.
///
/// Unknown states use `[state]`, `[nation]`, `[region]`, `[faction]`, or `[trait]`.
/// An empty executive-nation list and an unknown trait grouping give empty lists.
/// Faction slots fall back to the state's name. Slots 21/22 capitalize its first
/// visible letter, preserving any supplied rich-text tags. Slots 23/24 share the
/// context's population multiplier and clamp to ±2·abs(value).
pub fn effect_args(effect: &Value, ctx: &EffectContext, env: &dyn EffectEnv) -> Vec<String> {
    let value = effect.get("value").and_then(Value::as_f64).unwrap_or(0.0);
    let duration = effect.get("duration_months").and_then(Value::as_f64).unwrap_or(0.0);
    let str_value = match effect.get("strValue") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    let state1 = ctx.state1_name.as_deref();
    let state2 = ctx.state2_name.as_deref();
    let faction1 = ctx.state1_faction.as_deref().or(state1).unwrap_or("[faction]");
    let faction2 = ctx.state2_faction.as_deref().or(state2).unwrap_or("[faction]");
    let limit = 2.0 * value.abs();
    let scaled = (value * ctx.pop_scaling.unwrap_or(1.0)).clamp(-limit, limit);
    vec![
        format_big_or_small_number(value, env),
        state1.unwrap_or("[state]").to_owned(),
        state2.unwrap_or("[state]").to_owned(),
        format_p0(value),
        format_p0(1.0 - value),
        ctx.state1_nation_with_article.as_deref().unwrap_or("[nation]").to_owned(),
        ctx.state2_nation_with_article.as_deref().unwrap_or("[nation]").to_owned(),
        format_big_or_small_number(duration, env),
        format_p0(value - 1.0),
        state1.unwrap_or("[nation]").to_owned(),
        state1.unwrap_or("[region]").to_owned(),
        state2.unwrap_or("[nation]").to_owned(),
        state2.unwrap_or("[region]").to_owned(),
        env.template_display_name(&str_value).unwrap_or_else(|| str_value.clone()),
        faction1.to_owned(),
        faction2.to_owned(),
        env.trait_names_in_grouping(value as i64).join(", "),
        ctx.state1_councilor_trait.as_deref().unwrap_or("[trait]").to_owned(),
        format_p0(1.0 / value - 1.0),
        format_big_or_small_number(-value, env),
        env.councilor_attribute_label(&str_value).unwrap_or(str_value),
        capitalize_visible(faction1),
        capitalize_visible(faction2),
        format_small_number(scaled),
        format_small_number(scaled),
        format_p0(value.abs()),
        ctx.state1_executive_nations.join(", "),
        format_p0(1.0 - 1.0 / value),
        format_signed_percent(value - 1.0),
        format_signed_percent(value),
    ]
}

fn capitalize_visible(s: &str) -> String {
    let mut in_tag = false;
    for (i, c) in s.char_indices() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag && c.is_alphabetic() => {
                // Keep neutral placeholders lowercase.
                if s.starts_with('[') {
                    break;
                }
                return format!("{}{}{}", &s[..i], c.to_uppercase(), &s[i + c.len_utf8()..]);
            }
            _ => {}
        }
    }
    s.to_owned()
}

/// Substitute .NET composite format items and doubled-brace escapes.
///
/// Supports `{index[,alignment][:format]}`, including ASCII spaces around the
/// alignment. Format strings are ignored because every argument is a String,
/// matching String.Format's treatment of non-IFormattable arguments. Alignment
/// counts UTF-16 code units, as .NET String.Length does. No numeric or alignment
/// specifiers occur in this build's English effect descriptions.
///
/// # Errors
/// Returns `Err(())` for malformed braces/items or an out-of-range index. The
/// caller can then return the localization key, matching LocalizationManager.Find.
#[allow(clippy::result_unit_err)] // Required API: callers only need key fallback.
pub fn format_dotnet(template: &str, args: &[String]) -> Result<String, ()> {
    let bytes = template.as_bytes();
    let mut out = String::with_capacity(template.len());
    let mut pos = 0;
    let mut literal = 0;
    while pos < bytes.len() {
        if bytes[pos] != b'{' && bytes[pos] != b'}' {
            pos += 1;
            continue;
        }
        out.push_str(&template[literal..pos]);
        let brace = bytes[pos];
        pos += 1;
        if bytes.get(pos) == Some(&brace) {
            out.push(char::from(brace));
            pos += 1;
        } else if brace == b'}' {
            return Err(());
        } else {
            let index = parse_decimal(bytes, &mut pos)?;
            let arg = args.get(index).ok_or(())?;
            skip_spaces(bytes, &mut pos);
            let mut width = 0;
            let mut left = false;
            if bytes.get(pos) == Some(&b',') {
                pos += 1;
                skip_spaces(bytes, &mut pos);
                if bytes.get(pos) == Some(&b'-') {
                    left = true;
                    pos += 1;
                }
                width = parse_decimal(bytes, &mut pos)?;
                skip_spaces(bytes, &mut pos);
            }
            if bytes.get(pos) == Some(&b':') {
                pos += 1;
                while let Some(&c) = bytes.get(pos) {
                    if c == b'}' {
                        break;
                    }
                    if c == b'{' {
                        return Err(());
                    }
                    pos += 1;
                }
            }
            if bytes.get(pos) != Some(&b'}') {
                return Err(());
            }
            pos += 1;
            // Value markers of describe_effect_segments don't count toward the width.
            let len = arg.encode_utf16().filter(|&u| u != VALUE_START as u16 && u != VALUE_END as u16).count();
            let padding = width.saturating_sub(len);
            if !left {
                out.extend(std::iter::repeat_n(' ', padding));
            }
            out.push_str(arg);
            if left {
                out.extend(std::iter::repeat_n(' ', padding));
            }
        }
        literal = pos;
    }
    out.push_str(&template[literal..]);
    Ok(out)
}

fn skip_spaces(bytes: &[u8], pos: &mut usize) {
    while bytes.get(*pos) == Some(&b' ') {
        *pos += 1;
    }
}

fn parse_decimal(bytes: &[u8], pos: &mut usize) -> Result<usize, ()> {
    let start = *pos;
    let mut n = 0usize;
    while let Some(c @ b'0'..=b'9') = bytes.get(*pos) {
        // String.Format stops consuming digits once the preceding value has
        // reached its 1M parsing limit; the last digit can take it above 1M.
        if n >= 1_000_000 {
            return Err(());
        }
        n = n.checked_mul(10).and_then(|n| n.checked_add(usize::from(c - b'0'))).ok_or(())?;
        *pos += 1;
    }
    if *pos == start { Err(()) } else { Ok(n) }
}

/// Build the 30 arguments and substitute them into a localized template.
///
/// # Errors
/// Returns `Err(())` under the same conditions as [`format_dotnet`].
#[allow(clippy::result_unit_err)] // Required public signature.
pub fn describe_effect(template: &str, effect: &Value, ctx: &EffectContext, env: &dyn EffectEnv) -> Result<String, ()> {
    format_dotnet(template, &effect_args(effect, ctx, env))
}

/// Argument slots holding numbers or percentages (the rest are names and lists).
pub const NUMERIC_SLOTS: [usize; 13] = [0, 3, 4, 7, 8, 18, 19, 23, 24, 25, 27, 28, 29];

const VALUE_START: char = '\u{E000}';
const VALUE_END: char = '\u{E001}';

/// A run of a rendered effect description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectSegment {
    Text(String),
    /// A substituted number or percentage (one of [`NUMERIC_SLOTS`]).
    Value(String),
    /// Inline sprite by name (`<sprite name="metal">` → `metal`), drawn by the game in place of a word.
    Sprite(String),
}

/// Like [`describe_effect`], but keeps the substituted numbers and inline sprites apart
/// and drops markup. The game's leading list dash and surrounding whitespace are removed.
///
/// # Errors
/// Returns `Err(())` under the same conditions as [`format_dotnet`].
#[allow(clippy::result_unit_err)] // Same contract as describe_effect.
pub fn describe_effect_segments(template: &str, effect: &Value, ctx: &EffectContext, env: &dyn EffectEnv) -> Result<Vec<EffectSegment>, ()> {
    let mut args = effect_args(effect, ctx, env);
    for &slot in &NUMERIC_SLOTS {
        args[slot] = format!("{VALUE_START}{}{VALUE_END}", args[slot]);
    }
    let formatted = format_dotnet(template, &args)?;
    let mut out: Vec<EffectSegment> = Vec::new();
    let push_text = |out: &mut Vec<EffectSegment>, text: &str| match out.last_mut() {
        Some(EffectSegment::Text(prev)) => prev.push_str(text),
        _ if !text.is_empty() => out.push(EffectSegment::Text(text.to_owned())),
        _ => {}
    };
    for span in crate::text::parse_rich(&formatted) {
        match span {
            crate::text::Span::Text { text, .. } => {
                let mut rest = text.as_str();
                while let Some(start) = rest.find(VALUE_START) {
                    push_text(&mut out, &rest[..start]);
                    let after = &rest[start + VALUE_START.len_utf8()..];
                    let end = after.find(VALUE_END).unwrap_or(after.len());
                    out.push(EffectSegment::Value(after[..end].to_owned()));
                    rest = after.get(end + VALUE_END.len_utf8()..).unwrap_or("");
                }
                push_text(&mut out, rest);
            }
            crate::text::Span::LineBreak => push_text(&mut out, "\n"),
            crate::text::Span::Placeholder(p) => push_text(&mut out, &format!("{{{p}}}")),
            crate::text::Span::Sprite(tag) => out.push(EffectSegment::Sprite(sprite_name(&tag))),
            crate::text::Span::Align(_) => {}
        }
    }
    if let Some(EffectSegment::Text(first)) = out.first_mut() {
        *first = first.trim_start().trim_start_matches('-').trim_start().to_owned();
    }
    if let Some(EffectSegment::Text(last)) = out.last_mut() {
        last.truncate(last.trim_end().len());
    }
    out.retain(|s| !matches!(s, EffectSegment::Text(t) if t.is_empty()));
    Ok(out)
}

/// `sprite name="metal"` / `sprite="metal"` → `metal`.
fn sprite_name(tag: &str) -> String {
    let value = tag.split_once('=').map_or(tag, |(_, v)| v);
    let value = value.rsplit_once("name=").map_or(value, |(_, v)| v);
    value.split_whitespace().next().unwrap_or("").trim_matches('"').to_owned()
}

/// FormatSmallNumber: truncate to seven decimals, choose N precision, then trim
/// fractional zeros. Integers use N0, nonintegers ≥1 use N1, and successively
/// smaller magnitudes use 2–7 decimals with strict >0.1, >0.01, … thresholds.
pub fn format_small_number(v: f64) -> String {
    if !v.is_finite() {
        return special_number(v).to_owned();
    }
    let multiplied = v * 10_000_000.0;
    let v = if multiplied.is_finite() { multiplied.trunc() / 10_000_000.0 } else { v };
    let abs = v.abs();
    let digits = if v.fract() == 0.0 {
        0
    } else if abs >= 1.0 {
        1
    } else if abs > 0.1 {
        2
    } else if abs > 0.01 {
        3
    } else if abs > 0.001 {
        4
    } else if abs > 0.0001 {
        5
    } else if abs > 0.00001 {
        6
    } else {
        7
    };
    let text = format_n(v, digits);
    if digits == 0 {
        text
    } else {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// FormatBigOrSmallNumber, using localized K/M/B/T templates and N1 mantissas.
/// Missing or malformed suffix localization returns its raw key, like Loc.T.
pub fn format_big_or_small_number(v: f64, env: &dyn EffectEnv) -> String {
    if !v.is_finite() || v.abs() < 1_000.0 {
        return format_small_number(v);
    }
    let (divisor, key) = if v.abs() >= 1e12 {
        (1e12, "UI.Global.Trillions")
    } else if v.abs() >= 1e9 {
        (1e9, "UI.Global.Billions")
    } else if v.abs() >= 1e6 {
        (1e6, "UI.Global.Millions")
    } else {
        (1e3, "UI.Global.Thousands")
    };
    env.loc(key)
        .and_then(|template| format_dotnet(&template, &[format_n(v / divisor, 1)]).ok())
        .unwrap_or_else(|| key.to_owned())
}

/// ToString("P0") in English, with the percent-space removed. Uses grouped
/// digits and the game's midpoint-away-from-zero rounding; nonfinite values
/// retain the .NET NaN/Infinity symbols without a percent sign.
pub fn format_p0(v: f64) -> String {
    if !v.is_finite() {
        return special_number(v).to_owned();
    }
    format!("{}%", group_fixed(&rounded_fixed(v, 0, 2)))
}

/// ToString("+0%;-0%;0%"). The third section applies after rounding, so both
/// 0.001 and -0.001 yield "0%", without a sign. Custom 0 formats do not group.
/// See Microsoft's custom numeric format strings, section separator rules:
/// <https://learn.microsoft.com/en-us/dotnet/standard/base-types/custom-numeric-format-strings#the--section-separator>.
pub fn format_signed_percent(v: f64) -> String {
    if !v.is_finite() {
        return special_number(v).to_owned();
    }
    let rounded = rounded_fixed(v, 0, 2);
    if rounded == "0" {
        "0%".to_owned()
    } else if !rounded.starts_with('-') {
        format!("+{rounded}%")
    } else {
        format!("{rounded}%")
    }
}

fn special_number(v: f64) -> &'static str {
    if v.is_nan() {
        "NaN"
    } else if v.is_sign_negative() {
        "-Infinity"
    } else {
        "Infinity"
    }
}

/// English N format with explicit away-from-zero rounding, rather than Rust's
/// ties-to-even float formatting.
fn format_n(v: f64, digits: usize) -> String {
    if !v.is_finite() {
        return special_number(v).to_owned();
    }
    group_fixed(&rounded_fixed(v, digits, 0))
}

/// Round the decimal representation, then insert the decimal point. Moving the
/// point for percent formats avoids binary multiplication artifacts such as
/// 0.145 * 100 = 14.499999999999998, as well as overflow for large percentages.
/// Rust's Display gives finite floats in decimal notation without an exponent.
/// Rounded negative zero is normalized for the game's Unity/Mono-era output.
fn rounded_fixed(v: f64, places: usize, decimal_shift: usize) -> String {
    let source = v.abs().to_string();
    let integer_len = source.find('.').unwrap_or(source.len());
    let mut digits: Vec<u8> = source.bytes().filter(|&c| c != b'.').collect();
    let keep = integer_len + decimal_shift + places;
    digits.resize(digits.len().max(keep + 1), b'0');
    let round_up = digits[keep] >= b'5';
    digits.truncate(keep);
    if round_up {
        let mut carry = true;
        for digit in digits.iter_mut().rev() {
            if *digit != b'9' {
                *digit += 1;
                carry = false;
                break;
            }
            *digit = b'0';
        }
        if carry {
            digits.insert(0, b'1');
        }
    }
    let nonzero = digits.iter().any(|&c| c != b'0');
    let point = digits.len() - places;
    let mut out = String::with_capacity(digits.len() + 2);
    if v.is_sign_negative() && nonzero {
        out.push('-');
    }
    let mut first_nonzero = digits[..point].iter().position(|&c| c != b'0').unwrap_or(point - 1);
    while first_nonzero < digits.len() {
        if places != 0 && first_nonzero == point {
            out.push('.');
        }
        out.push(char::from(digits[first_nonzero]));
        first_nonzero += 1;
    }
    out
}

fn group_fixed(fixed: &str) -> String {
    let (sign, unsigned) = fixed.strip_prefix('-').map_or(("", fixed), |s| ("-", s));
    let (integer, fractional) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let mut result = String::with_capacity(fixed.len() + integer.len() / 3);
    result.push_str(sign);
    for (i, c) in integer.chars().enumerate() {
        if i != 0 && (integer.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    if !fractional.is_empty() {
        result.push('.');
        result.push_str(fractional);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    #[test]
    fn segments_keep_alignment() {
        let env = TestEnv::english();
        let segs = |t: &str| describe_effect_segments(t, &json!({"value": 5}), &EffectContext::default(), &env).unwrap();
        use EffectSegment::*;
        assert_eq!(segs("[{0,5}]"), vec![Text("[    ".into()), Value("5".into()), Text("]".into())]);
        assert_eq!(segs("[{0,-5}]"), vec![Text("[".into()), Value("5".into()), Text("    ]".into())]);
    }

    #[test]
    fn segments_mark_values_and_sprites() {
        let env = TestEnv::english();
        let effect = json!({"value": 0.25});
        let segs = describe_effect_segments(
            "-Our mines produce {3} more <color=#FFFFFFFF><sprite name=\"metal\"></color> and {0}<br/>x",
            &effect,
            &EffectContext::default(),
            &env,
        )
        .unwrap();
        use EffectSegment::*;
        assert_eq!(
            segs,
            vec![
                Text("Our mines produce ".into()),
                Value("25%".into()),
                Text(" more ".into()),
                Sprite("metal".into()),
                Text(" and ".into()),
                Value("0.25".into()),
                Text("\nx".into()),
            ]
        );
    }

    #[derive(Default)]
    struct TestEnv {
        strings: BTreeMap<String, String>,
        names: BTreeMap<String, String>,
        traits: BTreeMap<i64, Vec<String>>,
    }

    impl TestEnv {
        fn english() -> Self {
            Self {
                strings: [
                    ("UI.Global.Thousands", "{0}K"),
                    ("UI.Global.Millions", "{0}M"),
                    ("UI.Global.Billions", "{0}B"),
                    ("UI.Global.Trillions", "{0}T"),
                    ("UI.Global.Persuasion", "Persuasion"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
                names: [("Persuasion".to_owned(), "A template".to_owned())].into(),
                traits: [(0, vec!["First trait".to_owned(), "Second trait".to_owned()])].into(),
            }
        }
    }

    impl EffectEnv for TestEnv {
        fn loc(&self, key: &str) -> Option<String> {
            self.strings.get(key).cloned()
        }
        fn template_display_name(&self, data_name: &str) -> Option<String> {
            self.names.get(data_name).cloned()
        }
        fn trait_names_in_grouping(&self, grouping: i64) -> Vec<String> {
            self.traits.get(&grouping).cloned().unwrap_or_default()
        }
        fn councilor_attribute_label(&self, attribute: &str) -> Option<String> {
            self.loc(&format!("UI.Global.{attribute}"))
        }
    }

    #[test]
    fn small_numbers_truncate_select_precision_round_and_trim() {
        for (v, expected) in [
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.0, "-1"),
            (1.25, "1.3"),
            (-1.25, "-1.3"),
            (2.25, "2.3"),
            (1.24999999, "1.2"),
            (-1.24999999, "-1.2"),
            (0.125, "0.13"),
            (-0.125, "-0.13"),
            (0.0125, "0.013"),
            (0.00125, "0.0013"),
            (0.000125, "0.00013"),
            (0.0000125, "0.000013"),
            (0.00000125, "0.0000012"), // truncated to seven places first
            (0.00000019, "0.0000001"),
            (-0.00000019, "-0.0000001"),
            (0.00000009, "0"),
            (-0.00000009, "0"),
            (0.1, "0.1"),
            (0.01, "0.01"),
            (0.001, "0.001"),
            (0.0001, "0.0001"),
            (0.00001, "0.00001"),
            (0.1000001, "0.1"),
            (0.99999999, "1"),
            (999.95, "1,000"),
            (1_234.5, "1,234.5"),
            (-1_234.5, "-1,234.5"),
            (1_000_000.0, "1,000,000"),
            (1_000_000_000.0, "1,000,000,000"),
        ] {
            assert_eq!(format_small_number(v), expected, "value {v}");
        }
    }

    #[test]
    fn n_format_groups_digits_and_rounds_midpoints_away_from_zero() {
        for (v, digits, expected) in [
            (0.0, 1, "0.0"),
            (-0.001, 1, "0.0"),
            (2.5, 0, "3"),
            (-2.5, 0, "-3"),
            (1_234.45, 1, "1,234.5"),
            (-1_234.45, 1, "-1,234.5"),
            (999_999.5, 0, "1,000,000"),
        ] {
            assert_eq!(format_n(v, digits), expected);
        }
    }

    #[test]
    fn large_numbers_use_localized_suffixes_with_one_decimal() {
        let env = TestEnv::english();
        for (v, expected) in [
            (0.0, "0"),
            (-0.125, "-0.13"),
            (1.25, "1.3"),
            (999.0, "999"),
            (1_000.0, "1.0K"),
            (-1_000.0, "-1.0K"),
            (1_250.0, "1.3K"),
            (-1_250.0, "-1.3K"),
            (999_999.0, "1,000.0K"),
            (1_000_000.0, "1.0M"),
            (2_250_000.0, "2.3M"),
            (-2_250_000.0, "-2.3M"),
            (1_000_000_000.0, "1.0B"),
            (2_250_000_000.0, "2.3B"),
            (-2_250_000_000.0, "-2.3B"),
            (1_000_000_000_000.0, "1.0T"),
            (-2_250_000_000_000.0, "-2.3T"),
            (1_000_000_000_000_000.0, "1,000.0T"),
        ] {
            assert_eq!(format_big_or_small_number(v, &env), expected, "value {v}");
        }
        let mut translated = TestEnv::english();
        translated.strings.insert("UI.Global.Thousands".to_owned(), "[{0}] thousand".to_owned());
        assert_eq!(format_big_or_small_number(1_000.0, &translated), "[1.0] thousand");
        translated.strings.insert("UI.Global.Thousands".to_owned(), "{word}".to_owned());
        assert_eq!(format_big_or_small_number(1_000.0, &translated), "UI.Global.Thousands");
        assert_eq!(format_big_or_small_number(1_000.0, &TestEnv::default()), "UI.Global.Thousands");
    }

    #[test]
    fn p0_scales_groups_and_rounds() {
        for (v, expected) in [
            (0.0, "0%"),
            (-0.0, "0%"),
            (0.25, "25%"),
            (-0.25, "-25%"),
            (0.125, "13%"),
            (-0.125, "-13%"),
            (0.145, "15%"),
            (-0.145, "-15%"),
            (0.1449999999, "14%"),
            (1.005, "101%"),
            (-1.005, "-101%"),
            (0.005, "1%"),
            (-0.005, "-1%"),
            (0.001, "0%"),
            (-0.001, "0%"),
            (12.345, "1,235%"),
            (-12.345, "-1,235%"),
            (1_000.0, "100,000%"),
            (1_000_000.0, "100,000,000%"),
            (1_000_000_000.0, "100,000,000,000%"),
        ] {
            assert_eq!(format_p0(v), expected, "value {v}");
        }
    }

    #[test]
    fn signed_percent_selects_the_zero_section_after_rounding() {
        for (v, expected) in [
            (0.0, "0%"),
            (-0.0, "0%"),
            (0.25, "+25%"),
            (-0.25, "-25%"),
            (0.125, "+13%"),
            (-0.125, "-13%"),
            (0.145, "+15%"),
            (-0.145, "-15%"),
            (0.1449999999, "+14%"),
            (1.005, "+101%"),
            (-1.005, "-101%"),
            (0.005, "+1%"),
            (-0.005, "-1%"),
            (0.001, "0%"),
            (-0.001, "0%"),
            (0.004999999, "0%"),
            (-0.004999999, "0%"),
            (12.345, "+1235%"),
            (-12.345, "-1235%"),
            (1_000.0, "+100000%"),
            (1_000_000.0, "+100000000%"),
            (1_000_000_000.0, "+100000000000%"),
        ] {
            assert_eq!(format_signed_percent(v), expected, "value {v}");
        }
    }

    #[test]
    fn nonfinite_numbers_do_not_panic_or_acquire_suffixes() {
        let env = TestEnv::english();
        for (v, expected) in [(f64::NAN, "NaN"), (f64::INFINITY, "Infinity"), (f64::NEG_INFINITY, "-Infinity")] {
            assert_eq!(format_small_number(v), expected);
            assert_eq!(format_big_or_small_number(v, &env), expected);
            assert_eq!(format_p0(v), expected);
            assert_eq!(format_signed_percent(v), expected);
        }
        assert!(format_small_number(f64::MAX).starts_with("179,"));
        assert!(!format_p0(f64::MAX).contains("Infinity"));
        assert!(!format_signed_percent(f64::MAX).contains("inf"));
    }

    #[test]
    fn composite_format_substitutes_escapes_and_does_not_reparse_arguments() {
        let args = vec!["first".to_owned(), "{2}".to_owned(), "é🚀".to_owned()];
        for (template, expected) in [
            ("", ""),
            ("literal é🚀", "literal é🚀"),
            ("{1} {0} {0}", "{2} first first"),
            ("{{{0}}}", "{first}"),
            ("{{}}", "{}"),
            ("{{word}}", "{word}"),
            ("{{{{}}}}", "{{}}"),
            ("[{0,7}] [{0,-7}]", "[  first] [first  ]"),
            ("{0, 7 :N1}", "  first"),
            ("{0:bogus}", "first"),
            ("{0:}", "first"),
            ("{{{0:N1}}}", "{first}"),
            ("[{2,4}]", "[ é🚀]"), // three UTF-16 units, not two chars
            ("[{0,2}]", "[first]"),
            ("{00}", "first"),
        ] {
            assert_eq!(format_dotnet(template, &args), Ok(expected.to_owned()), "{template}");
        }
        let thirty: Vec<String> = (0..30).map(|n| n.to_string()).collect();
        assert_eq!(format_dotnet("{10} {29}", &thirty), Ok("10 29".to_owned()));
        assert_eq!(format_dotnet("{{0}}", &[]), Ok("{0}".to_owned()));
    }

    #[test]
    fn malformed_composite_formats_return_errors() {
        let args = ["x".to_owned()];
        for template in [
            "{1}",
            "{29}",
            "{word}",
            "{",
            "}",
            "{0",
            "{0}}",
            "{{{",
            "}}}",
            "{}",
            "{ 0}",
            "{-1}",
            "{+0}",
            "{0,x}",
            "{0,}",
            "{0,-}",
            "{0,+2}",
            "{0,- 2}",
            "{0,2x}",
            "{0\t}",
            "{0:{word}}",
            "{0:unclosed",
            "{999999999999999999999999999999}",
            "{0,999999999999999999999999999999}",
        ] {
            assert_eq!(format_dotnet(template, &args), Err(()), "{template}");
        }
        assert_eq!(format_dotnet("{0}", &[]), Err(()));
    }

    #[test]
    fn every_argument_uses_its_documented_slot() {
        let env = TestEnv::english();
        let ctx = EffectContext {
            state1_name: Some("Acting state".to_owned()),
            state2_name: Some("Target state".to_owned()),
            state1_faction: Some("the resistance".to_owned()),
            state2_faction: Some("<color=#fff>the academy</color>".to_owned()),
            state1_nation_with_article: Some("in the United States".to_owned()),
            state2_nation_with_article: Some("in France".to_owned()),
            state1_councilor_trait: Some("First trait".to_owned()),
            state1_executive_nations: vec!["France".to_owned(), "Germany".to_owned()],
            pop_scaling: Some(3.0),
        };
        let effect = json!({"value": 0.25, "duration_months": 12.5, "strValue": "Persuasion"});
        let expected = [
            "0.25",
            "Acting state",
            "Target state",
            "25%",
            "75%",
            "in the United States",
            "in France",
            "12.5",
            "-75%",
            "Acting state",
            "Acting state",
            "Target state",
            "Target state",
            "A template",
            "the resistance",
            "<color=#fff>the academy</color>",
            "First trait, Second trait",
            "First trait",
            "300%",
            "-0.25",
            "Persuasion",
            "The resistance",
            "<color=#fff>The academy</color>",
            "0.5",
            "0.5",
            "25%",
            "France, Germany",
            "-300%",
            "-75%",
            "+25%",
        ];
        let args = effect_args(&effect, &ctx, &env);
        assert_eq!(args.len(), 30);
        for (slot, expected) in expected.iter().enumerate() {
            assert_eq!(args[slot], *expected, "slot {slot}");
        }
        assert_eq!(
            describe_effect("{21}: {0}, {3}", &effect, &ctx, &env),
            Ok("The resistance: 0.25, 25%".to_owned())
        );
        assert_eq!(describe_effect("{word}", &effect, &ctx, &env), Err(()));
    }

    #[test]
    fn absent_context_and_json_defaults_are_neutral() {
        let env = TestEnv::english();
        let ctx = EffectContext::default();
        let args = effect_args(&json!({}), &ctx, &env);
        for (slot, expected) in [
            (0, "0"),
            (1, "[state]"),
            (2, "[state]"),
            (5, "[nation]"),
            (6, "[nation]"),
            (7, "0"),
            (9, "[nation]"),
            (10, "[region]"),
            (11, "[nation]"),
            (12, "[region]"),
            (13, ""),
            (14, "[faction]"),
            (15, "[faction]"),
            (17, "[trait]"),
            (18, "Infinity"),
            (20, ""),
            (21, "[faction]"),
            (22, "[faction]"),
            (23, "0"),
            (24, "0"),
            (26, ""),
            (27, "-Infinity"),
        ] {
            assert_eq!(args[slot], expected, "slot {slot}");
        }
        let fallback = EffectContext {
            state1_name: Some("the state".to_owned()),
            ..ctx
        };
        let args = effect_args(&json!({"value": -2, "duration_months": 6, "strValue": 3.5}), &fallback, &env);
        assert_eq!(args[7], "6");
        assert_eq!(args[13], "3.5");
        assert_eq!(args[14], "the state");
        assert_eq!(args[20], "3.5");
        assert_eq!(args[21], "The state");
        assert_eq!(args[23], "-2");
        assert_eq!(args[24], "-2");
        assert_eq!(effect_args(&Value::Null, &EffectContext::default(), &env).len(), 30);
    }

    #[test]
    fn population_scaling_clamps_both_signs_and_zero_without_panicking() {
        let env = TestEnv::english();
        for (value, scale, expected) in [(2.0, 10.0, "4"), (-2.0, 10.0, "-4"), (2.0, -10.0, "-4"), (0.0, 10.0, "0"), (2.0, 0.25, "0.5")] {
            let ctx = EffectContext {
                pop_scaling: Some(scale),
                ..EffectContext::default()
            };
            let args = effect_args(&json!({"value": value}), &ctx, &env);
            assert_eq!(args[23], expected);
            assert_eq!(args[24], expected);
        }
        let ctx = EffectContext {
            pop_scaling: Some(f64::NAN),
            ..EffectContext::default()
        };
        assert_eq!(effect_args(&json!({"value": 1}), &ctx, &env)[23], "NaN");
    }
}
