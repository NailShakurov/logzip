"""logzip — fast log compression for LLM analysis.

Rust-powered core, Python API.

    from logzip import compress, decompress

    result = compress(open("app.log").read())
    print(result.render(with_preamble=True))   # → send to Claude
    print(result.stats_str())
"""

from __future__ import annotations

from logzip._logzip import (  # type: ignore[import]
    CompressResult,
    compress_log as compress,
    decompress_log as decompress,
)

__version__ = "0.1.0"
__all__ = ["compress", "decompress", "CompressResult"]
