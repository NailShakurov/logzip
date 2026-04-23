# Recursive BPE (Multi-pass Legend Compression)

**Date:** 2026-04-22
**Status:** Approved

---

## Goal

Run a second legend-selection pass on the already-compressed body to find repeated tag sequences and compress them further. Expected gain: 5–10% on structured logs. No format changes. Full backward compatibility.

---

## Architecture

### Tag Namespace (no collision)

`select_legend_with_positions` receives a `tag_offset: usize` parameter.  
Pass 1: tags start at `base62::encode(0)` → `0`, `1`, ..., `Z`, `a`, ...  
Pass 2: tags start at `base62::encode(pass1_legend.len())` → continues from where pass 1 left off.

By construction, pass 2 values reference only pass 1 tags. No cycles possible. No format changes.

### compress.rs — changes

`compress()` gains parameter `bpe_passes: usize` (default `1`).

Pipeline after step 5 (apply_legend), if `bpe_passes >= 2` and `body.len() > 256`:

```
body_after_legend
  → select_legend_with_positions(body, max_entries, max_ngram, tag_offset = pass1_legend.len())
  → apply_legend_from_positions(body, meta_legend, meta_positions)
  → legend.extend(meta_legend)   // merged into single Vec, single --- LEGEND --- section
```

Guard: skip meta-pass if projected savings < 5% (compare `meta_body.len()` against `body_after_legend.len()`).

Stats field `bpe_passes_used: usize` — how many passes actually ran (may be less than requested if guard fired).

### legend.rs — changes

`select_legend_with_positions` signature:

```rust
pub fn select_legend_with_positions(
    text: &str,
    max_entries: usize,
    max_ngram: usize,
    tag_offset: usize,   // NEW — existing callers pass 0
) -> (Vec<LegendEntry>, Vec<Vec<usize>>)
```

Tag generation: `base62::encode((tag_offset + legend.len()) as u64)`.

### decompress — flatten_legend with DFS + memoization

Replace current `reverse_legend` call with two steps:

**Step 1: flatten_legend**

```rust
// Ошибка вместо паники — в FFI-слое (PyO3) panic! убивает процесс Python без исключения.
fn resolve(
    tag: &str,
    raw_map: &HashMap<&str, &str>,
    ac: &AhoCorasick,          // построен один раз на всех ключах legend
    keys: &[&str],             // порядок ключей для ac
    cache: &mut HashMap<String, String>,
    depth: usize,
) -> Result<String, String> {
    if depth > 10 {
        return Err(format!("Malformed legend: cyclic dependency at tag {tag}"));
    }
    if let Some(cached) = cache.get(tag) {
        return Ok(cached.clone());
    }
    let val = raw_map.get(tag).copied().unwrap_or(tag);
    // Используем тот же AhoCorasick (построен на ключах легенды) — не компилируем regex в цикле.
    let mut result = String::with_capacity(val.len());
    let mut last = 0;
    for m in ac.find_iter(val) {
        result.push_str(&val[last..m.start()]);
        let inner_tag = keys[m.pattern().as_usize()];
        result.push_str(&resolve(inner_tag, raw_map, ac, keys, cache, depth + 1)?);
        last = m.end();
    }
    result.push_str(&val[last..]);
    cache.insert(tag.to_string(), result.clone());
    Ok(result)
}
```

AhoCorasick строится **один раз** на всех ключах легенды (`#0#`, `#1#`, ...) перед резолвингом.  
Используется и для flatten (шаг 1), и для замены в BODY (шаг 2) — одна аллокация.  
Complexity: O(V+E). Depth guard: `Err` если depth > 10, пробрасывается до PyO3 как `ValueError`.

**Step 2: AhoCorasick on flat map**

Тот же `ac` объект + плоский `resolved_map` → `ac.replace_all(body, &resolved_values)`.  
Single pass over `--- BODY ---`. O(N).

### Python API — lib.rs

```rust
#[pyfunction]
#[pyo3(signature = (
    text,
    max_ngram = 2,
    max_legend_entries = 32,
    do_normalize = true,
    profile = None,
    do_templates = true,
    bpe_passes = 1,        // NEW
))]
fn compress_log(..., bpe_passes: usize) -> PyResult<PyCompressResult>
```

PyO3 `#[pyo3(signature = (...))]` пробрасывает имена параметров в Python as-is.  
`do_normalize=True`, `do_templates=True`, `bpe_passes=1` — соответствуют Python-конвенции.  
Существующий API (`do_normalize`, `do_templates`) не меняется, `bpe_passes` добавляется в конец — обратная совместимость сохранена.

`PyCompressResult.stats()` exposes `bpe_passes_used`.

### CLI — __main__.py

`--quality` already controls `max_legend_entries`. Extend to also control `bpe_passes`:

```python
quality_map = {
    "fast":     (32,  1),
    "balanced": (128, 1),   # auto-upgrades to 2 if len(raw) > 5_000_000
    "max":      (512, 2),
}
max_legend_entries, bpe_passes = quality_map[args.quality]
if args.quality == "balanced" and len(raw) > 5_000_000:
    bpe_passes = 2
```

No new CLI flags. `bpe_passes` is an implementation detail hidden from the user.

---

## Files Changed

| File | Change |
|------|--------|
| `src/legend.rs` | add `tag_offset` param to `select_legend_with_positions` |
| `src/compress.rs` | add `bpe_passes` param, second-pass loop, `flatten_legend` in decompress |
| `src/lib.rs` | expose `bpe_passes` in `compress_log`, add `bpe_passes_used` to stats |
| `python/logzip/__main__.py` | extend `quality_map` with bpe_passes |
| `tests/test_logzip.py` | add tests for multi-pass compression and round-trip |

---

## Constraints

- Max `bpe_passes`: 3 (risk of slowdown beyond that)
- Meta-pass guard: `body.len() > 256` AND projected savings > 5%
- `select_legend` is not touched beyond the `tag_offset` param
- No format changes — existing compressed files decompress correctly

---

## Tests

1. Round-trip: compress with `bpe_passes=2`, decompress → exact original
2. `bpe_passes_used` stat is correct (respects guard)
3. `bpe_passes=1` behavior identical to current (regression)
4. Decompressor handles single-pass files (tag_offset=0, no meta tags)
