# logzip MCP Server — Design Spec

**Дата:** 2026-04-23  
**Версия:** 1.0  
**Статус:** Утверждён

---

## Цель

Добавить MCP-сервер к logzip, чтобы Claude мог сжимать логи непосредственно в процессе диалога. Цель — попасть в официальный репозиторий MCP-серверов Anthropic.

---

## Решения

| Вопрос | Решение |
|---|---|
| Структура проекта | Cargo Workspace (3 крейта) |
| MCP-протокол | Ручной JSON-RPC 2.0, без SDK |
| Транспорт | stdio (stdin/stdout) |
| I/O модель | Синхронная (BufReader, без tokio) |
| Бинарник | Unified: `logzip` с сабкомандами |
| Sandboxing | `--allow-dir`, default → CWD |

---

## 1. Структура Workspace

### Файловая структура

```
logzip/files/
├── Cargo.toml                      ← workspace root + workspace.package
├── pyproject.toml                  ← tool.maturin.manifest-path указывает на logzip-py
├── crates/
│   ├── logzip-core/
│   │   ├── Cargo.toml              ← [lib], crate-type = ["rlib"], без pyo3
│   │   └── src/
│   │       ├── lib.rs              ← pub mod re-exports
│   │       ├── compress.rs         ← перенесён из src/
│   │       ├── legend.rs
│   │       ├── base62.rs
│   │       ├── normalizer.rs
│   │       ├── profiles.rs
│   │       └── templates.rs
│   ├── logzip-py/
│   │   ├── Cargo.toml              ← [lib] crate-type = ["cdylib"], depends: logzip-core + pyo3
│   │   └── src/lib.rs              ← текущий src/lib.rs без изменений
│   └── logzip-mcp/
│       ├── Cargo.toml              ← [[bin]] name = "logzip", depends: logzip-core + serde_json + serde
│       └── src/
│           ├── main.rs             ← CLI-диспетчер (аргументы → сабкоманды)
│           ├── mcp.rs              ← JSON-RPC цикл
│           ├── tools.rs            ← обработчики инструментов
│           └── sandbox.rs          ← валидация путей
├── python/logzip/                  ← без изменений
└── tests/                          ← без изменений (pytest)
```

### Корневой Cargo.toml

```toml
[workspace]
members = ["crates/logzip-core", "crates/logzip-py", "crates/logzip-mcp"]
resolver = "2"

[workspace.package]
version = "1.1.0"
edition = "2021"
license = "MIT"
```

Все дочерние крейты используют `version.workspace = true`, `edition.workspace = true`, `license.workspace = true`.

### pyproject.toml — maturin

```toml
[tool.maturin]
manifest-path = "crates/logzip-py/Cargo.toml"
python-source = "python"
module-name = "logzip._logzip"
```

---

## 2. MCP-протокол (mcp.rs)

### Транспорт

- Читаем: `BufReader<Stdin>`, построчно (`read_line`)
- Пишем: `println!("{}", serde_json::to_string(&response)?)` — одна строка, **никакого pretty-print**
- Логи сервера: только `eprintln!("[logzip-mcp] ...")` — никогда в stdout

### Структуры (serde)

```rust
#[derive(Deserialize)]
struct Request {
    jsonrpc: String,
    id: Option<Value>,   // Option! Notifications не имеют id
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct Response {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Serialize)]
struct RpcError {
    code: i32,
    message: String,
}
```

### Логика диспетчера

```
if id == None → Notification, ничего не пишем в stdout
if id == Some → Request, обязаны вернуть Response
```

### Поддерживаемые методы

| Метод | Действие |
|---|---|
| `initialize` | Возвращает server info + capabilities (tools, prompts) |
| `notifications/initialized` | Notification, no-op |
| `ping` | Возвращает `{}` в result (клиент проверяет живость) |
| `tools/list` | Список 3 инструментов с JSON Schema |
| `tools/call` | Диспетчер → tools.rs |
| `prompts/list` | Список промптов |
| `prompts/get` | Сжимает лог на сервере, возвращает готовый контекст |

Неизвестный метод → `-32601 MethodNotFound`.

### Коды ошибок JSON-RPC 2.0

| Код | Значение |
|---|---|
| `-32600` | InvalidRequest |
| `-32601` | MethodNotFound |
| `-32602` | InvalidParams (включая sandbox violation) |
| `-32603` | InternalError |

---

## 3. Инструменты (tools.rs)

Все инструменты возвращают результат в формате:
```json
{ "content": [{ "type": "text", "text": "..." }] }
```

### compress_file

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path":    { "type": "string", "description": "Путь к лог-файлу" },
    "quality": { "type": "string", "enum": ["fast", "balanced", "max"], "default": "balanced" }
  },
  "required": ["path"]
}
```

Алгоритм: `sandbox.validate(path)` → читаем файл → `logzip_core::compress(...)` → `result.render(with_preamble=true)` → возвращаем в `content[0].text`.

### compress_tail

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path":    { "type": "string" },
    "lines":   { "type": "integer", "default": 500, "minimum": 1 },
    "quality": { "type": "string", "enum": ["fast", "balanced", "max"], "default": "balanced" }
  },
  "required": ["path"]
}
```

Алгоритм: читаем файл с конца через обратный итератор по байтам (не грузим весь файл), берём последние N строк → компрессия → возврат.

### get_stats

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string" }
  },
  "required": ["path"]
}
```

Возвращает сериализованный JSON-объект как строку внутри `content[0].text`:
```json
{
  "file_size_bytes": 8347201,
  "estimated_tokens": 2086800,
  "detected_profile": "journalctl",
  "file_name": "app.log",
  "recommended_tool": "compress_tail"
}
```

`estimated_tokens = file_size_bytes / 4` (грубая оценка).  
`recommended_tool`: если `estimated_tokens > 50000` → `"compress_tail"`, иначе `"compress_file"`.

---

## 4. Промпты (prompts)

### analyze_logs

**Arguments:**
```json
[
  { "name": "path",  "description": "Путь к лог-файлу", "required": true },
  { "name": "lines", "description": "Количество последних строк", "required": false }
]
```

**Механика:** При вызове `prompts/get` сервер **сам** вызывает компрессию (`compress_tail(path, lines=500)`) и возвращает готовый контекст:

```json
{
  "messages": [
    {
      "role": "user",
      "content": {
        "type": "text",
        "text": "Ты — эксперт SRE. Перед тобой сжатый лог в формате logzip/v1.\nСначала изучи секцию LEGEND — она содержит словарь замен.\nЗатем читай BODY, ища аномалии, ошибки и паттерны.\n\n<compressed_log>\n{rendered_output}\n</compressed_log>"
      }
    }
  ]
}
```

Промпт не триггерит Claude вызвать инструмент. Сервер делает всю работу и отдаёт готовый «гамбургер» из инструкции и данных.

---

## 5. Sandboxing (sandbox.rs)

```rust
pub struct Sandbox {
    allowed: Vec<PathBuf>,  // все пути — уже canonicalized
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
            // canonicalize базовых путей обязателен:
            // на Windows canonicalize возвращает UNC-пути (\\?\C:\...)
            // если allowed содержит C:\logs, а path даст \\?\C:\logs\...,
            // то starts_with вернёт false — ложный sandbox miss.
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
            Err(format!(
                "Path '{}' is outside allowed directories",
                path
            ))
        }
    }
}
```

Symlink-атаки отсекаются: `canonicalize` разворачивает все симлинки до проверки prefix.

---

## 6. CLI (main.rs)

```
logzip compress  -i <path> [-o <path>] [--quality fast|balanced|max]
                 [--bpe-passes N] [--profile P] [--preamble] [--stats]
logzip decompress -i <path> [-o <path>]
logzip mcp       [--allow-dir <dir>]...
```

`logzip mcp` без `--allow-dir` → sandbox строится из CWD с предупреждением в stderr.

---

## 7. Дистрибуция

### Установка

```bash
cargo install logzip
```

> **Конфликт имён:** `pyproject.toml` регистрирует Python-скрипт `logzip` через `[project.scripts]`. При одновременной установке Python-пакета и Rust-бинарника возникает конфликт в PATH. Решение: переименовать Python-скрипт в `pyproject.toml` на `logzip-py`, оставив Rust-бинарник как основной `logzip`.

> **crates.io:** Перед публикацией проверить, не занято ли имя `logzip` на crates.io. Если занято — рассмотреть `logzip-mcp`.

### Claude Desktop Config (блок для README)

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

Путь к `claude_desktop_config.json`:
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

### Claude Code (MCP через CLI)

```bash
claude mcp add logzip -- logzip mcp --allow-dir /var/log
```

---

## 8. Зависимости

### logzip-core/Cargo.toml

```toml
[dependencies]
regex = "1"
rayon = "1.10"
aho-corasick = "1"
```

### logzip-py/Cargo.toml

```toml
[dependencies]
logzip-core = { path = "../logzip-core" }
pyo3 = { version = "0.22", features = ["extension-module", "abi3-py39"] }
```

### logzip-mcp/Cargo.toml

```toml
[[bin]]
name = "logzip"
path = "src/main.rs"

[dependencies]
logzip-core = { path = "../logzip-core" }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
```

Без tokio. Без async. Минимальный граф зависимостей.

---

## 9. Тестирование

- Существующие pytest-тесты (`tests/test_logzip.py`) запускаются через Python-обёртку — без изменений.
- Новые тесты для MCP: unit-тесты Rust в `logzip-mcp/src/` для `Sandbox::validate` (path traversal, symlinks, Windows UNC).
- Интеграционный smoke-test: скрипт запускает `logzip mcp`, посылает `initialize` + `tools/list` через stdin, проверяет JSON-ответы.

---

## Ограничения

- `compress_tail` не поддерживает бинарные файлы (только UTF-8 логи).
- MCP-сервер однопоточный — один клиент за раз (достаточно для Claude Desktop).
- `estimated_tokens` в `get_stats` — приближение (chars/4), не точный счёт.
