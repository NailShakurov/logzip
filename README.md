# logzip (Rust)

Compress logs **before** sending to LLM. Powered by Rust & PyO3.

```
raw log → [logzip compress] → compressed text → LLM (Claude Code / Cursor / API)
```

### Before / After

**Raw Log (Uvicorn):**
```text
INFO: 127.0.0.1:45678 - "GET /api/v1/status HTTP/1.1" 200 OK
INFO: 127.0.0.1:45679 - "GET /api/v1/status HTTP/1.1" 200 OK
... (100 similar lines) ...
```

**logzip output:**
```text
--- PREFIX ---
INFO: 127.0.0.1:
--- LEGEND ---
#0# = - "GET /api/v1/status HTTP/1.1" 200 OK
--- BODY ---
45678 #0#
45679 #0#
...
```

Typical savings: **40–60%** on structured logs (systemd, uvicorn, docker).  
Anomalies and unique lines stay uncompressed — visible at a glance in the BODY.

### 🚀 Зачем это нужно (RAG & LLM)

При работе с логами в LLM (Claude, GPT, RAG-системы) вы сталкиваетесь с двумя проблемами:
1. **Context Limit**: Логи огромны. 10МБ лога — это ~2.5 млн токенов.
2. **Noise**: 90% лога — это повторяющиеся `INFO` и однотипные запросы, которые мешают модели найти реальную ошибку.

`logzip` идеально ложится в **RAG-пайплайны**: вы сжимаете контекст перед отправкой в модель, экономя деньги на токенах и повышая точность ответов за счет выделения аномалий.

---

## Performance (8MB Log)

| Quality  | Time (s) | Savings (%) | Tokens (est.) | Entries | Description                |
|----------|----------|-------------|---------------|---------|----------------------------|
| **fast** | ~0.5s    | 35-40%      | ~1.2M         | 32      | Default, near instant      |
| **balanced**| ~0.4s | 50-55%      | ~0.9M         | 128     | Best for daily use         |
| **max**  | ~0.5s    | 55-60%      | ~0.8M         | 512     | Max compression            |

*Benchmarked on a real 8MB log (~2.0M tokens). Token estimation: 1 token ≈ 4 characters. Sub-second performance.*

---

## Install

```bash
pip install logzip
```

## CLI

```bash
# stdin → stdout (основной режим)
cat app.log | logzip compress | pbcopy      # → буфер → вставить в Claude

# с выбором качества (fast|balanced|max)
logzip compress --quality balanced < app.log

# с preamble (инструкции для LLM в начале вывода)
logzip compress --preamble < app.log > compressed.txt

# сохранить + показать статистику
logzip compress --stats -i app.log -o app.logzip

# явно указать профиль (иначе auto-detect)
logzip compress --profile journalctl < /tmp/syslog.txt
```

## Python API

```python
from logzip import compress, decompress

# сжатие
result = compress(raw_log_text, quality="balanced")
print(result.render(with_preamble=True))   # → в LLM
print(result.stats_str())                  # → в stderr
```

## Глазами LLM

В отличие от `gzip/zstd`, которые выдают бинарный шум, `logzip` выдает **структурированный текст**. Модель понимает легенду и может «распаковать» лог в уме или анализировать его прямо в сжатом виде.

**Вход для LLM:**
> Это сжатый лог. Правила: `#0#` заменяется на `GET /api/v1/status`.
>
> --- BODY ---
> 12:00:01 #0# 200 OK
> 12:00:02 #0# 500 ERR <-- Опа, аномалия!

Модель мгновенно видит 500-ю ошибку, не продираясь через тысячи строк одинаковых успешных запросов.

## Архитектура (Rust)

1. **Normalizer**: Схлопывание ANSI, таймстампов, IP и общего префикса.
2. **Frequency Analysis**: Параллельный подсчет n-грамм (rayon).
3. **Greedy Legend**: Оптимизированный выбор легенды через позиционный индекс (O(N)).
4. **Direct Replacement**: Прямая замена без повторного сканирования.
5. **Templates**: Извлечение повторяющихся структур строк.

## Тесты

```bash
python -m pytest tests/ -v
```

---

## Roadmap / v2

- [ ] MCP-сервер для Claude Code
- [ ] suffix automaton для поиска произвольных повторов
- [ ] streaming mode для гигантских файлов


[![PyPI version](https://img.shields.io/pypi/v/logzip.svg)](https://pypi.org/project/logzip/)
[![PyPI downloads](https://img.shields.io/pypi/dm/logzip.svg)](https://pypi.org/project/logzip/)
[![Python 3.9+](https://img.shields.io/pypi/pyversions/logzip.svg)](https://pypi.org/project/logzip/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/powered%20by-Rust-orange.svg)](https://www.rust-lang.org/)
