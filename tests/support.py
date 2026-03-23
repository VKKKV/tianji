from __future__ import annotations

import contextlib
import io
import json
from pathlib import Path
import sqlite3
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from tempfile import TemporaryDirectory
from typing import cast
import unittest
from unittest import mock

from tianji.cli import main
from tianji.backtrack import EventGroupSummary, backtrack_candidates
from tianji.fetch import TianJiInputError
from tianji.models import NormalizedEvent, ScoredEvent
from tianji import pipeline as pipeline_module
from tianji import storage
from tianji.tui import (
    HistoryListState,
    format_history_row,
    launch_history_tui,
    format_run_detail,
    wrap_text,
    format_status_footer,
    format_compare_detail,
    format_delta,
    build_compare_panel,
)
from tianji.pipeline import run_pipeline
from tianji.scoring import score_event, summarize_scenario

FIXTURE_PATH = Path(__file__).parent / "fixtures" / "sample_feed.xml"
