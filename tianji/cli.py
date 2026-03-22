from __future__ import annotations

import argparse
import json
from pathlib import Path

from .fetch import TianJiInputError
from .pipeline import run_pipeline
from .storage import (
    compare_runs,
    get_latest_run_id,
    get_latest_run_pair,
    get_run_summary,
    list_runs,
)


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

    history_parser = subparsers.add_parser(
        "history", help="List persisted TianJi runs from SQLite"
    )
    history_parser.add_argument(
        "--sqlite-path",
        required=True,
        help="SQLite database path containing persisted TianJi runs",
    )
    history_parser.add_argument(
        "--limit",
        type=int,
        default=20,
        help="Maximum number of runs to list (default: 20)",
    )
    history_parser.add_argument(
        "--mode",
        default=None,
        help="Optional run mode filter (for example: fixture, fetch, fetch+fixture)",
    )
    history_parser.add_argument(
        "--dominant-field",
        default=None,
        help="Optional dominant field filter for persisted runs",
    )
    history_parser.add_argument(
        "--risk-level",
        default=None,
        help="Optional risk level filter for persisted runs",
    )
    history_parser.add_argument(
        "--since",
        default=None,
        help="Optional inclusive lower bound for generated_at (ISO timestamp)",
    )
    history_parser.add_argument(
        "--until",
        default=None,
        help="Optional inclusive upper bound for generated_at (ISO timestamp)",
    )

    history_show_parser = subparsers.add_parser(
        "history-show", help="Show one persisted TianJi run summary from SQLite"
    )
    history_show_parser.add_argument(
        "--sqlite-path",
        required=True,
        help="SQLite database path containing persisted TianJi runs",
    )
    history_show_parser.add_argument(
        "--run-id",
        type=int,
        help="Run identifier to inspect",
    )
    history_show_parser.add_argument(
        "--latest",
        action="store_true",
        help="Show the latest persisted run instead of specifying --run-id",
    )

    history_compare_parser = subparsers.add_parser(
        "history-compare", help="Compare two persisted TianJi runs from SQLite"
    )
    history_compare_parser.add_argument(
        "--sqlite-path",
        required=True,
        help="SQLite database path containing persisted TianJi runs",
    )
    history_compare_parser.add_argument(
        "--left-run-id",
        type=int,
        help="Left-side run identifier for comparison",
    )
    history_compare_parser.add_argument(
        "--right-run-id",
        type=int,
        help="Right-side run identifier for comparison",
    )
    history_compare_parser.add_argument(
        "--latest-pair",
        action="store_true",
        help="Compare the two latest persisted runs instead of specifying run ids",
    )
    history_compare_parser.add_argument(
        "--run-id",
        type=int,
        help="Compare one explicit run against the latest persisted run",
    )
    history_compare_parser.add_argument(
        "--against-latest",
        action="store_true",
        help="Use the latest persisted run as the right-hand side for comparison",
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

    if args.command == "history":
        payload = list_runs(
            sqlite_path=args.sqlite_path,
            limit=args.limit,
            mode=args.mode,
            dominant_field=args.dominant_field,
            risk_level=args.risk_level,
            since=args.since,
            until=args.until,
        )
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    if args.command == "history-show":
        if args.latest and args.run_id is not None:
            parser.error("Use either --run-id or --latest for history-show, not both.")
        if not args.latest and args.run_id is None:
            parser.error("history-show requires either --run-id or --latest.")
        run_id = args.run_id
        if args.latest:
            run_id = get_latest_run_id(sqlite_path=args.sqlite_path)
            if run_id is None:
                parser.error("No persisted runs are available.")
        payload = get_run_summary(sqlite_path=args.sqlite_path, run_id=run_id)
        if payload is None:
            parser.error(f"Run not found: {run_id}")
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    if args.command == "history-compare":
        if args.latest_pair and (
            args.left_run_id is not None
            or args.right_run_id is not None
            or args.run_id is not None
            or args.against_latest
        ):
            parser.error(
                "Use either --latest-pair, --run-id with --against-latest, or explicit --left-run-id/--right-run-id, not a mix."
            )
        if args.latest_pair:
            pair = get_latest_run_pair(sqlite_path=args.sqlite_path)
            if pair is None:
                parser.error(
                    "At least two persisted runs are required for --latest-pair."
                )
            left_run_id, right_run_id = pair
        elif args.against_latest:
            if args.run_id is None:
                parser.error("history-compare with --against-latest requires --run-id.")
            latest_run_id = get_latest_run_id(sqlite_path=args.sqlite_path)
            if latest_run_id is None:
                parser.error("No persisted runs are available.")
            left_run_id = args.run_id
            right_run_id = latest_run_id
        else:
            if args.run_id is not None:
                parser.error(
                    "Use --run-id only with --against-latest for history-compare."
                )
            if args.left_run_id is None or args.right_run_id is None:
                parser.error(
                    "history-compare requires --latest-pair, --run-id with --against-latest, or both --left-run-id and --right-run-id."
                )
            left_run_id = args.left_run_id
            right_run_id = args.right_run_id
        payload = compare_runs(
            sqlite_path=args.sqlite_path,
            left_run_id=left_run_id,
            right_run_id=right_run_id,
        )
        if payload is None:
            parser.error(
                f"Run not found for comparison: {left_run_id} vs {right_run_id}"
            )
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    parser.error(f"Unsupported command: {args.command}")
    return 2
