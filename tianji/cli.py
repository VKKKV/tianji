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
    get_next_run_id,
    get_previous_run_id,
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
    history_parser.add_argument(
        "--min-top-impact-score",
        type=float,
        default=None,
        help="Optional minimum impact_score for the persisted run's top scored event",
    )
    history_parser.add_argument(
        "--max-top-impact-score",
        type=float,
        default=None,
        help="Optional maximum impact_score for the persisted run's top scored event",
    )
    history_parser.add_argument(
        "--min-top-field-attraction",
        type=float,
        default=None,
        help="Optional minimum field_attraction for the persisted run's top scored event",
    )
    history_parser.add_argument(
        "--max-top-field-attraction",
        type=float,
        default=None,
        help="Optional maximum field_attraction for the persisted run's top scored event",
    )
    history_parser.add_argument(
        "--min-top-divergence-score",
        type=float,
        default=None,
        help="Optional minimum divergence_score for the persisted run's top scored event",
    )
    history_parser.add_argument(
        "--max-top-divergence-score",
        type=float,
        default=None,
        help="Optional maximum divergence_score for the persisted run's top scored event",
    )
    history_parser.add_argument(
        "--top-group-dominant-field",
        default=None,
        help="Optional dominant field filter for the persisted run's top event group",
    )
    history_parser.add_argument(
        "--min-event-group-count",
        type=int,
        default=None,
        help="Optional minimum persisted event-group count for the run",
    )
    history_parser.add_argument(
        "--max-event-group-count",
        type=int,
        default=None,
        help="Optional maximum persisted event-group count for the run",
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
    history_show_parser.add_argument(
        "--previous",
        action="store_true",
        help="Show the persisted run immediately before --run-id",
    )
    history_show_parser.add_argument(
        "--next",
        action="store_true",
        help="Show the persisted run immediately after --run-id",
    )
    history_show_parser.add_argument(
        "--dominant-field",
        default=None,
        help="Optional dominant field filter for scored events inside the selected run",
    )
    history_show_parser.add_argument(
        "--min-impact-score",
        type=float,
        default=None,
        help="Optional minimum impact_score for scored events inside the selected run",
    )
    history_show_parser.add_argument(
        "--max-impact-score",
        type=float,
        default=None,
        help="Optional maximum impact_score for scored events inside the selected run",
    )
    history_show_parser.add_argument(
        "--min-field-attraction",
        type=float,
        default=None,
        help="Optional minimum field_attraction for scored events inside the selected run",
    )
    history_show_parser.add_argument(
        "--max-field-attraction",
        type=float,
        default=None,
        help="Optional maximum field_attraction for scored events inside the selected run",
    )
    history_show_parser.add_argument(
        "--min-divergence-score",
        type=float,
        default=None,
        help="Optional minimum divergence_score for scored events inside the selected run",
    )
    history_show_parser.add_argument(
        "--max-divergence-score",
        type=float,
        default=None,
        help="Optional maximum divergence_score for scored events inside the selected run",
    )
    history_show_parser.add_argument(
        "--limit-scored-events",
        type=int,
        default=None,
        help="Optional maximum number of scored events to return for the selected run",
    )
    history_show_parser.add_argument(
        "--only-matching-interventions",
        action="store_true",
        help="Keep only intervention candidates whose event_id remains in the final visible scored-event set after filters and limits",
    )
    history_show_parser.add_argument(
        "--group-dominant-field",
        default=None,
        help="Optional dominant field filter for persisted event groups inside the selected run",
    )
    history_show_parser.add_argument(
        "--limit-event-groups",
        type=int,
        default=None,
        help="Optional maximum number of persisted event groups to return for the selected run",
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
    history_compare_parser.add_argument(
        "--against-previous",
        action="store_true",
        help="Use the immediately previous persisted run as the left-hand side for comparison",
    )
    history_compare_parser.add_argument(
        "--dominant-field",
        default=None,
        help="Optional dominant field filter for scored events inside both compared runs",
    )
    history_compare_parser.add_argument(
        "--min-impact-score",
        type=float,
        default=None,
        help="Optional minimum impact_score for scored events inside both compared runs",
    )
    history_compare_parser.add_argument(
        "--max-impact-score",
        type=float,
        default=None,
        help="Optional maximum impact_score for scored events inside both compared runs",
    )
    history_compare_parser.add_argument(
        "--min-field-attraction",
        type=float,
        default=None,
        help="Optional minimum field_attraction for scored events inside both compared runs",
    )
    history_compare_parser.add_argument(
        "--max-field-attraction",
        type=float,
        default=None,
        help="Optional maximum field_attraction for scored events inside both compared runs",
    )
    history_compare_parser.add_argument(
        "--min-divergence-score",
        type=float,
        default=None,
        help="Optional minimum divergence_score for scored events inside both compared runs",
    )
    history_compare_parser.add_argument(
        "--max-divergence-score",
        type=float,
        default=None,
        help="Optional maximum divergence_score for scored events inside both compared runs",
    )
    history_compare_parser.add_argument(
        "--limit-scored-events",
        type=int,
        default=None,
        help="Optional maximum number of scored events to return for each compared run",
    )
    history_compare_parser.add_argument(
        "--only-matching-interventions",
        action="store_true",
        help="Keep only intervention candidates whose event_id remains in the final visible scored-event set for each compared run",
    )
    history_compare_parser.add_argument(
        "--group-dominant-field",
        default=None,
        help="Optional dominant field filter for persisted event groups inside both compared runs",
    )
    history_compare_parser.add_argument(
        "--limit-event-groups",
        type=int,
        default=None,
        help="Optional maximum number of persisted event groups to return for each compared run",
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


def validate_score_range(
    parser: argparse.ArgumentParser,
    *,
    min_value: float | None,
    max_value: float | None,
    min_flag: str,
    max_flag: str,
) -> None:
    if min_value is not None and max_value is not None and min_value > max_value:
        parser.error(f"{min_flag} cannot be greater than {max_flag}.")


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
        if args.limit < 0:
            parser.error("--limit must be zero or greater.")
        if args.min_event_group_count is not None and args.min_event_group_count < 0:
            parser.error("--min-event-group-count must be zero or greater.")
        if args.max_event_group_count is not None and args.max_event_group_count < 0:
            parser.error("--max-event-group-count must be zero or greater.")
        validate_score_range(
            parser,
            min_value=args.min_top_impact_score,
            max_value=args.max_top_impact_score,
            min_flag="--min-top-impact-score",
            max_flag="--max-top-impact-score",
        )
        validate_score_range(
            parser,
            min_value=args.min_top_field_attraction,
            max_value=args.max_top_field_attraction,
            min_flag="--min-top-field-attraction",
            max_flag="--max-top-field-attraction",
        )
        validate_score_range(
            parser,
            min_value=args.min_top_divergence_score,
            max_value=args.max_top_divergence_score,
            min_flag="--min-top-divergence-score",
            max_flag="--max-top-divergence-score",
        )
        if (
            args.min_event_group_count is not None
            and args.max_event_group_count is not None
            and args.min_event_group_count > args.max_event_group_count
        ):
            parser.error(
                "--min-event-group-count cannot be greater than --max-event-group-count."
            )
        payload = list_runs(
            sqlite_path=args.sqlite_path,
            limit=args.limit,
            mode=args.mode,
            dominant_field=args.dominant_field,
            risk_level=args.risk_level,
            since=args.since,
            until=args.until,
            min_top_impact_score=args.min_top_impact_score,
            max_top_impact_score=args.max_top_impact_score,
            min_top_field_attraction=args.min_top_field_attraction,
            max_top_field_attraction=args.max_top_field_attraction,
            min_top_divergence_score=args.min_top_divergence_score,
            max_top_divergence_score=args.max_top_divergence_score,
            top_group_dominant_field=args.top_group_dominant_field,
            min_event_group_count=args.min_event_group_count,
            max_event_group_count=args.max_event_group_count,
        )
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    if args.command == "history-show":
        if sum([bool(args.latest), bool(args.previous), bool(args.next)]) > 1:
            parser.error(
                "Use only one history-show navigation mode: --latest, --previous, or --next."
            )
        if args.limit_scored_events is not None and args.limit_scored_events < 0:
            parser.error("--limit-scored-events must be zero or greater.")
        if args.limit_event_groups is not None and args.limit_event_groups < 0:
            parser.error("--limit-event-groups must be zero or greater.")
        validate_score_range(
            parser,
            min_value=args.min_impact_score,
            max_value=args.max_impact_score,
            min_flag="--min-impact-score",
            max_flag="--max-impact-score",
        )
        validate_score_range(
            parser,
            min_value=args.min_field_attraction,
            max_value=args.max_field_attraction,
            min_flag="--min-field-attraction",
            max_flag="--max-field-attraction",
        )
        validate_score_range(
            parser,
            min_value=args.min_divergence_score,
            max_value=args.max_divergence_score,
            min_flag="--min-divergence-score",
            max_flag="--max-divergence-score",
        )
        if args.latest and args.run_id is not None:
            parser.error("Use either --run-id or --latest for history-show, not both.")
        if (args.previous or args.next) and args.run_id is None:
            parser.error("history-show with --previous/--next requires --run-id.")
        if (
            not args.latest
            and not args.previous
            and not args.next
            and args.run_id is None
        ):
            parser.error(
                "history-show requires --run-id, --latest, --previous, or --next."
            )
        run_id = args.run_id
        if args.latest:
            run_id = get_latest_run_id(sqlite_path=args.sqlite_path)
            if run_id is None:
                parser.error("No persisted runs are available.")
        elif args.previous:
            previous_run_id = get_previous_run_id(
                sqlite_path=args.sqlite_path, run_id=args.run_id
            )
            if previous_run_id is None:
                parser.error(
                    f"No previous persisted run exists before run {args.run_id}."
                )
            run_id = previous_run_id
        elif args.next:
            next_run_id = get_next_run_id(
                sqlite_path=args.sqlite_path, run_id=args.run_id
            )
            if next_run_id is None:
                parser.error(f"No next persisted run exists after run {args.run_id}.")
            run_id = next_run_id
        payload = get_run_summary(
            sqlite_path=args.sqlite_path,
            run_id=run_id,
            dominant_field=args.dominant_field,
            min_impact_score=args.min_impact_score,
            max_impact_score=args.max_impact_score,
            min_field_attraction=args.min_field_attraction,
            max_field_attraction=args.max_field_attraction,
            min_divergence_score=args.min_divergence_score,
            max_divergence_score=args.max_divergence_score,
            limit_scored_events=args.limit_scored_events,
            only_matching_interventions=args.only_matching_interventions,
            group_dominant_field=args.group_dominant_field,
            limit_event_groups=args.limit_event_groups,
        )
        if payload is None:
            parser.error(f"Run not found: {run_id}")
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    if args.command == "history-compare":
        if args.limit_scored_events is not None and args.limit_scored_events < 0:
            parser.error("--limit-scored-events must be zero or greater.")
        if args.limit_event_groups is not None and args.limit_event_groups < 0:
            parser.error("--limit-event-groups must be zero or greater.")
        validate_score_range(
            parser,
            min_value=args.min_impact_score,
            max_value=args.max_impact_score,
            min_flag="--min-impact-score",
            max_flag="--max-impact-score",
        )
        validate_score_range(
            parser,
            min_value=args.min_field_attraction,
            max_value=args.max_field_attraction,
            min_flag="--min-field-attraction",
            max_flag="--max-field-attraction",
        )
        validate_score_range(
            parser,
            min_value=args.min_divergence_score,
            max_value=args.max_divergence_score,
            min_flag="--min-divergence-score",
            max_flag="--max-divergence-score",
        )
        if args.latest_pair and (
            args.left_run_id is not None
            or args.right_run_id is not None
            or args.run_id is not None
            or args.against_latest
            or args.against_previous
        ):
            parser.error(
                "Use either --latest-pair, --run-id with --against-latest, --run-id with --against-previous, or explicit --left-run-id/--right-run-id, not a mix."
            )
        if args.latest_pair:
            pair = get_latest_run_pair(sqlite_path=args.sqlite_path)
            if pair is None:
                parser.error(
                    "At least two persisted runs are required for --latest-pair."
                )
            left_run_id, right_run_id = pair
        elif args.against_latest:
            if args.against_previous:
                parser.error(
                    "Use only one comparison preset: --against-latest or --against-previous."
                )
            if args.left_run_id is not None or args.right_run_id is not None:
                parser.error(
                    "Use either --latest-pair, --run-id with --against-latest, --run-id with --against-previous, or explicit --left-run-id/--right-run-id, not a mix."
                )
            if args.run_id is None:
                parser.error("history-compare with --against-latest requires --run-id.")
            latest_run_id = get_latest_run_id(sqlite_path=args.sqlite_path)
            if latest_run_id is None:
                parser.error("No persisted runs are available.")
            left_run_id = args.run_id
            right_run_id = latest_run_id
        elif args.against_previous:
            if args.left_run_id is not None or args.right_run_id is not None:
                parser.error(
                    "Use either --latest-pair, --run-id with --against-latest, --run-id with --against-previous, or explicit --left-run-id/--right-run-id, not a mix."
                )
            if args.run_id is None:
                parser.error(
                    "history-compare with --against-previous requires --run-id."
                )
            previous_run_id = get_previous_run_id(
                sqlite_path=args.sqlite_path, run_id=args.run_id
            )
            if previous_run_id is None:
                parser.error(
                    f"No previous persisted run exists before run {args.run_id}."
                )
            left_run_id = previous_run_id
            right_run_id = args.run_id
        else:
            if args.run_id is not None:
                parser.error(
                    "Use --run-id only with --against-latest or --against-previous for history-compare."
                )
            if args.left_run_id is None or args.right_run_id is None:
                parser.error(
                    "history-compare requires --latest-pair, --run-id with --against-latest, --run-id with --against-previous, or both --left-run-id and --right-run-id."
                )
            left_run_id = args.left_run_id
            right_run_id = args.right_run_id
        payload = compare_runs(
            sqlite_path=args.sqlite_path,
            left_run_id=left_run_id,
            right_run_id=right_run_id,
            dominant_field=args.dominant_field,
            min_impact_score=args.min_impact_score,
            max_impact_score=args.max_impact_score,
            min_field_attraction=args.min_field_attraction,
            max_field_attraction=args.max_field_attraction,
            min_divergence_score=args.min_divergence_score,
            max_divergence_score=args.max_divergence_score,
            limit_scored_events=args.limit_scored_events,
            only_matching_interventions=args.only_matching_interventions,
            group_dominant_field=args.group_dominant_field,
            limit_event_groups=args.limit_event_groups,
        )
        if payload is None:
            parser.error(
                f"Run not found for comparison: {left_run_id} vs {right_run_id}"
            )
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    parser.error(f"Unsupported command: {args.command}")
    return 2
