"""Command-line interface for logzip."""

from __future__ import annotations

import argparse
import sys

from logzip import compress, decompress


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="logzip",
        description="Compress logs for LLM analysis",
    )
    sub = parser.add_subparsers(dest="cmd", required=True)

    pc = sub.add_parser("compress", help="Compress a log file")
    pc.add_argument("-i", "--input", default="-", help="Input file (default: stdin)")
    pc.add_argument("-o", "--output", default="-", help="Output file (default: stdout)")
    pc.add_argument("--preamble", action="store_true", help="Prepend decode instructions")
    pc.add_argument("--stats", action="store_true", help="Print stats to stderr")
    pc.add_argument("--max-ngram", type=int, default=2, help="Max n-gram size (default: 2)")
    pc.add_argument("--no-normalize", action="store_true", help="Skip normalization")
    pc.add_argument("--no-templates", action="store_true", help="Skip template extraction")
    pc.add_argument("--quality", choices=["fast", "balanced", "max"], default="fast", help="Compression quality preset (default: fast)")
    pc.add_argument("--profile", default=None, help="Force profile: journalctl|docker|uvicorn|nodejs|plain")

    pd = sub.add_parser("decompress", help="Decompress a logzip file")
    pd.add_argument("-i", "--input", default="-", help="Input file (default: stdin)")
    pd.add_argument("-o", "--output", default="-", help="Output file (default: stdout)")

    args = parser.parse_args()

    if args.cmd == "compress":
        quality_map = {
            "fast":     (32,  1),
            "balanced": (128, 1),
            "max":      (512, 2),
        }

        raw = sys.stdin.read() if args.input == "-" else open(args.input).read()
        max_legend_entries, bpe_passes = quality_map[args.quality]
        if args.quality == "balanced" and len(raw) > 5_000_000:
            bpe_passes = 2

        result = compress(
            raw,
            max_ngram=args.max_ngram,
            max_legend_entries=max_legend_entries,
            do_normalize=not args.no_normalize,
            profile=args.profile,
            do_templates=not args.no_templates,
            bpe_passes=bpe_passes,
        )
        output = result.render(with_preamble=args.preamble)

        if args.output == "-":
            print(output)
        else:
            open(args.output, "w").write(output)

        if args.stats:
            print(result.stats_str(), file=sys.stderr)

    elif args.cmd == "decompress":
        raw = sys.stdin.read() if args.input == "-" else open(args.input).read()
        output = decompress(raw)

        if args.output == "-":
            print(output)
        else:
            open(args.output, "w").write(output)


if __name__ == "__main__":
    main()
