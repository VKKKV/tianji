from __future__ import annotations

import argparse
import json
from pathlib import Path

from .fetch import TianJiInputError
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
        "--source-config",
        default=None,
        help="Optional JSON file containing named source URLs",
    )
    run_parser.add_argument(
        "--source-name",
        action="append",
        default=[],
        help="Optional source name from --source-config; repeat to select multiple",
    )
    run_parser.add_argument(
        "--output",
        default="runs/latest-run.json",
        help="Path for the generated JSON artifact (default: runs/latest-run.json)",
    )
    run_parser.add_argument(
        "--sqlite-path",
        default=None,
        help="Optional SQLite database path for persisting run data",
    )
    return parser


def load_source_registry(path: str) -> dict[str, str]:
    try:
        payload = json.loads(Path(path).read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise ValueError(f"Source config file not found: {path}") from error
    except json.JSONDecodeError as error:
        raise ValueError(f"Source config is not valid JSON: {path}") from error

    sources = payload.get("sources")
    if not isinstance(sources, list) or not sources:
        raise ValueError("Source config must contain a non-empty 'sources' list.")

    registry: dict[str, str] = {}
    for entry in sources:
        if not isinstance(entry, dict):
            raise ValueError("Each source entry must be an object.")
        name = entry.get("name")
        url = entry.get("url")
        if not isinstance(name, str) or not name.strip():
            raise ValueError(
                "Each source entry must include a non-empty string 'name'."
            )
        if not isinstance(url, str) or not url.strip():
            raise ValueError("Each source entry must include a non-empty string 'url'.")
        if name in registry:
            raise ValueError(f"Duplicate source name in config: {name}")
        registry[name] = url

    return registry


def resolve_source_urls(
    *,
    registry: dict[str, str],
    selected_names: list[str],
) -> list[str]:
    if not selected_names:
        return list(registry.values())

    missing = [name for name in selected_names if name not in registry]
    if missing:
        names = ", ".join(sorted(missing))
        raise ValueError(f"Unknown source name(s) in config selection: {names}")

    return [registry[name] for name in selected_names]


def dedupe_urls(urls: list[str]) -> list[str]:
    seen: set[str] = set()
    deduped: list[str] = []
    for url in urls:
        if url in seen:
            continue
        seen.add(url)
        deduped.append(url)
    return deduped


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command == "run":
        if not args.fixture and not args.fetch:
            parser.error(
                "Provide at least one --fixture or enable --fetch with --source-url and/or --source-config."
            )
        resolved_source_urls = list(args.source_url)

        if args.source_name and not args.source_config:
            parser.error("--source-name requires --source-config.")

        if args.source_config:
            try:
                registry = load_source_registry(args.source_config)
                resolved_source_urls.extend(
                    resolve_source_urls(
                        registry=registry,
                        selected_names=args.source_name,
                    )
                )
            except ValueError as error:
                parser.error(str(error))

        resolved_source_urls = dedupe_urls(resolved_source_urls)

        if args.fetch and not resolved_source_urls:
            parser.error(
                "--fetch requires at least one source from --source-url or --source-config."
            )

        try:
            artifact = run_pipeline(
                fixture_paths=args.fixture,
                fetch=args.fetch,
                source_urls=resolved_source_urls,
                output_path=args.output,
                sqlite_path=args.sqlite_path,
            )
        except TianJiInputError as error:
            parser.error(str(error))
        print(json.dumps(artifact.to_dict(), ensure_ascii=False, indent=2))
        print(f"\nArtifact written to: {Path(args.output)}")
        return 0

    parser.error(f"Unsupported command: {args.command}")
    return 2
