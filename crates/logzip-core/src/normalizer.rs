//! Log text normalizer.
//!
//! Убирает «мусор» уникальный в каждой строке (ANSI, наносекунды, leading hex zeros)
//! и выносит общий префикс строк.

use regex::Regex;
use std::sync::OnceLock;

static ANSI_RE: OnceLock<Regex> = OnceLock::new();
static NANOS_RE: OnceLock<Regex> = OnceLock::new();
static HEX_RE: OnceLock<Regex> = OnceLock::new();
static WS_RE: OnceLock<Regex> = OnceLock::new();

fn ansi_re() -> &'static Regex {
    ANSI_RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap())
}

fn nanos_re() -> &'static Regex {
    NANOS_RE.get_or_init(|| Regex::new(r"(\.\d{3})\d{3,6}(Z|[+\-]\d{2}:?\d{2})?").unwrap())
}

fn hex_re() -> &'static Regex {
    HEX_RE.get_or_init(|| Regex::new(r"\b(0x)?(0{2,})([0-9A-Fa-f]+)\b").unwrap())
}

fn ws_re() -> &'static Regex {
    WS_RE.get_or_init(|| Regex::new(r"[ \t]{2,}").unwrap())
}

fn strip_ansi(text: &str) -> String {
    ansi_re().replace_all(text, "").into_owned()
}

fn trim_subsecond(text: &str) -> String {
    nanos_re()
        .replace_all(text, |caps: &regex::Captures| {
            let ms = &caps[1];
            let tz = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            format!("{ms}{tz}")
        })
        .into_owned()
}

fn strip_hex_leading_zeros(text: &str) -> String {
    hex_re()
        .replace_all(text, |caps: &regex::Captures| {
            let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let tail = &caps[3];
            let has_alpha = tail.chars().any(|c| c.is_ascii_alphabetic());
            if tail.len() >= 4 && (has_alpha || tail.len() > 7) {
                format!("{prefix}{tail}")
            } else {
                caps[0].to_string()
            }
        })
        .into_owned()
}

fn collapse_whitespace(text: &str) -> String {
    ws_re().replace_all(text, " ").into_owned()
}

/// Detect common prefix shared by all non-empty lines (min_len chars).
pub fn detect_common_prefix(lines: &[&str], min_len: usize) -> String {
    let non_empty: Vec<&str> = lines.iter().copied().filter(|l| !l.is_empty()).collect();
    if non_empty.len() < 2 {
        return String::new();
    }
    let mut prefix = non_empty[0];
    for line in &non_empty[1..] {
        let common_len = prefix
            .chars()
            .zip(line.chars())
            .take_while(|(a, b)| a == b)
            .count();
        // chars().count() is O(n), use byte offset for speed
        let byte_end = prefix
            .char_indices()
            .nth(common_len)
            .map(|(i, _)| i)
            .unwrap_or(prefix.len());
        prefix = &prefix[..byte_end];
        if prefix.len() < min_len {
            return String::new();
        }
    }
    // Trim to last meaningful separator
    for sep in [' ', 'T', ':', '-'] {
        if let Some(idx) = prefix.rfind(sep) {
            if idx + 1 >= min_len {
                return prefix[..idx + 1].to_string();
            }
        }
    }
    if prefix.len() >= min_len {
        prefix.to_string()
    } else {
        String::new()
    }
}

pub struct NormalizeResult {
    pub text: String,
    pub common_prefix: String,
}

/// Apply normalization pipeline + extract common prefix.
pub fn normalize(text: &str, extract_prefix: bool) -> NormalizeResult {
    let mut out = strip_ansi(text);
    out = trim_subsecond(&out);
    out = strip_hex_leading_zeros(&out);
    out = collapse_whitespace(&out);

    let mut common_prefix = String::new();
    if extract_prefix {
        let lines: Vec<&str> = out.lines().collect();
        common_prefix = detect_common_prefix(&lines, 8);
        if !common_prefix.is_empty() {
            let plen = common_prefix.len();
            out = lines
                .iter()
                .map(|l| {
                    if l.starts_with(&common_prefix) {
                        &l[plen..]
                    } else {
                        l
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
    }

    NormalizeResult {
        text: out,
        common_prefix,
    }
}
