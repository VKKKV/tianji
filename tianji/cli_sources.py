from __future__ import annotations

import json
from pathlib import Path

import click


FETCH_POLICY_CHOICES = ("always", "if-missing", "if-changed")

SourceConfigEntry = dict[str, str]
ResolvedSourceEntry = dict[str, str]


def validate_fetch_policy(*, value: object, context: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{context} must be a non-empty string.")
    normalized = value.strip().lower()
    if normalized not in FETCH_POLICY_CHOICES:
        allowed = ", ".join(FETCH_POLICY_CHOICES)
        raise ValueError(f"{context} must be one of: {allowed}.")
    return normalized


def load_source_registry(path: str) -> tuple[dict[str, SourceConfigEntry], str]:
    try:
        payload = json.loads(Path(path).read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise ValueError(f"Source config file not found: {path}") from error
    except json.JSONDecodeError as error:
        raise ValueError(f"Source config is not valid JSON: {path}") from error

    sources = payload.get("sources")
    if not isinstance(sources, list) or not sources:
        raise ValueError("Source config must contain a non-empty 'sources' list.")

    default_fetch_policy = payload.get("default_fetch_policy", "always")
    validated_default_fetch_policy = validate_fetch_policy(
        value=default_fetch_policy,
        context="Source config 'default_fetch_policy'",
    )

    registry: dict[str, SourceConfigEntry] = {}
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
        source_fetch_policy = entry.get("fetch_policy", validated_default_fetch_policy)
        validated_source_fetch_policy = validate_fetch_policy(
            value=source_fetch_policy,
            context=f"Source '{name}' fetch_policy",
        )
        registry[name] = {
            "name": name,
            "url": url,
            "fetch_policy": validated_source_fetch_policy,
        }

    return registry, validated_default_fetch_policy


def resolve_sources(
    *,
    registry: dict[str, SourceConfigEntry],
    selected_names: list[str],
) -> list[ResolvedSourceEntry]:
    if not selected_names:
        return [dict(entry) for entry in registry.values()]

    missing = [name for name in selected_names if name not in registry]
    if missing:
        names = ", ".join(sorted(missing))
        raise ValueError(f"Unknown source name(s) in config selection: {names}")

    return [dict(registry[name]) for name in selected_names]


def dedupe_sources(sources: list[ResolvedSourceEntry]) -> list[ResolvedSourceEntry]:
    seen: set[str] = set()
    deduped: list[ResolvedSourceEntry] = []
    for source in sources:
        url = source["url"]
        if url in seen:
            continue
        seen.add(url)
        deduped.append(source)
    return deduped


def _resolve_run_request(
    *,
    fixture: tuple[str, ...],
    fetch: bool,
    source_url: tuple[str, ...],
    source_config: str | None,
    source_name: tuple[str, ...],
    fetch_policy: str | None,
    output: str | None,
    sqlite_path: str | None,
) -> dict[str, object]:
    fixture_paths = list(fixture)
    if not fixture_paths and not fetch:
        raise click.UsageError(
            "Provide at least one --fixture or enable --fetch with --source-url and/or --source-config."
        )

    resolved_sources: list[ResolvedSourceEntry] = [
        {
            "name": source_url_value,
            "url": source_url_value,
            "fetch_policy": "always",
        }
        for source_url_value in source_url
    ]
    if source_name and not source_config:
        raise click.UsageError("--source-name requires --source-config.")

    if source_config:
        try:
            registry, default_fetch_policy = load_source_registry(source_config)
            resolved_sources.extend(
                resolve_sources(
                    registry=registry,
                    selected_names=list(source_name),
                )
            )
        except ValueError as error:
            raise click.UsageError(str(error)) from error
    else:
        default_fetch_policy = "always"

    if fetch_policy is not None:
        resolved_fetch_policy = fetch_policy
        resolved_sources = [
            {
                **source,
                "fetch_policy": resolved_fetch_policy,
            }
            for source in resolved_sources
        ]
    else:
        resolved_fetch_policy = default_fetch_policy

    resolved_sources = dedupe_sources(resolved_sources)
    resolved_source_urls = [source["url"] for source in resolved_sources]

    if fetch and not resolved_source_urls:
        raise click.UsageError(
            "--fetch requires at least one source from --source-url or --source-config."
        )

    return {
        "fixture_paths": fixture_paths,
        "fetch": fetch,
        "source_urls": resolved_source_urls,
        "fetch_policy": resolved_fetch_policy,
        "source_fetch_details": resolved_sources,
        "output_path": output,
        "sqlite_path": sqlite_path,
    }
