from __future__ import annotations

import argparse
import json
from pathlib import Path

from .pipeline import run_pipeline


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="TianJi one-shot MVP")
    subparsers = parser.add_subparsers(dest="command", required=True)

    run_parser = subparsers.add_parser(
        "run", help="Run one-shot fetch -> infer -> backtrack pipeline"
    )
    run_parser.add_argument(
        "--fixture",
        action="append",
        default=[],
        help="Path to a local RSS/Atom fixture file",
    )
    run_parser.add_argument(
        "--fetch", action="store_true", help="Fetch one or more live feeds once"
    )
    run_parser.add_argument(
        "--source-url", action="append", default=[], help="Feed URL used with --fetch"
    )
    run_parser.add_argument(
        "--output",
        default="runs/latest-run.json",
        help="Path for the generated JSON artifact (default: runs/latest-run.json)",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command == "run":
        if not args.fixture and not args.fetch:
            parser.error(
                "Provide at least one --fixture or enable --fetch with --source-url."
            )
        if args.fetch and not args.source_url:
            parser.error("--fetch requires at least one --source-url.")

        artifact = run_pipeline(
            fixture_paths=args.fixture,
            fetch=args.fetch,
            source_urls=args.source_url,
            output_path=args.output,
        )
        print(json.dumps(artifact.to_dict(), ensure_ascii=False, indent=2))
        print(f"\nArtifact written to: {Path(args.output)}")
        return 0

    parser.error(f"Unsupported command: {args.command}")
    return 2
