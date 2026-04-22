#!/usr/bin/env bash
# ~/.claude/hooks/logzip_preprocess.sh
#
# Hook для Claude Code: если в рабочей директории появились .log файлы,
# сжимаем их перед тем как Claude Code их читает.
#
# Установка:
#   cp logzip_preprocess.sh ~/.claude/hooks/
#   chmod +x ~/.claude/hooks/logzip_preprocess.sh
#   # добавить в ~/.claude/settings.json секцию hooks (см. settings_fragment.json)
#
# Использование в Claude Code:
#   /compress logs/app.log        → сжатый вывод в stdout
#   /compress-all logs/            → все .log файлы в директории

set -euo pipefail

# Читаем JSON из stdin (Claude Code передаёт контекст вызова инструмента)
INPUT=$(cat)
TOOL_NAME=$(echo "$INPUT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tool_name',''))" 2>/dev/null || echo "")
COMMAND=$(echo "$INPUT"  | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tool_input',{}).get('command',''))" 2>/dev/null || echo "")

# Ищем .log файлы в команде
if echo "$COMMAND" | grep -qE '\.log\b'; then
    LOG_FILE=$(echo "$COMMAND" | grep -oE '[^ ]+\.log\b' | head -1)
    if [[ -f "$LOG_FILE" ]]; then
        SIZE=$(wc -c < "$LOG_FILE")
        # Сжимаем только если файл > 10KB
        if [[ $SIZE -gt 10240 ]]; then
            COMPRESSED=$(logzip compress --stats < "$LOG_FILE" 2>/tmp/logzip_stats.txt)
            STATS=$(cat /tmp/logzip_stats.txt)
            # Выводим в stderr — Claude Code видит это как контекст
            echo "[logzip hook] $STATS" >&2
        fi
    fi
fi

# Всегда пропускаем оригинальный вызов
echo '{"action": "continue"}'
