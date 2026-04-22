//! Main compression/decompression algorithm.

use crate::{legend, normalizer, profiles, templates};
use std::collections::HashMap;

pub struct CompressResult {
    pub body: String,
    pub legend: Vec<legend::LegendEntry>,
    pub templates: Vec<templates::Template>,
    pub common_prefix: String,
    pub detected_profile: String,
    pub stats: HashMap<String, String>,
}

pub fn compress(
    text: &str,
    max_ngram: usize,
    max_legend_entries: usize,
    do_normalize: bool,
    profile: Option<&str>,
    do_templates: bool,
    bpe_passes: usize,
) -> CompressResult {
    let original_len = text.len();

    // 1. Profile
    let prof = if let Some(name) = profile {
        profiles::Profile::from_name(name).unwrap_or_else(|| profiles::auto_detect(text))
    } else {
        profiles::auto_detect(text)
    };
    let detected_profile = prof.name().to_string();
    let mut working = profiles::apply_profile(text, &prof);

    // 2. Normalize
    let mut common_prefix = String::new();
    if do_normalize {
        let norm = normalizer::normalize(&working, true);
        working = norm.text;
        common_prefix = norm.common_prefix;
    }

    // 3+4. Select legend entries + collect chosen positions (O(N) NFA scan)
    let (legend, chosen_positions) =
        legend::select_legend_with_positions(&working, max_legend_entries, max_ngram, 0);

    // 5. Direct substitution from known positions — no second AhoCorasick scan
    let mut body_working =
        legend::apply_legend_from_positions(&working, &legend, &chosen_positions);
    let mut all_legend = legend;
    let mut passes_used = 1usize;

    // 5b. Meta-passes (recursive BPE)
    for _ in 1..bpe_passes {
        if body_working.len() <= 256 {
            break;
        }
        let tag_offset = all_legend.len();
        let (meta_legend, meta_positions) = legend::select_legend_with_positions(
            &body_working,
            max_legend_entries,
            max_ngram,
            tag_offset,
        );
        if meta_legend.is_empty() {
            break;
        }
        let meta_body =
            legend::apply_legend_from_positions(&body_working, &meta_legend, &meta_positions);
        // Guard: skip if savings < 5%
        if meta_body.len() * 20 > body_working.len() * 19 {
            break;
        }
        all_legend.extend(meta_legend);
        body_working = meta_body;
        passes_used += 1;
    }

    // 6. Templates
    let (body, tmpl_list) = if do_templates {
        let lines: Vec<&str> = body_working.lines().collect();
        let (new_lines, tmpls) = templates::extract_templates(&lines);
        (new_lines.join("\n"), tmpls)
    } else {
        (body_working, vec![])
    };

    // Stats
    let compressed_len = body.len()
        + all_legend
            .iter()
            .map(|e| format!("#{tag}# = {val}\n", tag = e.tag, val = e.value).len())
            .sum::<usize>()
        + tmpl_list
            .iter()
            .map(|t| format!("&{tag} = {pat}\n", tag = t.tag, pat = t.pattern).len())
            .sum::<usize>()
        + common_prefix.len();

    let ratio = 100.0 * (1.0 - compressed_len as f64 / original_len.max(1) as f64);

    let mut stats = HashMap::new();
    stats.insert("original_chars".to_string(), original_len.to_string());
    stats.insert("compressed_chars".to_string(), compressed_len.to_string());
    stats.insert("ratio_pct".to_string(), format!("{ratio:.1}"));
    stats.insert("legend_entries".to_string(), all_legend.len().to_string());
    stats.insert("template_entries".to_string(), tmpl_list.len().to_string());
    stats.insert("profile".to_string(), detected_profile.clone());
    stats.insert("bpe_passes_used".to_string(), passes_used.to_string());

    CompressResult {
        body,
        legend: all_legend,
        templates: tmpl_list,
        common_prefix,
        detected_profile,
        stats,
    }
}

// ─── Section regexes for decompress ──────────────────────────────────────────

use regex::Regex;
use std::sync::OnceLock;

static SECTION_RE: OnceLock<Regex> = OnceLock::new();
static LEGEND_LINE_RE: OnceLock<Regex> = OnceLock::new();
static TMPL_LEGEND_RE: OnceLock<Regex> = OnceLock::new();

fn section_re() -> &'static Regex {
    SECTION_RE.get_or_init(|| Regex::new(r"^---\s*(\w+)\s*---\s*$").unwrap())
}

fn legend_line_re() -> &'static Regex {
    LEGEND_LINE_RE.get_or_init(|| Regex::new(r"^#([0-9a-zA-Z]+)#\s*=\s*(.*)$").unwrap())
}

fn tmpl_legend_re() -> &'static Regex {
    TMPL_LEGEND_RE.get_or_init(|| Regex::new(r"^&([0-9a-zA-Z]+)\s*=\s*(.*)$").unwrap())
}

pub fn decompress(rendered: &str) -> String {
    let mut section = String::new();
    let mut prefix = String::new();
    let mut legend_entries: Vec<legend::LegendEntry> = Vec::new();
    let mut tmpl_entries: Vec<templates::Template> = Vec::new();
    let mut body_lines: Vec<String> = Vec::new();

    for line in rendered.lines() {
        // Skip preamble comment lines before any section
        if line.starts_with('#') && section.is_empty() {
            continue;
        }
        if let Some(caps) = section_re().captures(line) {
            section = caps[1].to_uppercase();
            continue;
        }
        match section.as_str() {
            "PREFIX" => prefix = line.to_string(),
            "LEGEND" => {
                if let Some(caps) = legend_line_re().captures(line) {
                    legend_entries.push(legend::LegendEntry {
                        tag: caps[1].to_string(),
                        value: caps[2].to_string(),
                        count: 0,
                        profit: 0,
                    });
                } else if let Some(caps) = tmpl_legend_re().captures(line) {
                    tmpl_entries.push(templates::Template {
                        tag: caps[1].to_string(),
                        pattern: caps[2].to_string(),
                        values: vec![],
                        line_indices: vec![],
                    });
                }
            }
            "BODY" => body_lines.push(line.to_string()),
            _ => {}
        }
    }

    let body_refs: Vec<&str> = body_lines.iter().map(|s| s.as_str()).collect();
    let expanded_lines = templates::reverse_templates(&body_refs, &tmpl_entries);
    let body = expanded_lines.join("\n");
    let expanded = legend::reverse_legend(&body, &legend_entries);

    if prefix.is_empty() {
        expanded
    } else {
        expanded
            .lines()
            .map(|l| format!("{prefix}{l}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
