//! Log format profiles — auto-detect and per-format normalization.
//!
//! Profiles: journalctl, docker, uvicorn, nodejs, plain (fallback).

use regex::Regex;
use std::sync::OnceLock;

// ─── Journalctl ───────────────────────────────────────────────────────────────

static JOURNAL_CLASSIC: OnceLock<Regex> = OnceLock::new();
static JOURNAL_ISO: OnceLock<Regex> = OnceLock::new();
static JOURNAL_PID: OnceLock<Regex> = OnceLock::new();

fn journal_classic() -> &'static Regex {
    JOURNAL_CLASSIC.get_or_init(|| {
        Regex::new(
            r"^[A-Z][a-z]{2}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}\s+\S+\s+[^\[:\s]+(?:\[\d+\])?:\s+",
        )
        .unwrap()
    })
}

fn journal_iso() -> &'static Regex {
    JOURNAL_ISO.get_or_init(|| {
        Regex::new(
            r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[.,]\d+[+\-Z][\d:]*\s+\S+\s+[^\[:\s]+(?:\[\d+\])?:\s+",
        )
        .unwrap()
    })
}

fn journal_pid() -> &'static Regex {
    JOURNAL_PID.get_or_init(|| Regex::new(r"\[(\d+)\]:").unwrap())
}

// ─── Docker ───────────────────────────────────────────────────────────────────

static DOCKER_JSON: OnceLock<Regex> = OnceLock::new();
static DOCKER_COMPOSE: OnceLock<Regex> = OnceLock::new();

fn docker_json() -> &'static Regex {
    DOCKER_JSON.get_or_init(|| Regex::new(r#"^\{"log":"#).unwrap())
}

fn docker_compose() -> &'static Regex {
    DOCKER_COMPOSE.get_or_init(|| Regex::new(r"^[\w_-]+\s+\|\s+").unwrap())
}

// ─── Uvicorn ──────────────────────────────────────────────────────────────────

static UVICORN_PORT: OnceLock<Regex> = OnceLock::new();
static UVICORN_DETECT: OnceLock<Regex> = OnceLock::new();

fn uvicorn_port() -> &'static Regex {
    UVICORN_PORT.get_or_init(|| Regex::new(r"(\d{1,3}(?:\.\d{1,3}){3}):\d{4,5}").unwrap())
}

fn uvicorn_detect() -> &'static Regex {
    UVICORN_DETECT.get_or_init(|| {
        Regex::new(
            r#"(?:INFO\s*:\s*)?(?:\d{1,3}(?:\.\d{1,3}){3})(?::\d+)?\s+-\s+"[A-Z]+ \S+ HTTP/"#,
        )
        .unwrap()
    })
}

// ─── Node.js / Pino ──────────────────────────────────────────────────────────

static PINO_JSON: OnceLock<Regex> = OnceLock::new();
static WINSTON: OnceLock<Regex> = OnceLock::new();

fn pino_json() -> &'static Regex {
    PINO_JSON.get_or_init(|| Regex::new(r#"^\{"level":\s*\d+,\s*"time":\s*\d+"#).unwrap())
}

fn winston() -> &'static Regex {
    WINSTON.get_or_init(|| Regex::new(r"^\[\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\]").unwrap())
}

// ─── Profile enum ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Profile {
    Journalctl,
    Docker,
    Uvicorn,
    Nodejs,
    Plain,
}

impl Profile {
    pub fn name(&self) -> &'static str {
        match self {
            Profile::Journalctl => "journalctl",
            Profile::Docker => "docker",
            Profile::Uvicorn => "uvicorn",
            Profile::Nodejs => "nodejs",
            Profile::Plain => "plain",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "journalctl" => Some(Profile::Journalctl),
            "docker" => Some(Profile::Docker),
            "uvicorn" => Some(Profile::Uvicorn),
            "nodejs" => Some(Profile::Nodejs),
            "plain" => Some(Profile::Plain),
            _ => None,
        }
    }

    fn detect_score(&self, lines: &[&str]) -> usize {
        let sample = lines.iter().take(20).count();
        if sample == 0 {
            return 0;
        }
        let hits = match self {
            Profile::Journalctl => lines
                .iter()
                .take(20)
                .filter(|l| journal_classic().is_match(l) || journal_iso().is_match(l))
                .count(),
            Profile::Docker => lines
                .iter()
                .take(20)
                .filter(|l| docker_json().is_match(l) || docker_compose().is_match(l))
                .count(),
            Profile::Uvicorn => lines
                .iter()
                .take(20)
                .filter(|l| l.contains("uvicorn") || uvicorn_detect().is_match(l))
                .count(),
            Profile::Nodejs => lines
                .iter()
                .take(20)
                .filter(|l| pino_json().is_match(l) || winston().is_match(l))
                .count(),
            Profile::Plain => return sample, // always matches
        };
        hits
    }

    fn threshold(&self) -> f64 {
        match self {
            Profile::Journalctl => 0.5,
            Profile::Docker => 0.4,
            Profile::Uvicorn => 0.3,
            Profile::Nodejs => 0.4,
            Profile::Plain => 0.0,
        }
    }

    pub fn normalize_line(&self, line: &str) -> String {
        match self {
            Profile::Journalctl => journal_pid().replace_all(line, "[PID]:").into_owned(),
            Profile::Docker => {
                if docker_json().is_match(line) {
                    // Minimal JSON parse: extract "log" field value
                    if let Some(val) = extract_docker_log_field(line) {
                        return val.trim_end_matches('\n').to_string();
                    }
                }
                if let Some(m) = docker_compose().find(line) {
                    return line[m.end()..].to_string();
                }
                line.to_string()
            }
            Profile::Uvicorn => uvicorn_port()
                .replace_all(line, |caps: &regex::Captures| caps[1].to_string())
                .into_owned(),
            Profile::Nodejs => {
                if pino_json().is_match(line) {
                    if let Some(normalized) = normalize_pino_line(line) {
                        return normalized;
                    }
                }
                line.to_string()
            }
            Profile::Plain => line.to_string(),
        }
    }
}

/// Auto-detect profile from first lines of text.
pub fn auto_detect(text: &str) -> Profile {
    let lines: Vec<&str> = text.lines().take(40).collect();
    let sample = lines.len().max(1);

    for profile in [
        Profile::Journalctl,
        Profile::Docker,
        Profile::Uvicorn,
        Profile::Nodejs,
    ] {
        let score = profile.detect_score(&lines);
        let ratio = score as f64 / sample as f64;
        if ratio >= profile.threshold() {
            return profile;
        }
    }
    Profile::Plain
}

/// Apply profile normalization line-by-line.
pub fn apply_profile(text: &str, profile: &Profile) -> String {
    text.lines()
        .map(|line| profile.normalize_line(line))
        .collect::<Vec<_>>()
        .join("\n")
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Minimal extraction of "log" field from docker JSON without pulling serde.
fn extract_docker_log_field(line: &str) -> Option<String> {
    // {"log":"actual message\n",...}
    let key = r#""log":""#;
    let start = line.find(key)? + key.len();
    let rest = &line[start..];
    // Find closing quote, handling escaped chars
    let mut end = 0;
    let mut chars = rest.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            chars.next(); // skip escaped char
            continue;
        }
        if c == '"' {
            end = i;
            break;
        }
    }
    if end == 0 && !rest.is_empty() {
        return None;
    }
    Some(rest[..end].replace("\\n", "\n").replace("\\t", "\t").replace("\\\"", "\""))
}

/// Normalize pino JSON line to readable format.
fn normalize_pino_line(line: &str) -> Option<String> {
    // Simple key extraction without serde — avoids extra dep
    let level_raw = extract_json_number(line, "level")?;
    let msg = extract_json_string(line, "msg").unwrap_or_default();
    let level_name = match level_raw {
        10 => "TRACE",
        20 => "DEBUG",
        30 => "INFO",
        40 => "WARN",
        50 => "ERROR",
        60 => "FATAL",
        _ => "UNKNOWN",
    };
    Some(format!("{level_name} {msg}"))
}

fn extract_json_number(json: &str, key: &str) -> Option<u64> {
    let search = format!(r#""{key}":"#);
    let pos = json.find(&search)? + search.len();
    let rest = json[pos..].trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let search = format!(r#""{key}":""#);
    let pos = json.find(&search)? + search.len();
    let rest = &json[pos..];
    let mut out = String::new();
    let mut chars = rest.chars();
    loop {
        match chars.next()? {
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                '"' => out.push('"'),
                c => out.push(c),
            },
            '"' => break,
            c => out.push(c),
        }
    }
    Some(out)
}
