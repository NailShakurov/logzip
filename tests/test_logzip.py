"""Tests for logzip Rust extension."""

import pytest


def test_import():
    from logzip import compress, decompress, CompressResult
    assert compress is not None
    assert decompress is not None


def test_compress_basic():
    from logzip import compress

    log = "\n".join([
        "2026-04-21T14:32:17.123456789Z INFO server started on port 8080",
        "2026-04-21T14:32:18.234567890Z INFO request received GET /api/users",
        "2026-04-21T14:32:18.345678901Z INFO request received GET /api/users",
        "2026-04-21T14:32:18.456789012Z INFO request received POST /api/users",
        "2026-04-21T14:32:19.567890123Z ERROR connection refused port 5432",
        "2026-04-21T14:32:19.678901234Z ERROR connection refused port 5432",
        "2026-04-21T14:32:19.789012345Z ERROR connection refused port 5432",
    ] * 10)

    result = compress(log)
    assert result.body
    assert len(result.legend) > 0
    assert result.stats_str().startswith("[logzip]")


def test_compress_reduces_size():
    from logzip import compress

    log = "\n".join([
        f"2026-04-21T14:32:{i:02d}.123456789Z INFO processing request for user_id=1234 session=abcdef"
        for i in range(60)
    ])

    result = compress(log)
    rendered = result.render(with_preamble=False)
    assert len(rendered) < len(log), "Compressed should be smaller than original"


def test_render_with_preamble():
    from logzip import compress

    log = "INFO server started\n" * 20
    result = compress(log)
    rendered = result.render(with_preamble=True)
    assert "logzip/v1" in rendered
    assert "--- BODY ---" in rendered


def test_decompress_roundtrip():
    from logzip import compress, decompress

    log = "INFO server started on port 8080\n" * 30
    result = compress(log, do_normalize=False, do_templates=False)
    rendered = result.render()
    restored = decompress(rendered)
    assert "INFO server started on port 8080" in restored


def test_profile_journalctl():
    from logzip import compress

    log = "\n".join([
        f"Apr 21 14:32:{i:02d} myhost myservice[12345]: message number {i} for testing"
        for i in range(30)
    ])
    result = compress(log)
    assert result.detected_profile == "journalctl"


def test_compress_stats():
    from logzip import compress

    # Use varied log so normalization doesn't collapse everything into prefix
    log = "\n".join([
        f"INFO  database connection established host=db.internal port=5432 latency={i}ms"
        for i in range(50)
    ] + [
        f"ERROR database connection failed host=db.internal port=5432 attempt={i}"
        for i in range(50)
    ])
    result = compress(log)
    stats = result.stats()
    assert "original_chars" in stats
    assert "ratio_pct" in stats
    # With varied log, should have legend entries
    assert stats["original_chars"] > stats["compressed_chars"]


def test_max_ngram_param():
    from logzip import compress

    log = "connecting to database server at host 192.168.1.1\n" * 30
    result1 = compress(log, max_ngram=1)
    result2 = compress(log, max_ngram=5)
    assert len(result2.legend) >= len(result1.legend)
