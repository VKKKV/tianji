from __future__ import annotations

from dataclasses import dataclass
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler
import json
from typing import Any, cast
from urllib.parse import parse_qs, urlsplit

from .models import RUN_ARTIFACT_SCHEMA_VERSION
from . import storage


API_VERSION = "v1"
API_META_DATA: dict[str, object] = {
    "artifact_schema_version": RUN_ARTIFACT_SCHEMA_VERSION,
    "cli_source_of_truth": True,
    "compare_resources": {
        "mirrored_backend_surface": ["history-compare"],
        "payload_fixture": "tests/fixtures/contracts/history_compare_v1.json",
        "v1_routes": [
            "GET /api/v1/compare?left_run_id=<id>&right_run_id=<id>",
        ],
    },
    "persistence": {
        "sqlite_optional": True,
    },
    "resources": [
        "/api/v1/meta",
        "/api/v1/runs",
        "/api/v1/runs/{run_id}",
        "/api/v1/compare?left_run_id=<id>&right_run_id=<id>",
    ],
}


@dataclass(frozen=True, slots=True)
class ApiError:
    code: str
    message: str
    status: HTTPStatus


class TianJiApiServerProtocol:
    sqlite_path: str


class TianJiApiRequestHandler(BaseHTTPRequestHandler):
    server_version = "TianJiLocalAPI/1.0"
    error_content_type = "application/json"

    def do_GET(self) -> None:  # noqa: N802
        try:
            payload, status = self._handle_get()
        except ApiRouteError as exc:
            payload = _error_envelope(exc.error)
            status = exc.error.status
        body = json.dumps(
            payload,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
        ).encode("utf-8")
        self.send_response(int(status))
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        return

    def _handle_get(self) -> tuple[dict[str, object], HTTPStatus]:
        split = urlsplit(self.path)
        path = split.path
        query = parse_qs(split.query, strict_parsing=False, max_num_fields=20)

        if path == "/api/v1/meta":
            return _success_envelope(
                cast(dict[str, object], _deep_copy(API_META_DATA))
            ), HTTPStatus.OK

        if path == "/api/v1/runs":
            limit = _parse_optional_positive_int(query, name="limit")
            payload = {
                "resource": "/api/v1/runs",
                "item_contract_fixture": "tests/fixtures/contracts/history_list_item_v1.json",
                "items": storage.list_runs(
                    sqlite_path=self._server.sqlite_path,
                    limit=limit if limit is not None else 20,
                ),
            }
            return _success_envelope(payload), HTTPStatus.OK

        if path == "/api/v1/runs/latest":
            latest_run_id = storage.get_latest_run_id(
                sqlite_path=self._server.sqlite_path
            )
            if latest_run_id is None:
                raise ApiRouteError(
                    ApiError(
                        code="run_not_found",
                        message="Run not found: latest",
                        status=HTTPStatus.NOT_FOUND,
                    )
                )
            payload = storage.get_run_summary(
                sqlite_path=self._server.sqlite_path,
                run_id=latest_run_id,
            )
            if payload is None:
                raise ApiRouteError(
                    ApiError(
                        code="run_not_found",
                        message=f"Run not found: {latest_run_id}",
                        status=HTTPStatus.NOT_FOUND,
                    )
                )
            return _success_envelope(payload), HTTPStatus.OK

        if path == "/api/v1/compare":
            left_run_id = _parse_required_positive_int(query, name="left_run_id")
            right_run_id = _parse_required_positive_int(query, name="right_run_id")
            payload = storage.compare_runs(
                sqlite_path=self._server.sqlite_path,
                left_run_id=left_run_id,
                right_run_id=right_run_id,
            )
            if payload is None:
                raise ApiRouteError(
                    ApiError(
                        code="run_not_found",
                        message=(
                            "Run not found for compare pair: "
                            f"left_run_id={left_run_id}, right_run_id={right_run_id}"
                        ),
                        status=HTTPStatus.NOT_FOUND,
                    )
                )
            return _success_envelope(payload), HTTPStatus.OK

        run_id = _parse_run_detail_path(path)
        if run_id is not None:
            payload = storage.get_run_summary(
                sqlite_path=self._server.sqlite_path,
                run_id=run_id,
            )
            if payload is None:
                raise ApiRouteError(
                    ApiError(
                        code="run_not_found",
                        message=f"Run not found: {run_id}",
                        status=HTTPStatus.NOT_FOUND,
                    )
                )
            return _success_envelope(payload), HTTPStatus.OK

        raise ApiRouteError(
            ApiError(
                code="route_not_found",
                message=f"Route not found: {path}",
                status=HTTPStatus.NOT_FOUND,
            )
        )

    @property
    def _server(self) -> TianJiApiServerProtocol:
        return cast(TianJiApiServerProtocol, self.server)


class ApiRouteError(Exception):
    def __init__(self, error: ApiError) -> None:
        super().__init__(error.message)
        self.error = error


def _success_envelope(data: object) -> dict[str, object]:
    return {
        "api_version": API_VERSION,
        "data": data,
        "error": None,
    }


def _error_envelope(error: ApiError) -> dict[str, object]:
    return {
        "api_version": API_VERSION,
        "data": None,
        "error": {
            "code": error.code,
            "message": error.message,
        },
    }


def _parse_run_detail_path(path: str) -> int | None:
    normalized_parts = [part for part in path.split("/") if part]
    if normalized_parts[:3] != ["api", "v1", "runs"] or len(normalized_parts) != 4:
        return None
    run_token = normalized_parts[3]
    try:
        run_id = int(run_token)
    except ValueError:
        return None
    if run_id <= 0:
        return None
    return run_id


def _parse_required_positive_int(query: dict[str, list[str]], *, name: str) -> int:
    values = query.get(name)
    if not values:
        raise ApiRouteError(
            ApiError(
                code="invalid_query",
                message=(
                    "Malformed compare query: expected positive integer query fields "
                    "'left_run_id' and 'right_run_id'"
                ),
                status=HTTPStatus.BAD_REQUEST,
            )
        )
    return _coerce_positive_int(values[-1], field_name=name)


def _parse_optional_positive_int(
    query: dict[str, list[str]], *, name: str
) -> int | None:
    values = query.get(name)
    if not values:
        return None
    return _coerce_positive_int(values[-1], field_name=name)


def _coerce_positive_int(raw_value: str, *, field_name: str) -> int:
    try:
        value = int(raw_value)
    except ValueError as exc:
        raise ApiRouteError(
            ApiError(
                code="invalid_query",
                message=f"Query field '{field_name}' must be a positive integer",
                status=HTTPStatus.BAD_REQUEST,
            )
        ) from exc
    if value <= 0:
        raise ApiRouteError(
            ApiError(
                code="invalid_query",
                message=f"Query field '{field_name}' must be a positive integer",
                status=HTTPStatus.BAD_REQUEST,
            )
        )
    return value


def _deep_copy(value: Any) -> Any:
    return json.loads(json.dumps(value, ensure_ascii=False, allow_nan=False))
