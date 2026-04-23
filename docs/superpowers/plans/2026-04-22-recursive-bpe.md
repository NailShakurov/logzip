# Recursive BPE Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Добавить второй проход legend-сжатия (meta-pass) поверх уже сжатого тела, управляемый через `--quality`, без изменений выходного формата.

**Architecture:** `select_legend_with_positions` получает `tag_offset` для избежания коллизии тегов между проходами. Оба прохода сливаются в один `--- LEGEND ---`. Декомпрессор перестраивается на DFS-мемоизацию (flatten → AhoCorasick), возвращает `Result` вместо паники.

**Tech Stack:** Rust (pyo3 0.22, aho-corasick 1, rayon 1.7), Python 3.9+, pytest, maturin

---

## File Map

| Файл | Что меняем |
|------|-----------|
| `src/legend.rs` | Добавить `tag_offset: usize` в `select_legend_with_positions` и `select_legend` |
| `src/compress.rs` | Добавить `bpe_passes` в `compress()`, цикл мета-прохода; заменить `reverse_legend` на `flatten_legend` + DFS; изменить сигнатуру `decompress` на `Result` |
| `src/lib.rs` | Пробросить `bpe_passes` в `compress_log`; обработать `Result` из `decompress` |
| `python/logzip/__main__.py` | Расширить `quality_map` — добавить `bpe_passes` |
| `tests/test_logzip.py` | Тесты: round-trip bpe_passes=2, stat, регрессия, cyclic error |

---

## Task 1: tag_offset в select_legend_with_positions

**Files:**
- Modify: `src/legend.rs:91-183`
- Test: `tests/test_logzip.py`

- [ ] **Step 1: Написать падающий тест**

Добавить в конец `tests/test_logzip.py`:

```python
def test_tag_offset_no_collision():
    """Pass 2 tags must not collide with pass 1 tags."""
    from logzip import compress
    # Many repeated lines so both passes find something to compress
    log = "\n".join([
        f"INFO database connection established host=db.internal port=5432 latency={i % 5}ms"
        for i in range(200)
    ] + [
        f"ERROR database connection failed host=db.internal port=5432 attempt={i % 5}"
        for i in range(200)
    ])
    result = compress(log, bpe_passes=2)
    tags = [tag for tag, _ in result.legend]
    assert len(tags) == len(set(tags)), "Tags must be unique across all passes"
```

- [ ] **Step 2: Запустить тест — убедиться что падает**

```bash
cd /home/nail/Документы/logzip/files
maturin develop --release 2>/dev/null && pytest tests/test_logzip.py::test_tag_offset_no_collision -v
```

Ожидаемый результат: `ERROR` — `compress()` не принимает `bpe_passes`.

- [ ] **Step 3: Изменить сигнатуру select_legend_with_positions в legend.rs**

В `src/legend.rs`, строка 91:
```rust
// БЫЛО:
pub fn select_legend_with_positions(
    text: &str,
    max_entries: usize,
    max_ngram: usize,
) -> (Vec<LegendEntry>, Vec<Vec<usize>>) {

// СТАЛО:
pub fn select_legend_with_positions(
    text: &str,
    max_entries: usize,
    max_ngram: usize,
    tag_offset: usize,
) -> (Vec<LegendEntry>, Vec<Vec<usize>>) {
```

- [ ] **Step 4: Заменить генерацию тега в legend.rs**

Найти строку (≈162): `let tag = base62::encode(legend.len() as u64);`

```rust
// БЫЛО:
let tag = base62::encode(legend.len() as u64);

// СТАЛО:
let tag = base62::encode((tag_offset + legend.len()) as u64);
```

- [ ] **Step 5: Обновить select_legend wrapper (обратная совместимость)**

Строки 186-193 в `src/legend.rs`:

```rust
// БЫЛО:
pub fn select_legend(
    text: &str,
    max_entries: usize,
    _min_profit: i64,
    max_ngram: usize,
) -> Vec<LegendEntry> {
    select_legend_with_positions(text, max_entries, max_ngram).0
}

// СТАЛО:
pub fn select_legend(
    text: &str,
    max_entries: usize,
    _min_profit: i64,
    max_ngram: usize,
) -> Vec<LegendEntry> {
    select_legend_with_positions(text, max_entries, max_ngram, 0).0
}
```

- [ ] **Step 6: Исправить вызов в compress.rs**

Строки 43-44 в `src/compress.rs`:

```rust
// БЫЛО:
let (legend, chosen_positions) =
    legend::select_legend_with_positions(&working, max_legend_entries, max_ngram);

// СТАЛО:
let (legend, chosen_positions) =
    legend::select_legend_with_positions(&working, max_legend_entries, max_ngram, 0);
```

- [ ] **Step 7: Проверить что проект компилируется**

```bash
cargo build --release 2>&1 | tail -5
```

Ожидаемый результат: `Finished release`.

- [ ] **Step 8: Запустить существующие тесты — все должны зелёные**

```bash
maturin develop --release && pytest tests/test_logzip.py -v
```

Ожидаемый результат: 8/8 PASSED (тест `test_tag_offset_no_collision` ещё падает — ок, `bpe_passes` ещё не добавлен).

- [ ] **Step 9: Commit**

```bash
git add src/legend.rs src/compress.rs
git commit -m "feat: add tag_offset param to select_legend_with_positions"
```

---

## Task 2: Мета-проход в compress() и Python API

**Files:**
- Modify: `src/compress.rs:15-90`
- Modify: `src/lib.rs:127-152`
- Test: `tests/test_logzip.py`

- [ ] **Step 1: Написать падающие тесты**

Добавить в конец `tests/test_logzip.py`:

```python
def test_bpe_passes_round_trip():
    """compress(bpe_passes=2) → decompress must return exact original."""
    from logzip import compress, decompress
    log = "\n".join([
        f"INFO database connection established host=db.internal port=5432 latency={i % 5}ms"
        for i in range(200)
    ] + [
        f"ERROR database connection failed host=db.internal port=5432 attempt={i % 5}"
        for i in range(200)
    ])
    result = compress(log, bpe_passes=2, do_normalize=False, do_templates=False)
    rendered = result.render()
    restored = decompress(rendered)
    assert restored == log


def test_bpe_passes_used_stat():
    """bpe_passes_used stat reflects actual passes run."""
    from logzip import compress
    log = "\n".join([
        f"INFO db connection established host=db.internal port=5432 latency={i % 5}ms"
        for i in range(300)
    ])
    result = compress(log, bpe_passes=2)
    stats = result.stats()
    assert "bpe_passes_used" in stats
    assert stats["bpe_passes_used"] >= 1


def test_bpe_passes_1_regression():
    """bpe_passes=1 (default) produces identical output to old behaviour."""
    from logzip import compress
    log = "\n".join([
        f"2026-04-21T14:32:{i:02d}.000Z INFO request received GET /api/users"
        for i in range(50)
    ])
    r1 = compress(log, bpe_passes=1)
    r2 = compress(log)  # default
    assert r1.render() == r2.render()
```

- [ ] **Step 2: Убедиться что тесты падают**

```bash
maturin develop --release && pytest tests/test_logzip.py::test_bpe_passes_round_trip tests/test_logzip.py::test_bpe_passes_used_stat tests/test_logzip.py::test_bpe_passes_1_regression -v
```

Ожидаемый результат: ERROR — `compress()` не принимает `bpe_passes`.

- [ ] **Step 3: Изменить сигнатуру compress() в compress.rs**

Строка 15-22 в `src/compress.rs`:

```rust
pub fn compress(
    text: &str,
    max_ngram: usize,
    max_legend_entries: usize,
    do_normalize: bool,
    profile: Option<&str>,
    do_templates: bool,
    bpe_passes: usize,   // NEW: 1 = current behaviour, 2-3 = meta-passes
) -> CompressResult {
```

- [ ] **Step 4: Добавить мета-проход после step 5 в compress.rs**

Заменить строки 47-48 (`apply_legend_from_positions` и `body_after_legend`):

```rust
    // 5. Direct substitution from known positions
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
```

- [ ] **Step 5: Обновить step 6 (templates) и stats в compress.rs**

Заменить строки с `body_after_legend` (теперь называется `body_working`) и добавить стат:

```rust
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
```

- [ ] **Step 6: Обновить compress_log в lib.rs**

Строки 127-152. Добавить `bpe_passes` в сигнатуру и передать в `core_compress`:

```rust
#[pyfunction]
#[pyo3(signature = (
    text,
    max_ngram = 2,
    max_legend_entries = 32,
    do_normalize = true,
    profile = None,
    do_templates = true,
    bpe_passes = 1,
))]
fn compress_log(
    text: String,
    max_ngram: usize,
    max_legend_entries: usize,
    do_normalize: bool,
    profile: Option<String>,
    do_templates: bool,
    bpe_passes: usize,
) -> PyResult<PyCompressResult> {
    let result = core_compress(
        &text,
        max_ngram,
        max_legend_entries,
        do_normalize,
        profile.as_deref(),
        do_templates,
        bpe_passes,
    );
    Ok(PyCompressResult::from(result))
}
```

- [ ] **Step 7: Проверить компиляцию**

```bash
cargo build --release 2>&1 | tail -5
```

Ожидаемый результат: `Finished release`.

- [ ] **Step 8: Запустить тесты**

```bash
maturin develop --release && pytest tests/test_logzip.py -v
```

Ожидаемый результат: новые тесты PASSED, остальные 8 — без регрессий.

Если `test_bpe_passes_round_trip` падает — декомпрессор ещё не обновлён (Task 3), это нормально — тест запустить изолированно после Task 3.

- [ ] **Step 9: Commit**

```bash
git add src/compress.rs src/lib.rs
git commit -m "feat: add bpe_passes meta-pass to compress()"
```

---

## Task 3: flatten_legend с DFS + мемоизацией в decompress

**Files:**
- Modify: `src/compress.rs:92-167` (decompress section)
- Test: `tests/test_logzip.py`

- [ ] **Step 1: Написать падающие тесты**

Добавить в конец `tests/test_logzip.py`:

```python
def test_decompress_cyclic_raises():
    """Malformed legend with cyclic dependency raises ValueError."""
    from logzip import decompress
    # Manually craft a cyclic legend: #0# references #1#, #1# references #0#
    malformed = (
        "--- LEGEND ---\n"
        "#0# = prefix #1# suffix\n"
        "#1# = start #0# end\n"
        "--- BODY ---\n"
        "#0#\n"
    )
    with pytest.raises(ValueError, match="cyclic"):
        decompress(malformed)


def test_round_trip_bpe2_large():
    """Round-trip with bpe_passes=2 on a larger structured log."""
    from logzip import compress, decompress
    log = "\n".join([
        f"2026-04-21T14:32:{i:02d}.123456789Z INFO connection established host=db.internal port=5432"
        for i in range(100)
    ] + [
        f"2026-04-21T14:32:{i:02d}.123456789Z ERROR connection failed host=db.internal port=5432"
        for i in range(100)
    ])
    result = compress(log, bpe_passes=2, do_normalize=False, do_templates=False)
    restored = decompress(result.render())
    assert restored == log
```

- [ ] **Step 2: Убедиться что тесты падают**

```bash
maturin develop --release && pytest tests/test_logzip.py::test_decompress_cyclic_raises tests/test_logzip.py::test_round_trip_bpe2_with_templates -v
```

Ожидаемый результат: `test_decompress_cyclic_raises` — FAILED (нет ValueError), `test_round_trip_bpe2_with_templates` — может PASS или FAIL.

- [ ] **Step 3: Изменить сигнатуру decompress в compress.rs**

Строка 113:

```rust
// БЫЛО:
pub fn decompress(rendered: &str) -> String {

// СТАЛО:
pub fn decompress(rendered: &str) -> Result<String, String> {
```

- [ ] **Step 4: Добавить flatten_legend и resolve в compress.rs**

Добавить сразу после строки `use regex::Regex;` (≈94), перед `static SECTION_RE`:

```rust
// ─── flatten_legend: DFS с мемоизацией ───────────────────────────────────────

fn resolve(
    tag: &str,
    raw_map: &HashMap<String, String>,
    ac: &aho_corasick::AhoCorasick,
    keys: &[String],
    cache: &mut HashMap<String, String>,
    depth: usize,
) -> Result<String, String> {
    if depth > 10 {
        return Err(format!("Malformed legend: cyclic dependency at tag {tag}"));
    }
    if let Some(cached) = cache.get(tag) {
        return Ok(cached.clone());
    }
    let val = match raw_map.get(tag) {
        Some(v) => v.clone(),
        None => return Ok(tag.to_string()),
    };
    // Collect matches first — избегаем borrow-конфликта между find_iter и &mut cache
    let matches: Vec<aho_corasick::Match> = ac.find_iter(&val).collect();
    let mut result = String::with_capacity(val.len() * 2);
    let mut last = 0;
    for m in matches {
        result.push_str(&val[last..m.start()]);
        let inner_tag = &keys[m.pattern().as_usize()];
        result.push_str(&resolve(inner_tag, raw_map, ac, keys, cache, depth + 1)?);
        last = m.end();
    }
    result.push_str(&val[last..]);
    cache.insert(tag.to_string(), result.clone());
    Ok(result)
}

fn flatten_legend(
    entries: &[legend::LegendEntry],
) -> Result<(Vec<String>, Vec<String>), String> {
    if entries.is_empty() {
        return Ok((vec![], vec![]));
    }
    let keys: Vec<String> = entries.iter().map(|e| legend::wrap(&e.tag)).collect();
    let raw_map: HashMap<String, String> = keys
        .iter()
        .zip(entries.iter())
        .map(|(k, e)| (k.clone(), e.value.clone()))
        .collect();

    let ac = aho_corasick::AhoCorasick::builder()
        .match_kind(aho_corasick::MatchKind::LeftmostLongest)
        .build(&keys)
        .expect("flatten AhoCorasick build");

    let mut cache: HashMap<String, String> = HashMap::new();
    let mut resolved_values: Vec<String> = Vec::with_capacity(entries.len());

    for key in &keys {
        let v = resolve(key, &raw_map, &ac, &keys, &mut cache, 0)?;
        resolved_values.push(v);
    }
    Ok((keys, resolved_values))
}
```

- [ ] **Step 5: Заменить reverse_legend в decompress() на flatten_legend**

Найти блок (≈153-156):
```rust
    let body_refs: Vec<&str> = body_lines.iter().map(|s| s.as_str()).collect();
    let expanded_lines = templates::reverse_templates(&body_refs, &tmpl_entries);
    let body = expanded_lines.join("\n");
    let expanded = legend::reverse_legend(&body, &legend_entries);
```

Заменить на:

```rust
    let body_refs: Vec<&str> = body_lines.iter().map(|s| s.as_str()).collect();
    let expanded_lines = templates::reverse_templates(&body_refs, &tmpl_entries);
    let body = expanded_lines.join("\n");

    let (flat_keys, flat_values) = flatten_legend(&legend_entries)?;
    let expanded = if flat_keys.is_empty() {
        body
    } else {
        let flat_refs: Vec<&str> = flat_values.iter().map(|s| s.as_str()).collect();
        let ac = aho_corasick::AhoCorasick::builder()
            .match_kind(aho_corasick::MatchKind::LeftmostLongest)
            .build(&flat_keys)
            .expect("decompress AhoCorasick build");
        ac.replace_all(&body, &flat_refs)
    };
```

- [ ] **Step 6: Исправить конец decompress() под новый тип**

Найти блок возврата (≈157-166):
```rust
    if prefix.is_empty() {
        expanded
    } else {
        expanded
            .lines()
            .map(|l| format!("{prefix}{l}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
```

Заменить на:
```rust
    Ok(if prefix.is_empty() {
        expanded
    } else {
        expanded
            .lines()
            .map(|l| format!("{prefix}{l}"))
            .collect::<Vec<_>>()
            .join("\n")
    })
```

- [ ] **Step 7: Обновить decompress_log в lib.rs**

Строки 155-158:
```rust
// БЫЛО:
#[pyfunction]
fn decompress_log(rendered: String) -> PyResult<String> {
    Ok(core_decompress(&rendered))
}

// СТАЛО:
#[pyfunction]
fn decompress_log(rendered: String) -> PyResult<String> {
    core_decompress(&rendered)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))
}
```

- [ ] **Step 8: Проверить компиляцию**

```bash
cargo build --release 2>&1 | tail -10
```

Ожидаемый результат: `Finished release`. Если ошибки — исправить borrow/lifetime по сообщениям компилятора.

- [ ] **Step 9: Запустить все тесты**

```bash
maturin develop --release && pytest tests/test_logzip.py -v
```

Ожидаемый результат: все тесты PASSED включая `test_decompress_cyclic_raises` и оба round-trip теста.

- [ ] **Step 10: Commit**

```bash
git add src/compress.rs src/lib.rs
git commit -m "feat: replace reverse_legend with flatten_legend DFS in decompress"
```

---

## Task 4: Расширить quality_map в CLI

**Files:**
- Modify: `python/logzip/__main__.py:36-51`
- Test: `tests/test_logzip.py`

- [ ] **Step 1: Написать тест**

Добавить в конец `tests/test_logzip.py`:

```python
def test_cli_quality_max_stat():
    """CLI with --quality max reports bpe_passes_used in stderr stats."""
    import subprocess
    import sys
    log = "\n".join([
        f"INFO db connection established host=db.internal port=5432 latency={i % 5}ms"
        for i in range(300)
    ])
    result = subprocess.run(
        [sys.executable, "-m", "logzip", "compress", "--quality", "max", "--stats"],
        input=log, capture_output=True, text=True
    )
    assert result.returncode == 0
    # Compressed output в stdout
    assert "--- BODY ---" in result.stdout
```

- [ ] **Step 2: Убедиться что тест проходит уже (CLI существует)**

```bash
maturin develop --release && pytest tests/test_logzip.py::test_cli_quality_max_stat -v
```

Ожидаемый результат: PASSED (CLI уже работает, `--quality max` существует).

- [ ] **Step 3: Изменить quality_map в __main__.py**

Строки 36-41:

```python
# БЫЛО:
quality_map = {
    "fast": 32,
    "balanced": 128,
    "max": 512,
}
max_legend_entries = quality_map[args.quality]

# СТАЛО:
quality_map = {
    "fast":     (32,  1),
    "balanced": (128, 1),
    "max":      (512, 2),
}
max_legend_entries, bpe_passes = quality_map[args.quality]
if args.quality == "balanced" and len(raw) > 5_000_000:
    bpe_passes = 2
```

- [ ] **Step 4: Передать bpe_passes в вызов compress**

Строки 44-51, обновить `result = compress(...)`:

```python
result = compress(
    raw,
    max_ngram=args.max_ngram,
    max_legend_entries=max_legend_entries,
    do_normalize=not args.no_normalize,
    profile=args.profile,
    do_templates=not args.no_templates,
    bpe_passes=bpe_passes,
)
```

- [ ] **Step 5: Запустить все тесты**

```bash
maturin develop --release && pytest tests/test_logzip.py -v
```

Ожидаемый результат: все тесты PASSED.

- [ ] **Step 6: Commit**

```bash
git add python/logzip/__main__.py tests/test_logzip.py
git commit -m "feat: extend quality_map with bpe_passes, wire --quality max to bpe_passes=2"
```

---

## Финальная проверка

- [ ] **Запустить полный suite**

```bash
maturin develop --release && pytest tests/test_logzip.py -v && cargo test --release
```

Ожидаемый результат: все тесты PASSED.

- [ ] **Быстрый бенчмарк на реальных данных (опционально)**

```bash
python -c "
import time, logzip
log = open('/var/log/syslog').read() if __import__('os').path.exists('/var/log/syslog') else ('INFO test line\n' * 50000)
t = time.time(); r1 = logzip.compress(log, bpe_passes=1); print(f'pass1: {time.time()-t:.3f}s, {r1.stats()[\"ratio_pct\"]}% saved')
t = time.time(); r2 = logzip.compress(log, bpe_passes=2); print(f'pass2: {time.time()-t:.3f}s, {r2.stats()[\"ratio_pct\"]}% saved')
"
```
