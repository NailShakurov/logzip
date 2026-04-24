# logzip MCP Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

## ✅ Прогресс (2026-04-24)

| Task | Статус | Коммит |
|---|---|---|
| Task 1: Cargo Workspace skeleton | ✅ DONE | `06bfd44` |
| Task 2: logzip-core + render() + PREAMBLE | ✅ DONE | `bf0fb8a` |
| Task 3: logzip-py + maturin | ✅ DONE | `f8eded3` |
| Task 4: sandbox.rs TDD | ✅ DONE | `8036a45` |
| Task 5: tools.rs TDD | ✅ DONE | `8d21cc6` |
| Task 6: mcp.rs JSON-RPC loop | ✅ DONE | `1c2ccad` |
| Task 7: main.rs CLI dispatcher | ✅ DONE | `7e9746b` |
| Task 8: smoke_mcp.py | ✅ DONE | `39b8399` |
| Task 9: README + финал | ✅ DONE | `08565bd` |

**Статус: все задачи выполнены ✅**

---

**Goal:** Добавить MCP-сервер к logzip через Cargo Workspace: три крейта (`logzip-core`, `logzip-py`, `logzip-mcp`), единый бинарник `logzip` с сабкомандами `compress`, `decompress`, `mcp`.

**Architecture:** Существующий код из `src/` переезжает в `crates/logzip-core/`. PyO3-обёртка — в `crates/logzip-py/`. Новый крейт `crates/logzip-mcp/` собирает бинарник `logzip` с ручным JSON-RPC 2.0 через stdio. Зависимости MCP: только `serde` + `serde_json`, без tokio.

**Tech Stack:** Rust (workspace, rlib + cdylib + bin), serde_json 1.x, PyO3 0.22, maturin 1.5+, pytest.

**Spec:** `docs/superpowers/specs/2026-04-23-logzip-mcp-design.md`

---

## File Map

| Действие | Путь |
|---|---|
| Modify | `Cargo.toml` → workspace root |
| Modify | `pyproject.toml` → добавить manifest-path, rename script |
| Create | `crates/logzip-core/Cargo.toml` |
| Move+Modify | `src/*.rs` → `crates/logzip-core/src/*.rs` (lib.rs пишется заново) |
| Create | `crates/logzip-py/Cargo.toml` |
| Move+Modify | `src/lib.rs` → `crates/logzip-py/src/lib.rs` (обновить импорты) |
| Create | `crates/logzip-mcp/Cargo.toml` |
| Create | `crates/logzip-mcp/src/main.rs` |
| Create | `crates/logzip-mcp/src/sandbox.rs` |
| Create | `crates/logzip-mcp/src/tools.rs` |
| Create | `crates/logzip-mcp/src/mcp.rs` |
| Create | `tests/smoke_mcp.py` |
| Modify | `README.md` → добавить MCP секцию |

---

## Task 1: Cargo Workspace skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/logzip-core/Cargo.toml`
- Create: `crates/logzip-py/Cargo.toml`
- Create: `crates/logzip-mcp/Cargo.toml`

- [ ] **Step 1: Создать директории**

```bash
mkdir -p crates/logzip-core/src
mkdir -p crates/logzip-py/src
mkdir -p crates/logzip-mcp/src
```

- [ ] **Step 2: Заменить корневой Cargo.toml**

Полностью заменить содержимое `Cargo.toml`:

```toml
[workspace]
members = [
    "crates/logzip-core",
    "crates/logzip-py",
    "crates/logzip-mcp",
]
resolver = "2"

[workspace.package]
version = "1.1.0"
edition = "2021"
license = "MIT"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

- [ ] **Step 3: Создать crates/logzip-core/Cargo.toml**

```toml
[package]
name = "logzip-core"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Core compression engine for logzip"

[lib]
crate-type = ["rlib"]

[dependencies]
regex = "1"
rayon = "1.10"
aho-corasick = "1"

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "benches"
harness = false
```

- [ ] **Step 4: Создать crates/logzip-py/Cargo.toml**

```toml
[package]
name = "logzip-py"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Python bindings for logzip"

[lib]
name = "_logzip"
crate-type = ["cdylib"]

[dependencies]
logzip-core = { path = "../logzip-core" }
pyo3 = { version = "0.22", features = ["extension-module", "abi3-py39"] }
```

- [ ] **Step 5: Создать crates/logzip-mcp/Cargo.toml**

```toml
[package]
name = "logzip"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Compress logs for LLM analysis — CLI + MCP server"
keywords = ["log", "compression", "llm", "mcp"]

[[bin]]
name = "logzip"
path = "src/main.rs"

[dependencies]
logzip-core = { path = "../logzip-core" }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
```

- [ ] **Step 6: Проверить workspace**

```bash
cargo metadata --no-deps --format-version 1 | grep '"name"'
```

Ожидается вывод с тремя именами: `logzip-core`, `logzip-py`, `logzip`.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/
git commit -m "chore: init cargo workspace with three crates"
```

---

## Task 2: logzip-core — перенос и публикация API

**Files:**
- Create: `crates/logzip-core/src/lib.rs`
- Move: `src/compress.rs` → `crates/logzip-core/src/compress.rs` (добавить render + PREAMBLE)
- Move: `src/legend.rs` → `crates/logzip-core/src/legend.rs`
- Move: `src/base62.rs` → `crates/logzip-core/src/base62.rs`
- Move: `src/normalizer.rs` → `crates/logzip-core/src/normalizer.rs`
- Move: `src/profiles.rs` → `crates/logzip-core/src/profiles.rs`
- Move: `src/templates.rs` → `crates/logzip-core/src/templates.rs`

- [ ] **Step 1: Скопировать файлы из src/**

```bash
cp src/compress.rs   crates/logzip-core/src/compress.rs
cp src/legend.rs     crates/logzip-core/src/legend.rs
cp src/base62.rs     crates/logzip-core/src/base62.rs
cp src/normalizer.rs crates/logzip-core/src/normalizer.rs
cp src/profiles.rs   crates/logzip-core/src/profiles.rs
cp src/templates.rs  crates/logzip-core/src/templates.rs
```

Внутри скопированных файлов `use crate::...` ссылки остаются без изменений — они по-прежнему внутри одного крейта.

- [ ] **Step 2: Добавить PREAMBLE и render() в crates/logzip-core/src/compress.rs**

В начало файла (после `use` блоков) добавить константу:

```rust
pub const PREAMBLE: &str = "\
# logzip/v1 — compressed log. Decode rules:
# #tag#  → replace with value from LEGEND
# &tag:v → replace with LEGEND &tag pattern, substitute @ with v
# PREFIX → prepend to every BODY line (if present)
";
```

В конец файла, после `pub fn decompress(...)`, добавить метод на `CompressResult`:

```rust
impl CompressResult {
    pub fn render(&self, with_preamble: bool) -> String {
        let mut parts: Vec<String> = Vec::new();
        if with_preamble {
            parts.push(PREAMBLE.to_string());
        }
        if !self.common_prefix.is_empty() {
            parts.push(format!("--- PREFIX ---\n{}", self.common_prefix));
        }
        if !self.legend.is_empty() || !self.templates.is_empty() {
            parts.push("--- LEGEND ---".to_string());
            for e in &self.legend {
                parts.push(format!("#{tag}# = {value}", tag = e.tag, value = e.value));
            }
            for t in &self.templates {
                parts.push(format!("&{tag} = {pattern}", tag = t.tag, pattern = t.pattern));
            }
        }
        parts.push("--- BODY ---".to_string());
        parts.push(self.body.clone());
        parts.join("\n")
    }
}
```

- [ ] **Step 3: Создать crates/logzip-core/src/lib.rs**

```rust
pub mod base62;
pub mod compress;
pub mod legend;
pub mod normalizer;
pub mod profiles;
pub mod templates;

pub use compress::{compress, decompress, CompressResult, PREAMBLE};
```

- [ ] **Step 4: Проверить компиляцию logzip-core**

```bash
cargo build -p logzip-core 2>&1
```

Ожидается: `Compiling logzip-core v1.1.0` без ошибок.

- [ ] **Step 5: Commit**

```bash
git add crates/logzip-core/
git commit -m "feat: extract logzip-core crate with render() and PREAMBLE"
```

---

## Task 3: logzip-py — обновить импорты, подключить к maturin

**Files:**
- Create: `crates/logzip-py/src/lib.rs`
- Modify: `pyproject.toml`

- [ ] **Step 1: Создать crates/logzip-py/src/lib.rs**

Это бывший `src/lib.rs`. Три изменения: убрать `mod`-декларации модулей, заменить импорты на `logzip_core`, убрать PREAMBLE (она теперь в core).

```rust
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;

use logzip_core::compress::{compress as core_compress, decompress as core_decompress, CompressResult, PREAMBLE};

/// Python-exposed compression result.
#[pyclass(name = "CompressResult")]
#[derive(Clone)]
pub struct PyCompressResult {
    #[pyo3(get)]
    body: String,
    #[pyo3(get)]
    legend: Vec<(String, String)>,
    #[pyo3(get)]
    templates: Vec<(String, String)>,
    #[pyo3(get)]
    common_prefix: String,
    #[pyo3(get)]
    detected_profile: String,
    stats_raw: HashMap<String, String>,
}

#[pymethods]
impl PyCompressResult {
    #[pyo3(signature = (with_preamble = false))]
    fn render(&self, with_preamble: bool) -> String {
        // Делегируем в core render через временный CompressResult не нужен —
        // используем PREAMBLE напрямую, логика рендеринга здесь минимальна.
        let mut parts: Vec<String> = Vec::new();
        if with_preamble {
            parts.push(PREAMBLE.to_string());
        }
        if !self.common_prefix.is_empty() {
            parts.push(format!("--- PREFIX ---\n{}", self.common_prefix));
        }
        if !self.legend.is_empty() || !self.templates.is_empty() {
            parts.push("--- LEGEND ---".to_string());
            for (tag, value) in &self.legend {
                parts.push(format!("#{tag}# = {value}"));
            }
            for (tag, pattern) in &self.templates {
                parts.push(format!("&{tag} = {pattern}"));
            }
        }
        parts.push("--- BODY ---".to_string());
        parts.push(self.body.clone());
        parts.join("\n")
    }

    fn stats_str(&self) -> String {
        let s = &self.stats_raw;
        let profile = s.get("profile").map(|v| v.as_str()).unwrap_or("?");
        let orig = s.get("original_chars").map(|v| v.as_str()).unwrap_or("?");
        let comp = s.get("compressed_chars").map(|v| v.as_str()).unwrap_or("?");
        let ratio = s.get("ratio_pct").map(|v| v.as_str()).unwrap_or("?");
        let entries = s.get("legend_entries").map(|v| v.as_str()).unwrap_or("0");
        let tmpl = s.get("template_entries").map(|v| v.as_str()).unwrap_or("0");
        format!(
            "[logzip] profile={profile} | {orig} → {comp} chars ({ratio}% saved) | legend={entries} tmpl={tmpl}"
        )
    }

    fn stats<'py>(&self, py: Python<'py>) -> Bound<'py, PyDict> {
        let d = PyDict::new_bound(py);
        for (k, v) in &self.stats_raw {
            if let Ok(i) = v.parse::<i64>() {
                d.set_item(k, i).unwrap();
            } else if let Ok(f) = v.parse::<f64>() {
                d.set_item(k, f).unwrap();
            } else {
                d.set_item(k, v).unwrap();
            }
        }
        d
    }

    fn __repr__(&self) -> String {
        self.stats_str()
    }
}

impl From<CompressResult> for PyCompressResult {
    fn from(r: CompressResult) -> Self {
        Self {
            body: r.body,
            legend: r.legend.into_iter().map(|e| (e.tag, e.value)).collect(),
            templates: r.templates.into_iter().map(|t| (t.tag, t.pattern)).collect(),
            common_prefix: r.common_prefix,
            detected_profile: r.detected_profile,
            stats_raw: r.stats,
        }
    }
}

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
        &text, max_ngram, max_legend_entries, do_normalize,
        profile.as_deref(), do_templates, bpe_passes,
    );
    Ok(PyCompressResult::from(result))
}

#[pyfunction]
fn decompress_log(rendered: String) -> PyResult<String> {
    core_decompress(&rendered)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))
}

#[pymodule]
fn _logzip(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compress_log, m)?)?;
    m.add_function(wrap_pyfunction!(decompress_log, m)?)?;
    m.add_class::<PyCompressResult>()?;
    m.add("__version__", "1.1.0")?;
    Ok(())
}
```

- [ ] **Step 2: Обновить pyproject.toml**

Добавить `manifest-path` в секцию `[tool.maturin]` и переименовать Python-скрипт:

```toml
[tool.maturin]
manifest-path = "crates/logzip-py/Cargo.toml"
python-source = "python"
module-name = "logzip._logzip"
features = ["pyo3/extension-module"]
```

В секции `[project.scripts]` заменить:
```toml
[project.scripts]
logzip-py = "logzip.__main__:main"
```

(Было `logzip`, стало `logzip-py` — иначе конфликт с Rust-бинарником.)

- [ ] **Step 3: Пересобрать Python-модуль**

```bash
maturin develop --release 2>&1
```

Ожидается: `📦 Built wheel for CPython ... logzip-1.1.0-...` без ошибок.

- [ ] **Step 4: Прогнать все тесты — должно быть 15/15**

```bash
pytest tests/ -v 2>&1
```

Ожидается: `15 passed`.

- [ ] **Step 5: Удалить старую src/**

```bash
git rm -r src/
```

- [ ] **Step 6: Commit**

```bash
git add crates/logzip-py/ pyproject.toml
git commit -m "feat: extract logzip-py crate, wire maturin to crates/logzip-py"
```

---

## Task 4: sandbox.rs — TDD (path validation)

**Files:**
- Create: `crates/logzip-mcp/src/sandbox.rs`

- [ ] **Step 1: Написать failing тесты**

Создать `crates/logzip-mcp/src/sandbox.rs` с заглушкой и тестами:

```rust
use std::path::PathBuf;

pub struct Sandbox {
    pub allowed: Vec<PathBuf>,
}

impl Sandbox {
    pub fn new(_dirs: Vec<PathBuf>) -> Result<Self, String> {
        unimplemented!()
    }

    pub fn validate(&self, _path: &str) -> Result<PathBuf, String> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::env;

    fn make_temp_file(dir: &PathBuf, name: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, b"test log content").unwrap();
        p
    }

    #[test]
    fn test_path_inside_allowed_dir() {
        let tmp = env::temp_dir();
        let file = make_temp_file(&tmp, "logzip_test_valid.log");
        let sandbox = Sandbox::new(vec![tmp.clone()]).unwrap();
        assert!(sandbox.validate(file.to_str().unwrap()).is_ok());
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn test_path_outside_allowed_dir() {
        let tmp = env::temp_dir();
        let subdir = tmp.join("logzip_sandbox_allowed");
        fs::create_dir_all(&subdir).unwrap();
        let file = make_temp_file(&tmp, "logzip_test_outside.log");

        let sandbox = Sandbox::new(vec![subdir.clone()]).unwrap();
        assert!(sandbox.validate(file.to_str().unwrap()).is_err());

        fs::remove_file(file).unwrap();
        fs::remove_dir(subdir).unwrap();
    }

    #[test]
    fn test_path_traversal_rejected() {
        let tmp = env::temp_dir();
        let allowed = tmp.join("logzip_sandbox_traversal_test");
        fs::create_dir_all(&allowed).unwrap();
        let sandbox = Sandbox::new(vec![allowed.clone()]).unwrap();

        // Попытка path traversal — canonicalize упадёт (файл не существует)
        // или вернёт путь вне allowed
        let traversal = format!("{}/../../etc/passwd", allowed.display());
        assert!(sandbox.validate(&traversal).is_err());

        fs::remove_dir_all(allowed).unwrap();
    }

    #[test]
    fn test_empty_dirs_defaults_to_cwd() {
        let sandbox = Sandbox::new(vec![]).unwrap();
        let expected = std::fs::canonicalize(env::current_dir().unwrap()).unwrap();
        assert_eq!(sandbox.allowed.len(), 1);
        assert_eq!(sandbox.allowed[0], expected);
    }

    #[test]
    fn test_invalid_allow_dir_returns_error() {
        let result = Sandbox::new(vec![PathBuf::from("/nonexistent/path/xyz")]);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Убедиться что тесты падают**

Создать временный `crates/logzip-mcp/src/main.rs` чтобы крейт компилировался:

```rust
mod sandbox;
fn main() {}
```

```bash
cargo test -p logzip --lib 2>&1 | head -20
```

Ожидается: `FAILED` с `not implemented`.

- [ ] **Step 3: Реализовать Sandbox**

Заменить заглушки реальной реализацией в `crates/logzip-mcp/src/sandbox.rs`:

```rust
use std::path::PathBuf;

pub struct Sandbox {
    pub allowed: Vec<PathBuf>,
}

impl Sandbox {
    pub fn new(dirs: Vec<PathBuf>) -> Result<Self, String> {
        let targets = if dirs.is_empty() {
            vec![std::env::current_dir().map_err(|e| e.to_string())?]
        } else {
            dirs
        };

        let mut allowed = Vec::new();
        for dir in targets {
            // canonicalize базовых путей обязателен: на Windows canonicalize возвращает
            // UNC-пути (\\?\C:\...), поэтому сравниваем яблоки с яблоками.
            match std::fs::canonicalize(&dir) {
                Ok(canon) => allowed.push(canon),
                Err(e) => return Err(format!("Invalid --allow-dir {:?}: {}", dir, e)),
            }
        }
        Ok(Self { allowed })
    }

    pub fn validate(&self, path: &str) -> Result<PathBuf, String> {
        let canonical = std::fs::canonicalize(path)
            .map_err(|e| format!("Cannot resolve path '{}': {}", path, e))?;

        if self.allowed.iter().any(|a| canonical.starts_with(a)) {
            Ok(canonical)
        } else {
            Err(format!("Path '{}' is outside allowed directories", path))
        }
    }
}

#[cfg(test)]
mod tests {
    // ... (те же тесты что выше)
}
```

- [ ] **Step 4: Запустить тесты — все должны пройти**

```bash
cargo test -p logzip --lib sandbox 2>&1
```

Ожидается: `5 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/logzip-mcp/
git commit -m "feat: sandbox.rs — path validation with canonicalize + TDD"
```

---

## Task 5: tools.rs — compress_file, compress_tail, get_stats

**Files:**
- Create: `crates/logzip-mcp/src/tools.rs`
- Modify: `crates/logzip-mcp/src/main.rs` (добавить `mod tools;`)

- [ ] **Step 1: Написать тесты для tools.rs**

`RpcError` определяется в самом `tools.rs` (mcp.rs зависит от tools, не наоборот).

```rust
// crates/logzip-mcp/src/tools.rs
use serde_json::{json, Value};
use std::path::Path;
use logzip_core::compress;
use crate::sandbox::Sandbox;

// RpcError определён здесь — mcp.rs импортирует его отсюда.
#[derive(serde::Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

// ─── Публичные функции ────────────────────────────────────────────────────────

pub fn list() -> Result<Value, RpcError> {
    unimplemented!()
}

pub fn call(params: Option<&Value>, sandbox: &Sandbox) -> Result<Value, RpcError> {
    unimplemented!()
}

pub fn compress_tail_internal(path: &Path, lines: usize, quality: &str) -> Result<String, RpcError> {
    unimplemented!()
}

// ─── Внутренние хелперы ───────────────────────────────────────────────────────

fn content_text(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn compress_file_impl(path: &Path, quality: &str) -> Result<Value, String> {
    unimplemented!()
}

fn compress_tail_impl(path: &Path, lines: usize, quality: &str) -> Result<Value, String> {
    unimplemented!()
}

fn get_stats_impl(path: &Path) -> Result<Value, String> {
    unimplemented!()
}

fn read_tail(path: &Path, n_lines: usize) -> std::io::Result<String> {
    unimplemented!()
}

fn quality_params(quality: &str) -> (usize, usize) {
    match quality {
        "max"      => (512, 2),
        "balanced" => (128, 1),
        _          => (32,  1),  // "fast" и дефолт
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::env;

    fn sample_log(n: usize) -> String {
        (0..n).map(|i| format!("2024-01-01T00:00:{:02}Z INFO request id={} status=200\n", i % 60, i))
              .collect()
    }

    #[test]
    fn test_compress_file_returns_content_array() {
        let tmp = env::temp_dir().join("logzip_tools_test_cf.log");
        fs::write(&tmp, sample_log(50)).unwrap();
        let result = compress_file_impl(&tmp, "fast").unwrap();
        assert!(result["content"].is_array());
        assert_eq!(result["content"][0]["type"], "text");
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("BODY"));
        fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn test_compress_tail_returns_last_n_lines() {
        let tmp = env::temp_dir().join("logzip_tools_test_ct.log");
        let log = sample_log(200);
        fs::write(&tmp, &log).unwrap();
        let result = compress_tail_impl(&tmp, 10, "fast").unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("BODY"));
        fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn test_get_stats_fields() {
        let tmp = env::temp_dir().join("logzip_tools_test_gs.log");
        fs::write(&tmp, sample_log(100)).unwrap();
        let result = get_stats_impl(&tmp).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let stats: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(stats["file_size_bytes"].as_u64().unwrap() > 0);
        assert!(stats["estimated_tokens"].as_u64().unwrap() > 0);
        assert!(stats["recommended_tool"].as_str().is_some());
        fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn test_read_tail_exact_lines() {
        let tmp = env::temp_dir().join("logzip_tools_test_rt.log");
        let lines: Vec<String> = (0..100).map(|i| format!("line {}", i)).collect();
        fs::write(&tmp, lines.join("\n") + "\n").unwrap();
        let tail = read_tail(&tmp, 10).unwrap();
        let tail_lines: Vec<&str> = tail.trim_end().split('\n').collect();
        assert_eq!(tail_lines.len(), 10);
        assert_eq!(tail_lines[0], "line 90");
        assert_eq!(tail_lines[9], "line 99");
        fs::remove_file(tmp).unwrap();
    }
}
```

- [ ] **Step 2: Запустить тесты — убедиться что падают**

```bash
cargo test -p logzip --lib tools 2>&1 | grep -E "FAILED|error"
```

Ожидается: `FAILED` (unimplemented).

- [ ] **Step 3: Реализовать tools.rs**

Заменить содержимое `crates/logzip-mcp/src/tools.rs`:

```rust
use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use logzip_core::compress;
use crate::sandbox::Sandbox;

#[derive(serde::Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

pub fn list() -> Result<Value, RpcError> {
    Ok(json!({
        "tools": [
            {
                "name": "compress_file",
                "description": "Compress an entire log file using logzip and return the result ready for LLM analysis.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path":    { "type": "string", "description": "Absolute path to the log file" },
                        "quality": { "type": "string", "enum": ["fast", "balanced", "max"], "default": "balanced" }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "compress_tail",
                "description": "Compress only the last N lines of a log file. Efficient for large files — does not load the entire file into memory.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path":    { "type": "string", "description": "Absolute path to the log file" },
                        "lines":   { "type": "integer", "minimum": 1, "default": 500, "description": "Number of tail lines to compress" },
                        "quality": { "type": "string", "enum": ["fast", "balanced", "max"], "default": "balanced" }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "get_stats",
                "description": "Return file metadata and token estimates without compressing. Call this first to decide whether to use compress_file or compress_tail.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to the log file" }
                    },
                    "required": ["path"]
                }
            }
        ]
    }))
}

pub fn call(params: Option<&Value>, sandbox: &Sandbox) -> Result<Value, RpcError> {
    let params = params.ok_or_else(|| RpcError { code: -32602, message: "Missing params".into() })?;
    let name = params["name"].as_str()
        .ok_or_else(|| RpcError { code: -32602, message: "Missing tool name".into() })?;
    let args = &params["arguments"];

    let path_str = args["path"].as_str()
        .ok_or_else(|| RpcError { code: -32602, message: "Missing required argument: path".into() })?;

    let path = sandbox.validate(path_str)
        .map_err(|e| RpcError { code: -32602, message: e })?;

    match name {
        "compress_file" => {
            let quality = args["quality"].as_str().unwrap_or("balanced");
            compress_file_impl(&path, quality).map_err(|e| RpcError { code: -32603, message: e })
        }
        "compress_tail" => {
            let lines = args["lines"].as_u64().unwrap_or(500) as usize;
            let quality = args["quality"].as_str().unwrap_or("balanced");
            compress_tail_impl(&path, lines, quality).map_err(|e| RpcError { code: -32603, message: e })
        }
        "get_stats" => {
            get_stats_impl(&path).map_err(|e| RpcError { code: -32603, message: e })
        }
        _ => Err(RpcError { code: -32602, message: format!("Unknown tool: {}", name) })
    }
}

// ─── Внутренние реализации ────────────────────────────────────────────────────

fn content_text(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn quality_params(quality: &str) -> (usize, usize) {
    match quality {
        "max"      => (512, 2),
        "balanced" => (128, 1),
        _          => (32,  1),
    }
}

fn compress_file_impl(path: &Path, quality: &str) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file: {}", e))?;
    let (max_legend, bpe_passes) = quality_params(quality);
    let result = compress(&text, 2, max_legend, true, None, true, bpe_passes);
    Ok(content_text(result.render(true)))
}

fn compress_tail_impl(path: &Path, lines: usize, quality: &str) -> Result<Value, String> {
    let text = read_tail(path, lines)
        .map_err(|e| format!("Cannot read file tail: {}", e))?;
    let (max_legend, bpe_passes) = quality_params(quality);
    let result = compress(&text, 2, max_legend, true, None, true, bpe_passes);
    Ok(content_text(result.render(true)))
}

fn get_stats_impl(path: &Path) -> Result<Value, String> {
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("Cannot stat file: {}", e))?;
    let file_size = meta.len();
    let estimated_tokens = file_size / 4;
    let file_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Определяем профиль через mini-compress первых 4096 байт.
    // profiles::auto_detect может быть pub(crate) — безопаснее использовать compress().
    let mut sample_buf = vec![0u8; 4096.min(file_size as usize)];
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    f.read(&mut sample_buf).map_err(|e| e.to_string())?;
    let sample_str = String::from_utf8_lossy(&sample_buf);
    let mini = logzip_core::compress(&sample_str, 1, 1, false, None, false, 1);
    let detected_profile = mini.detected_profile;

    let recommended_tool = if estimated_tokens > 50_000 {
        "compress_tail"
    } else {
        "compress_file"
    };

    let stats = json!({
        "file_size_bytes":   file_size,
        "estimated_tokens":  estimated_tokens,
        "detected_profile":  detected_profile,
        "file_name":         file_name,
        "recommended_tool":  recommended_tool
    });

    Ok(content_text(serde_json::to_string_pretty(&stats).unwrap()))
}

fn read_tail(path: &Path, n_lines: usize) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let size = file.seek(SeekFrom::End(0))?;
    if size == 0 {
        return Ok(String::new());
    }

    const CHUNK: u64 = 4096;
    let mut newlines = 0usize;
    let mut scan_pos = size;
    let mut found_pos = 0u64;

    'outer: loop {
        let chunk_end = scan_pos;
        let chunk_start = scan_pos.saturating_sub(CHUNK);
        let chunk_size = (chunk_end - chunk_start) as usize;

        file.seek(SeekFrom::Start(chunk_start))?;
        let mut buf = vec![0u8; chunk_size];
        file.read_exact(&mut buf)?;

        for i in (0..chunk_size).rev() {
            if buf[i] == b'\n' {
                newlines += 1;
                if newlines > n_lines {
                    found_pos = chunk_start + i as u64 + 1;
                    break 'outer;
                }
            }
        }

        if chunk_start == 0 {
            break;
        }
        scan_pos = chunk_start;
    }

    file.seek(SeekFrom::Start(found_pos))?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::env;

    fn sample_log(n: usize) -> String {
        (0..n).map(|i| format!("2024-01-01T00:00:{:02}Z INFO request id={} status=200\n", i % 60, i))
              .collect()
    }

    #[test]
    fn test_compress_file_returns_content_array() {
        let tmp = env::temp_dir().join("logzip_tools_test_cf.log");
        fs::write(&tmp, sample_log(50)).unwrap();
        let result = compress_file_impl(&tmp, "fast").unwrap();
        assert!(result["content"].is_array());
        assert_eq!(result["content"][0]["type"], "text");
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("BODY"));
        fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn test_compress_tail_returns_last_n_lines() {
        let tmp = env::temp_dir().join("logzip_tools_test_ct.log");
        fs::write(&tmp, sample_log(200)).unwrap();
        let result = compress_tail_impl(&tmp, 10, "fast").unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("BODY"));
        fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn test_get_stats_fields() {
        let tmp = env::temp_dir().join("logzip_tools_test_gs.log");
        fs::write(&tmp, sample_log(100)).unwrap();
        let result = get_stats_impl(&tmp).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let stats: Value = serde_json::from_str(text).unwrap();
        assert!(stats["file_size_bytes"].as_u64().unwrap() > 0);
        assert!(stats["estimated_tokens"].as_u64().unwrap() > 0);
        assert!(stats["recommended_tool"].as_str().is_some());
        fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn test_read_tail_exact_lines() {
        let tmp = env::temp_dir().join("logzip_tools_test_rt.log");
        let lines: Vec<String> = (0..100).map(|i| format!("line {}", i)).collect();
        fs::write(&tmp, lines.join("\n") + "\n").unwrap();
        let tail = read_tail(&tmp, 10).unwrap();
        let tail_lines: Vec<&str> = tail.trim_end().split('\n').collect();
        assert_eq!(tail_lines.len(), 10);
        assert_eq!(tail_lines[0], "line 90");
        assert_eq!(tail_lines[9], "line 99");
        fs::remove_file(tmp).unwrap();
    }
}
```

- [ ] **Step 4: Запустить тесты — все должны пройти**

```bash
cargo test -p logzip --lib tools 2>&1
```

Ожидается: `4 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/logzip-mcp/src/tools.rs
git commit -m "feat: tools.rs — compress_file, compress_tail, get_stats with TDD"
```

---

## Task 6: mcp.rs — JSON-RPC 2.0 loop

**Files:**
- Create: `crates/logzip-mcp/src/mcp.rs`

- [ ] **Step 1: Написать тесты для JSON-RPC структур**

```rust
// crates/logzip-mcp/src/mcp.rs
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::sandbox::Sandbox;
use crate::tools;

#[derive(Deserialize)]
struct Request {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct Response {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<tools::RpcError>,
}

pub fn run(_sandbox: Sandbox) {
    unimplemented!()
}

fn handle_request(method: &str, params: Option<&Value>, sandbox: &Sandbox) -> Result<Value, tools::RpcError> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_has_no_id() {
        let json = r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert!(req.id.is_none());
    }

    #[test]
    fn test_request_with_integer_id() {
        let json = r#"{"jsonrpc":"2.0","id":42,"method":"tools/list"}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, Some(json!(42)));
    }

    #[test]
    fn test_request_with_string_id() {
        let json = r#"{"jsonrpc":"2.0","id":"abc-123","method":"ping"}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, Some(json!("abc-123")));
    }

    #[test]
    fn test_response_no_newlines_in_output() {
        let resp = Response {
            jsonrpc: "2.0",
            id: json!(1),
            result: Some(json!({"tools": []})),
            error: None,
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(!s.contains('\n'), "Response must be single-line: {}", s);
    }

    #[test]
    fn test_error_response_omits_result() {
        let resp = Response {
            jsonrpc: "2.0",
            id: json!(1),
            result: None,
            error: Some(tools::RpcError { code: -32601, message: "not found".into() }),
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(!s.contains("\"result\""), "result should be absent: {}", s);
        assert!(s.contains("\"error\""));
    }
}
```

- [ ] **Step 2: Запустить — убедиться что тесты по структурам проходят, run падает**

```bash
cargo test -p logzip --lib mcp 2>&1 | grep -E "test .* (ok|FAILED)"
```

Ожидается: 4 структурных теста pass, `run` не вызывается.

- [ ] **Step 3: Реализовать mcp.rs полностью**

```rust
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::BufRead;
use crate::sandbox::Sandbox;
use crate::tools;

#[derive(Deserialize)]
struct Request {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct Response {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<tools::RpcError>,
}

pub fn run(sandbox: Sandbox) {
    let stdin = std::io::stdin();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut line = String::new();

    eprintln!("[logzip-mcp] ready, allowed dirs: {:?}", sandbox.allowed);

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => { eprintln!("[logzip-mcp] read error: {}", e); break; }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[logzip-mcp] parse error: {}", e);
                let resp = Response {
                    jsonrpc: "2.0",
                    id: Value::Null,
                    result: None,
                    error: Some(tools::RpcError { code: -32700, message: format!("Parse error: {}", e) }),
                };
                println!("{}", serde_json::to_string(&resp).unwrap());
                continue;
            }
        };

        // Notification (нет id) — обработать, не отвечать
        if request.id.is_none() {
            eprintln!("[logzip-mcp] notification: {}", request.method);
            continue;
        }

        let id = request.id.clone().unwrap();
        let result = handle_request(&request.method, request.params.as_ref(), &sandbox);

        let response = match result {
            Ok(r)  => Response { jsonrpc: "2.0", id, result: Some(r), error: None },
            Err(e) => Response { jsonrpc: "2.0", id, result: None, error: Some(e) },
        };

        println!("{}", serde_json::to_string(&response).unwrap());
    }
}

fn handle_request(method: &str, params: Option<&Value>, sandbox: &Sandbox) -> Result<Value, tools::RpcError> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {}, "prompts": {} },
            "serverInfo": { "name": "logzip", "version": env!("CARGO_PKG_VERSION") }
        })),

        "ping" => Ok(json!({})),

        "tools/list" => tools::list(),

        "tools/call" => tools::call(params, sandbox),

        "prompts/list" => Ok(json!({
            "prompts": [{
                "name": "analyze_logs",
                "description": "Compress and prepare a log file for SRE analysis",
                "arguments": [
                    { "name": "path",  "description": "Path to log file", "required": true },
                    { "name": "lines", "description": "Tail lines to compress (default: 500)", "required": false }
                ]
            }]
        })),

        "prompts/get" => {
            let params = params.ok_or_else(|| tools::RpcError { code: -32602, message: "Missing params".into() })?;
            if params["name"].as_str() != Some("analyze_logs") {
                return Err(tools::RpcError { code: -32602, message: "Unknown prompt".into() });
            }
            let path_str = params["arguments"]["path"].as_str()
                .ok_or_else(|| tools::RpcError { code: -32602, message: "Missing argument: path".into() })?;
            let lines = params["arguments"]["lines"].as_u64().unwrap_or(500) as usize;

            let path = sandbox.validate(path_str)
                .map_err(|e| tools::RpcError { code: -32602, message: e })?;

            // Сервер сам делает компрессию и отдаёт готовый контекст
            let text = {
                use std::io::Read;
                let mut tail_result = crate::tools::compress_tail_internal(&path, lines, "balanced")?;
                tail_result
            };

            Ok(json!({
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Ты — эксперт SRE. Перед тобой сжатый лог в формате logzip/v1.\n\
                             Сначала изучи секцию LEGEND — она содержит словарь замен.\n\
                             Затем читай BODY, ища аномалии, ошибки и паттерны.\n\n\
                             <compressed_log>\n{}\n</compressed_log>",
                            text
                        )
                    }
                }]
            }))
        }

        _ => Err(tools::RpcError { code: -32601, message: format!("Method not found: {}", method) }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_has_no_id() {
        let json = r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert!(req.id.is_none());
    }

    #[test]
    fn test_request_with_integer_id() {
        let json = r#"{"jsonrpc":"2.0","id":42,"method":"tools/list"}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, Some(json!(42)));
    }

    #[test]
    fn test_request_with_string_id() {
        let json = r#"{"jsonrpc":"2.0","id":"abc-123","method":"ping"}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, Some(json!("abc-123")));
    }

    #[test]
    fn test_response_no_newlines_in_output() {
        let resp = Response {
            jsonrpc: "2.0",
            id: json!(1),
            result: Some(json!({"tools": []})),
            error: None,
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(!s.contains('\n'), "Response must be single-line: {}", s);
    }

    #[test]
    fn test_error_response_omits_result() {
        let resp = Response {
            jsonrpc: "2.0",
            id: json!(1),
            result: None,
            error: Some(tools::RpcError { code: -32601, message: "not found".into() }),
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(!s.contains("\"result\""), "result should be absent: {}", s);
        assert!(s.contains("\"error\""));
    }
}
```

**Важно:** `prompts/get` вызывает `crate::tools::compress_tail_internal`. Нужно добавить эту pub-функцию в `tools.rs`:

```rust
// Добавить в tools.rs
pub fn compress_tail_internal(path: &std::path::Path, lines: usize, quality: &str) -> Result<String, RpcError> {
    let text = read_tail(path, lines)
        .map_err(|e| RpcError { code: -32603, message: format!("Cannot read tail: {}", e) })?;
    let (max_legend, bpe_passes) = quality_params(quality);
    let result = logzip_core::compress(&text, 2, max_legend, true, None, true, bpe_passes);
    Ok(result.render(true))
}
```

- [ ] **Step 4: Запустить тесты mcp.rs**

```bash
cargo test -p logzip --lib mcp 2>&1
```

Ожидается: `5 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/logzip-mcp/src/mcp.rs crates/logzip-mcp/src/tools.rs
git commit -m "feat: mcp.rs — JSON-RPC 2.0 loop with all protocol methods"
```

---

## Task 7: main.rs — CLI dispatcher

**Files:**
- Modify: `crates/logzip-mcp/src/main.rs`

- [ ] **Step 1: Написать полный main.rs**

```rust
mod mcp;
mod sandbox;
mod tools;

use std::path::PathBuf;
use sandbox::Sandbox;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match cmd {
        "compress"   => cmd_compress(&args[2..]),
        "decompress" => cmd_decompress(&args[2..]),
        "mcp"        => cmd_mcp(&args[2..]),
        "--version" | "-V" => {
            println!("logzip {}", env!("CARGO_PKG_VERSION"));
        }
        _ => {
            eprintln!("logzip {}", env!("CARGO_PKG_VERSION"));
            eprintln!("Usage:");
            eprintln!("  logzip compress   -i <file> [-o <file>] [--quality fast|balanced|max] [--bpe-passes N] [--preamble] [--stats]");
            eprintln!("  logzip decompress -i <file> [-o <file>]");
            eprintln!("  logzip mcp        [--allow-dir <dir>]...");
            if cmd != "help" && cmd != "--help" && cmd != "-h" {
                std::process::exit(1);
            }
        }
    }
}

fn cmd_compress(args: &[String]) {
    let mut input_path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut quality = "balanced";
    let mut bpe_passes: Option<usize> = None;
    let mut preamble = false;
    let mut stats = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-i" | "--input"      => { i += 1; input_path = Some(args[i].clone()); }
            "-o" | "--output"     => { i += 1; output_path = Some(args[i].clone()); }
            "--quality"           => { i += 1; quality = Box::leak(args[i].clone().into_boxed_str()); }
            "--bpe-passes"        => { i += 1; bpe_passes = args[i].parse().ok(); }
            "--preamble"          => { preamble = true; }
            "--stats"             => { stats = true; }
            _ => {}
        }
        i += 1;
    }

    let quality_map: &[(&str, usize, usize)] = &[
        ("fast",     32,  1),
        ("balanced", 128, 1),
        ("max",      512, 2),
    ];
    let (max_legend, mut passes) = quality_map.iter()
        .find(|(q, _, _)| *q == quality)
        .map(|(_, l, p)| (*l, *p))
        .unwrap_or((128, 1));
    if let Some(p) = bpe_passes { passes = p; }

    let text = match input_path {
        Some(ref p) => std::fs::read_to_string(p).unwrap_or_else(|e| { eprintln!("Error: {}", e); std::process::exit(1); }),
        None => { use std::io::Read; let mut s = String::new(); std::io::stdin().read_to_string(&mut s).unwrap(); s }
    };

    let result = logzip_core::compress(&text, 2, max_legend, true, None, true, passes);
    let output = result.render(preamble);

    match output_path {
        Some(ref p) => std::fs::write(p, &output).unwrap_or_else(|e| { eprintln!("Error: {}", e); std::process::exit(1); }),
        None => println!("{}", output),
    }

    if stats {
        let s = &result.stats;
        let orig  = s.get("original_chars").map(|v| v.as_str()).unwrap_or("?");
        let comp  = s.get("compressed_chars").map(|v| v.as_str()).unwrap_or("?");
        let ratio = s.get("ratio_pct").map(|v| v.as_str()).unwrap_or("?");
        eprintln!("[logzip] {} → {} chars ({}% saved)", orig, comp, ratio);
    }
}

fn cmd_decompress(args: &[String]) {
    let mut input_path: Option<String> = None;
    let mut output_path: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-i" | "--input"  => { i += 1; input_path = Some(args[i].clone()); }
            "-o" | "--output" => { i += 1; output_path = Some(args[i].clone()); }
            _ => {}
        }
        i += 1;
    }

    let text = match input_path {
        Some(ref p) => std::fs::read_to_string(p).unwrap_or_else(|e| { eprintln!("Error: {}", e); std::process::exit(1); }),
        None => { use std::io::Read; let mut s = String::new(); std::io::stdin().read_to_string(&mut s).unwrap(); s }
    };

    let output = logzip_core::decompress(&text).unwrap_or_else(|e| { eprintln!("Error: {}", e); std::process::exit(1); });

    match output_path {
        Some(ref p) => std::fs::write(p, &output).unwrap_or_else(|e| { eprintln!("Error: {}", e); std::process::exit(1); }),
        None => println!("{}", output),
    }
}

fn cmd_mcp(args: &[String]) {
    let mut allow_dirs: Vec<PathBuf> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        if args[i] == "--allow-dir" {
            i += 1;
            if i < args.len() {
                allow_dirs.push(PathBuf::from(&args[i]));
            }
        }
        i += 1;
    }

    if allow_dirs.is_empty() {
        eprintln!("[logzip-mcp] No --allow-dir specified, defaulting to CWD");
    }

    let sandbox = Sandbox::new(allow_dirs).unwrap_or_else(|e| {
        eprintln!("[logzip-mcp] Fatal: {}", e);
        std::process::exit(1);
    });

    mcp::run(sandbox);
}
```

- [ ] **Step 2: Собрать бинарник**

```bash
cargo build --release -p logzip 2>&1
```

Ожидается: `Compiling logzip v1.1.0` → `Finished release`.

- [ ] **Step 3: Проверить сабкоманды**

```bash
./target/release/logzip --version
echo "2024-01-01 INFO test" | ./target/release/logzip compress --stats
```

Ожидается: версия и статистика сжатия.

- [ ] **Step 4: Commit**

```bash
git add crates/logzip-mcp/src/main.rs
git commit -m "feat: main.rs — unified CLI with compress, decompress, mcp subcommands"
```

---

## Task 8: Интеграционный smoke test

**Files:**
- Create: `tests/smoke_mcp.py`

- [ ] **Step 1: Создать tests/smoke_mcp.py**

```python
#!/usr/bin/env python3
"""Smoke test for logzip MCP server — sends JSON-RPC via stdin."""
import subprocess
import json
import sys
import os
import tempfile

BINARY = os.path.join(os.path.dirname(__file__), "..", "target", "release", "logzip")

def send_recv(proc, msg):
    line = json.dumps(msg) + "\n"
    proc.stdin.write(line.encode())
    proc.stdin.flush()
    response = proc.stdout.readline()
    assert response, "Server returned empty response"
    return json.loads(response)

def send_notification(proc, msg):
    line = json.dumps(msg) + "\n"
    proc.stdin.write(line.encode())
    proc.stdin.flush()

def main():
    # Создать тестовый лог
    with tempfile.NamedTemporaryFile(mode='w', suffix='.log', dir='/tmp', delete=False) as f:
        for i in range(200):
            f.write(f"2024-01-01T00:{i//60:02}:{i%60:02}Z INFO request id={i} path=/api/v1/users status=200\n")
        tmp_path = f.name

    proc = subprocess.Popen(
        [BINARY, "mcp", "--allow-dir", "/tmp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    try:
        # 1. initialize
        resp = send_recv(proc, {
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test", "version": "0.1"}}
        })
        assert "result" in resp, f"initialize failed: {resp}"
        assert resp["result"]["serverInfo"]["name"] == "logzip"
        print("✓ initialize")

        # 2. notification — сервер не должен отвечать (проверяем через ping)
        send_notification(proc, {"jsonrpc": "2.0", "method": "notifications/initialized"})

        # 3. ping
        resp = send_recv(proc, {"jsonrpc": "2.0", "id": 2, "method": "ping"})
        assert resp.get("result") == {}, f"ping failed: {resp}"
        print("✓ ping")

        # 4. tools/list
        resp = send_recv(proc, {"jsonrpc": "2.0", "id": 3, "method": "tools/list"})
        tool_names = {t["name"] for t in resp["result"]["tools"]}
        assert tool_names == {"compress_file", "compress_tail", "get_stats"}, f"Wrong tools: {tool_names}"
        print("✓ tools/list")

        # 5. get_stats
        resp = send_recv(proc, {
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {"name": "get_stats", "arguments": {"path": tmp_path}}
        })
        stats = json.loads(resp["result"]["content"][0]["text"])
        assert stats["file_size_bytes"] > 0
        assert stats["recommended_tool"] in ("compress_file", "compress_tail")
        print("✓ get_stats")

        # 6. compress_file
        resp = send_recv(proc, {
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": {"name": "compress_file", "arguments": {"path": tmp_path, "quality": "fast"}}
        })
        text = resp["result"]["content"][0]["text"]
        assert "BODY" in text, f"No BODY in compress output"
        print("✓ compress_file")

        # 7. compress_tail
        resp = send_recv(proc, {
            "jsonrpc": "2.0", "id": 6, "method": "tools/call",
            "params": {"name": "compress_tail", "arguments": {"path": tmp_path, "lines": 20, "quality": "fast"}}
        })
        assert "BODY" in resp["result"]["content"][0]["text"]
        print("✓ compress_tail")

        # 8. sandbox violation
        resp = send_recv(proc, {
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": {"name": "get_stats", "arguments": {"path": "/etc/passwd"}}
        })
        assert "error" in resp, f"Sandbox violation should return error: {resp}"
        assert resp["error"]["code"] == -32602
        print("✓ sandbox violation rejected")

        # 9. unknown method → MethodNotFound
        resp = send_recv(proc, {"jsonrpc": "2.0", "id": 8, "method": "nonexistent/method"})
        assert resp.get("error", {}).get("code") == -32601
        print("✓ MethodNotFound")

    finally:
        proc.terminate()
        os.unlink(tmp_path)

    print("\n✅ All MCP smoke tests passed!")

if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Собрать release-бинарник и запустить smoke test**

```bash
cargo build --release -p logzip && python3 tests/smoke_mcp.py
```

Ожидается: `✅ All MCP smoke tests passed!`

- [ ] **Step 3: Прогнать все Python-тесты — проверить отсутствие регрессий**

```bash
maturin develop --release && pytest tests/test_logzip.py -v 2>&1
```

Ожидается: `15 passed`.

- [ ] **Step 4: Commit**

```bash
git add tests/smoke_mcp.py
git commit -m "test: smoke_mcp.py — integration test for MCP server (9 scenarios)"
```

---

## Task 9: README + финальная синхронизация

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Добавить секцию MCP в README.md**

Добавить раздел после существующего раздела про Python API:

```markdown
## MCP Server (Claude Desktop / Claude Code)

Install the Rust binary:

```bash
cargo install logzip
```

Add to your `claude_desktop_config.json`:

- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "logzip": {
      "command": "logzip",
      "args": ["mcp", "--allow-dir", "/var/log", "--allow-dir", "/home/user/logs"]
    }
  }
}
```

Or add via Claude Code CLI:

```bash
claude mcp add logzip -- logzip mcp --allow-dir /var/log
```

### Available tools

| Tool | Description |
|---|---|
| `get_stats(path)` | File size, token estimate, detected profile — call first to decide strategy |
| `compress_file(path, quality)` | Compress entire file — for files < 200K tokens |
| `compress_tail(path, lines, quality)` | Compress last N lines — efficient for large files |

### Available prompts

| Prompt | Description |
|---|---|
| `analyze_logs` | Compresses the log server-side and prepares an SRE analysis context |

### Security

The MCP server only reads files inside directories specified via `--allow-dir`.  
If no `--allow-dir` is given, defaults to the current working directory.  
All paths are canonicalized before comparison to prevent path traversal attacks.
```

- [ ] **Step 2: Финальная проверка — всё вместе**

```bash
cargo test --workspace 2>&1 | tail -5
pytest tests/ -v 2>&1 | tail -5
cargo build --release -p logzip 2>&1 | tail -3
python3 tests/smoke_mcp.py
```

Ожидается: все тесты green, smoke test passed.

- [ ] **Step 3: Финальный commit**

```bash
git add README.md
git commit -m "docs: add MCP server section to README with config and tool table"
```

---

## Self-Review: Spec Coverage

| Требование из спека | Задача |
|---|---|
| Cargo Workspace (logzip-core, logzip-py, logzip-mcp) | Task 1, 2, 3 |
| render() + PREAMBLE в logzip-core | Task 2 |
| PyO3 обёртка без изменений логики | Task 3 |
| maturin manifest-path | Task 3 |
| Rename Python script → logzip-py | Task 3 |
| sandbox.rs с canonicalize на allowed dirs | Task 4 |
| CWD по умолчанию если --allow-dir не указан | Task 4, Task 7 |
| tools/list с JSON Schema + required | Task 5 |
| compress_file, compress_tail, get_stats | Task 5 |
| get_stats возвращает content[type:text] | Task 5 |
| read_tail через backward chunk scan | Task 5 |
| id: Option<Value> в Request | Task 6 |
| ping → {} | Task 6 |
| Одна строка на ответ (no pretty-print) | Task 6 |
| notifications не получают ответа | Task 6 |
| prompts/get — серверная компрессия | Task 6 |
| -32602 для sandbox violation | Task 5, 6 |
| CLI: compress, decompress, mcp subcommands | Task 7 |
| Smoke test с 9 сценариями | Task 8 |
| README с claude_desktop_config.json | Task 9 |
