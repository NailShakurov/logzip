# logzip (Rust)

Compress logs **before** sending to LLM. Powered by Rust & PyO3.

```
raw log → [logzip compress] → compressed text → LLM (Claude Code / Cursor / API)
```

Typical savings: **40–60%** on structured logs (systemd, uvicorn, docker).  
Anomalies and unique lines stay uncompressed — visible at a glance in the BODY.

---

## Performance (8MB Log)

| Quality  | Time (s) | Savings (%) | Entries | Description                |
|----------|----------|-------------|---------|----------------------------|
| **fast** | ~0.5s    | 35-40%      | 32      | Default, near instant      |
| **balanced**| ~0.4s | 50-55%      | 128     | Best for daily use         |
| **max**  | ~0.5s    | 55-60%      | 512     | Max compression            |

*Benchmarked on a real 8MB RAG system log. Sub-second performance for multi-megabyte files.*

---

## Install

```bash
# Requires Rust toolchain for building from source
pip install .
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
