from __future__ import annotations

import sys
import termios
import tty
from collections.abc import Callable
from typing import Any

from rich.console import Console
from rich.live import Live

from .storage import list_runs
from .tui_render import (
    build_compare_panel,
    build_layout,
    format_compare_detail,
    format_delta,
    format_history_row,
    format_run_detail,
    format_status_footer,
    wrap_text,
)
from .tui_state import HistoryListState, handle_history_browser_key


def getch() -> str:
    fd = sys.stdin.fileno()
    old_settings = termios.tcgetattr(fd)
    try:
        tty.setraw(fd)
        ch = sys.stdin.read(1)
        if ch == "\x1b":
            ch += sys.stdin.read(2)
            if ch.endswith("5") or ch.endswith("6"):
                ch += sys.stdin.read(1)
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)
    return ch


def launch_history_tui(*, sqlite_path: str, limit: int) -> int:
    rows = list_runs(sqlite_path=sqlite_path, limit=limit)
    if not rows:
        print("No persisted runs are available for the TUI browser.")
        return 0
    state = HistoryListState(rows=rows, sqlite_path=sqlite_path)
    run_history_list_browser(state)
    return 0


def run_history_list_browser(state: HistoryListState) -> None:
    console = Console()
    with Live(screen=True, auto_refresh=False, console=console) as live:
        run_history_browser_session(state, console=console, live=live)


def run_history_browser_session(
    state: HistoryListState,
    *,
    console: Console,
    live: Any,
    read_key: Callable[[], str] | None = None,
) -> None:
    key_reader = getch if read_key is None else read_key
    while True:
        height = console.size.height
        width = console.size.width
        page_size = max(height - 5, 1)

        state.prepare_active_view_cache(width=width)
        layout = build_layout(state, height, width, page_size)
        live.update(layout, refresh=True)

        key = key_reader()
        decision = handle_history_browser_key(state, key=key, page_size=page_size)
        if decision.should_exit:
            return
