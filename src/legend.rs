//! Legend builder.
//!
//! Алгоритм O(N·K/cores + K log K):
//! 1. count_candidates  — параллельный подсчёт n-грамм (rayon).
//! 2. pre_filter        — топ max_entries по rough_profit.
//! 3. build_index       — параллельный match_indices, O(N).
//! 4. greedy_select     — blocked[pos] O(1), fill только start-байта.
//! 5. apply_from_pos    — прямая замена по уже известным позициям, без AhoCorasick.

use crate::base62;
use aho_corasick::{AhoCorasick, MatchKind};
use rayon::prelude::*;
use std::collections::HashMap;

// ─── Structs ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LegendEntry {
    pub tag: String,
    pub value: String,
    pub count: usize,
    pub profit: i64,
}

#[inline]
pub fn wrap(tag: &str) -> String {
    format!("#{tag}#")
}

#[inline]
fn profit_score(value_len: usize, count: usize, tag_len: usize) -> i64 {
    let wrapped = tag_len + 2;
    let saved = value_len as i64 - wrapped as i64;
    saved * count as i64 - (wrapped + 3 + value_len + 1) as i64
}

#[inline]
fn rough_profit(value_len: usize, count: usize) -> i64 {
    // tag="#0#" len=3
    (value_len as i64 - 3) * count as i64 - (3 + 3 + value_len + 1) as i64
}

// ─── Frequency analysis ───────────────────────────────────────────────────────

pub fn count_candidates(
    text: &str,
    max_ngram: usize,
    min_token_len: usize,
) -> HashMap<String, usize> {
    let lines: Vec<&str> = text.lines().collect();
    let local: Vec<HashMap<String, usize>> = lines
        .par_chunks(8_000)
        .map(|chunk| {
            let mut m: HashMap<String, usize> = HashMap::new();
            for line in chunk {
                let tokens: Vec<&str> = line.split_whitespace().collect();
                for tok in &tokens {
                    if tok.len() >= min_token_len {
                        *m.entry(tok.to_string()).or_insert(0) += 1;
                    }
                }
                for n in 2..=max_ngram {
                    if tokens.len() < n {
                        break;
                    }
                    for w in tokens.windows(n) {
                        let ng = w.join(" ");
                        if ng.len() >= min_token_len + 2 {
                            *m.entry(ng).or_insert(0) += 1;
                        }
                    }
                }
            }
            m
        })
        .collect();

    let mut result: HashMap<String, usize> = HashMap::new();
    for map in local {
        for (k, v) in map {
            *result.entry(k).or_insert(0) += v;
        }
    }
    result
}

// ─── Selection — возвращает записи + их выбранные позиции ────────────────────

/// Возвращает (entries, chosen_positions_per_entry).
/// chosen_positions[i] — отсортированные byte-offsets вхождений legend[i].
pub fn select_legend_with_positions(
    text: &str,
    max_entries: usize,
    max_ngram: usize,
) -> (Vec<LegendEntry>, Vec<Vec<usize>>) {
    // 1. Count
    let counter = count_candidates(text, max_ngram, 5);

    // 2. Pre-filter
    let mut candidates: Vec<(String, usize)> = counter
        .into_iter()
        .filter(|(v, cnt)| *cnt >= 2 && rough_profit(v.len(), *cnt) > 0)
        .collect();

    if candidates.is_empty() {
        return (vec![], vec![]);
    }

    candidates.sort_unstable_by(|a, b| {
        rough_profit(b.0.len(), b.1).cmp(&rough_profit(a.0.len(), a.1))
    });
    candidates.truncate(max_entries);

    // Позиционный индекс: параллельный match_indices для каждого кандидата.
    // Ограничено max_entries. match_indices использует SIMD (memchr), O(N) per candidate.
    let positions_by_cand: Vec<Vec<usize>> = candidates
        .par_iter()
        .map(|(val, _)| {
            text.match_indices(val.as_str())
                .map(|(pos, _)| pos)
                .collect()
        })
        .collect();

    // Перекладываем: positions[i] = позиции для candidates[i]
    let positions = positions_by_cand;


    // 4. Жадный выбор
    let mut ranked: Vec<usize> = (0..candidates.len())
        .filter(|&i| positions[i].len() >= 2)
        .collect();
    ranked.sort_unstable_by(|&a, &b| {
        rough_profit(candidates[b].0.len(), positions[b].len())
            .cmp(&rough_profit(candidates[a].0.len(), positions[a].len()))
    });

    let text_len = text.len();
    // blocked[pos]=true — байт занят. Проверяем только первый байт (all tokens are word-aligned).
    let mut blocked: Vec<bool> = vec![false; text_len + 1];
    let mut legend: Vec<LegendEntry> = Vec::with_capacity(max_entries.min(ranked.len()));
    let mut chosen_positions: Vec<Vec<usize>> = Vec::with_capacity(max_entries.min(ranked.len()));

    for &ci in &ranked {
        if legend.len() >= max_entries {
            break;
        }
        let value = &candidates[ci].0;
        let val_len = value.len();

        // Собираем незаблокированные позиции
        let free: Vec<usize> = positions[ci]
            .iter()
            .filter(|&&pos| pos + val_len <= text_len && !blocked[pos])
            .copied()
            .collect();

        if free.len() < 2 {
            continue;
        }

        let tag = base62::encode(legend.len() as u64);
        let p = profit_score(val_len, free.len(), tag.len());
        if p <= 0 {
            continue;
        }

        // Блокируем: только первый байт каждого вхождения
        for &pos in &free {
            blocked[pos] = true;
        }

        legend.push(LegendEntry {
            tag,
            value: value.clone(),
            count: free.len(),
            profit: p,
        });
        chosen_positions.push(free);
    }

    (legend, chosen_positions)
}

// Обёртка для обратной совместимости с compress.rs
pub fn select_legend(
    text: &str,
    max_entries: usize,
    _min_profit: i64,
    max_ngram: usize,
) -> Vec<LegendEntry> {
    select_legend_with_positions(text, max_entries, max_ngram).0
}

// ─── Применение — прямая замена по известным позициям ────────────────────────

/// Прямая замена без AhoCorasick.
/// Принимает позиции из select_legend_with_positions → O(N + total_matches).
pub fn apply_legend_from_positions(
    text: &str,
    legend: &[LegendEntry],
    positions: &[Vec<usize>],
) -> String {
    if legend.is_empty() {
        return text.to_string();
    }

    // Позиции из find_iter уже отсортированы по возрастанию.
    // K-way merge через BinaryHeap: (pos, entry_idx, iter_idx).
    // Это O((N + total_matches) × log K) вместо O(total_matches × log total_matches).
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;

    let replacements: Vec<String> = legend.iter().map(|e| wrap(&e.tag)).collect();

    // heap: (Reverse(pos), entry_idx, position_list_offset)
    let mut heap: BinaryHeap<(Reverse<usize>, usize, usize)> = BinaryHeap::new();
    for (i, pos_list) in positions.iter().enumerate() {
        if let Some(&first) = pos_list.first() {
            heap.push((Reverse(first), i, 0));
        }
    }

    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    let text_len = text.len();

    while let Some((Reverse(pos), ei, offset)) = heap.pop() {
        let val_len = legend[ei].value.len();
        let end = pos + val_len;

        // Следующий элемент из этого потока
        let next_offset = offset + 1;
        if next_offset < positions[ei].len() {
            heap.push((Reverse(positions[ei][next_offset]), ei, next_offset));
        }

        // Пропускаем перекрытия (blocked уже гарантирует их не будет, но safety first)
        if pos < last || end > text_len {
            continue;
        }

        out.push_str(&text[last..pos]);
        out.push_str(&replacements[ei]);
        last = end;
    }
    out.push_str(&text[last..]);
    out
}


// ─── apply_legend для обратной совместимости (decompress не использует позиции)

pub fn apply_legend(text: &str, legend: &[LegendEntry]) -> String {
    if legend.is_empty() {
        return text.to_string();
    }
    let mut ordered: Vec<&LegendEntry> = legend.iter().collect();
    ordered.sort_unstable_by(|a, b| b.value.len().cmp(&a.value.len()));

    let patterns: Vec<&str> = ordered.iter().map(|e| e.value.as_str()).collect();
    let replacements: Vec<String> = ordered.iter().map(|e| wrap(&e.tag)).collect();
    let repl_refs: Vec<&str> = replacements.iter().map(|s| s.as_str()).collect();

    let ac = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostLongest)
        .build(&patterns)
        .expect("aho-corasick apply build");

    ac.replace_all(text, &repl_refs)
}

// ─── Reverse (decompress) ─────────────────────────────────────────────────────

pub fn reverse_legend(compressed: &str, legend: &[LegendEntry]) -> String {
    if legend.is_empty() {
        return compressed.to_string();
    }
    let mut ordered: Vec<&LegendEntry> = legend.iter().collect();
    ordered.sort_unstable_by(|a, b| {
        b.tag.len().cmp(&a.tag.len()).then(b.tag.cmp(&a.tag))
    });

    let wrapped: Vec<String> = ordered.iter().map(|e| wrap(&e.tag)).collect();
    let values: Vec<&str> = ordered.iter().map(|e| e.value.as_str()).collect();
    let tag_refs: Vec<&str> = wrapped.iter().map(|s| s.as_str()).collect();

    let ac = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostLongest)
        .build(&tag_refs)
        .expect("aho-corasick reverse build");

    ac.replace_all(compressed, &values)
}
