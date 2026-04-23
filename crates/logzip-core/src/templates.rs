//! Template extractor — finds lines differing only in one slot.
//!
//! Format in legend:  &tag = pattern (@ marks the variable slot)
//! Format in body:    &tag:value

use crate::base62;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Template {
    pub tag: String,
    pub pattern: String,
    pub values: Vec<String>,
    pub line_indices: Vec<usize>,
}

const TMPL_PREFIX: char = '&';
const TMPL_SEP: char = ':';
const TMPL_MARKER: char = '@';
const MIN_OCCURRENCES: usize = 3;
const MAX_TEMPLATES: usize = 62;

/// Find and apply templates to body lines. Returns (new_lines, templates).
pub fn extract_templates(lines: &[&str]) -> (Vec<String>, Vec<Template>) {
    let templates = find_templates(lines);
    if templates.is_empty() {
        return (lines.iter().map(|l| l.to_string()).collect(), vec![]);
    }

    // line_idx → (tmpl, value)
    let mut line_map: HashMap<usize, (&Template, &str)> = HashMap::new();
    for tmpl in &templates {
        for (idx, val) in tmpl.line_indices.iter().zip(tmpl.values.iter()) {
            line_map.insert(*idx, (tmpl, val.as_str()));
        }
    }

    let new_lines: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if let Some((tmpl, val)) = line_map.get(&i) {
                format!("{}{}{}{}", TMPL_PREFIX, tmpl.tag, TMPL_SEP, val)
            } else {
                line.to_string()
            }
        })
        .collect();

    (new_lines, templates)
}

fn find_templates(lines: &[&str]) -> Vec<Template> {
    // sig_map: signature_tuple_as_string → [(line_idx, actual_value)]
    let mut sig_map: HashMap<String, Vec<(usize, String)>> = HashMap::new();

    for (idx, line) in lines.iter().enumerate() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 2 || tokens.len() > 20 {
            continue;
        }
        for pos in 0..tokens.len() {
            let val = tokens[pos];
            // Skip values with special chars
            if val.chars().any(|c| matches!(c, ':' | '#' | '&' | '@')) {
                continue;
            }
            let sig = build_signature(&tokens, pos);
            sig_map.entry(sig).or_default().push((idx, val.to_string()));
        }
    }

    // Sort by occurrences * pattern_len desc
    let mut candidates: Vec<(String, Vec<(usize, String)>)> = sig_map.into_iter().collect();
    candidates.sort_unstable_by(|a, b| {
        let score_a = a.1.len() * a.0.len();
        let score_b = b.1.len() * b.0.len();
        score_b.cmp(&score_a)
    });

    let mut templates: Vec<Template> = Vec::new();
    let mut covered: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut tag_counter = 0usize;

    for (sig, occurrences) in candidates {
        let unique_lines: std::collections::HashSet<usize> =
            occurrences.iter().map(|(i, _)| *i).collect();
        let fresh: Vec<usize> = unique_lines.difference(&covered).copied().collect();
        if fresh.len() < MIN_OCCURRENCES {
            continue;
        }

        let pattern = sig.replace('@', &TMPL_MARKER.to_string());
        let tag = base62::encode(tag_counter as u64);

        let fresh_set: std::collections::HashSet<usize> = fresh.iter().copied().collect();
        let values: Vec<String> = occurrences
            .iter()
            .filter(|(i, _)| fresh_set.contains(i))
            .map(|(_, v)| v.clone())
            .collect();
        let line_indices: Vec<usize> = occurrences
            .iter()
            .filter(|(i, _)| fresh_set.contains(i))
            .map(|(i, _)| *i)
            .collect();

        templates.push(Template {
            tag,
            pattern,
            values,
            line_indices,
        });
        covered.extend(fresh);
        tag_counter += 1;

        if tag_counter >= MAX_TEMPLATES {
            break;
        }
    }

    templates
}

fn build_signature(tokens: &[&str], mask_pos: usize) -> String {
    tokens
        .iter()
        .enumerate()
        .map(|(i, t)| if i == mask_pos { "@" } else { t })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reverse template substitution for decompress.
pub fn reverse_templates(lines: &[&str], templates: &[Template]) -> Vec<String> {
    if templates.is_empty() {
        return lines.iter().map(|l| l.to_string()).collect();
    }
    let tmpl_map: HashMap<&str, &str> =
        templates.iter().map(|t| (t.tag.as_str(), t.pattern.as_str())).collect();

    lines
        .iter()
        .map(|line| {
            if line.starts_with(TMPL_PREFIX) {
                if let Some(sep_pos) = line.find(TMPL_SEP) {
                    let tag = &line[1..sep_pos];
                    let val = &line[sep_pos + 1..];
                    if let Some(pattern) = tmpl_map.get(tag) {
                        return pattern.replace(TMPL_MARKER, val);
                    }
                }
            }
            line.to_string()
        })
        .collect()
}
