from __future__ import annotations

from rich.layout import Layout
from rich.panel import Panel
from rich.text import Text

from .tui_state import (
    HistoryListState,
    coerce_int,
    format_lens_change_message,
)


def build_right_panel(state: HistoryListState, width: int, page_size: int) -> Panel:
    if state.active_view == "compare":
        return build_compare_panel(state, width, page_size)
    return build_detail_panel(state, width, page_size)


def build_compare_panel(state: HistoryListState, width: int, page_size: int) -> Panel:
    title_text = " Compare "
    if not state.rows or state.staged_compare_left_run_id is None:
        content = Text("Select a second run to compare.")
    else:
        selected_row = state.rows[state.selected_index]
        right_run_id = coerce_int(selected_row.get("run_id"))
        title_text = f" Compare L:{state.staged_compare_left_run_id} R:{right_run_id} "

        if right_run_id == state.staged_compare_left_run_id:
            content = Text("Cannot compare a run with itself.\nSelect a different run.")
        else:
            if state.cached_compare_lines:
                total_lines = len(state.cached_compare_lines)
                if total_lines > page_size:
                    end_line = min(state.detail_scroll_offset + page_size, total_lines)
                    title_text = f"{title_text.rstrip()} {state.detail_scroll_offset + 1}-{end_line}/{total_lines} "

                visible_lines = state.cached_compare_lines[
                    state.detail_scroll_offset : state.detail_scroll_offset + page_size
                ]
                lines = [Text(line) for line in visible_lines]
                content = Text("\n").join(lines)
            else:
                content = Text("")

    title = (
        Text(f" [{title_text.strip()}] ", style="reverse bold")
        if state.focused_pane != "list"
        else Text(title_text, style="bold")
    )
    return Panel(content, title=title, title_align="left")


def build_layout(
    state: HistoryListState, height: int, width: int, page_size: int
) -> Layout:
    state.ensure_selection_visible(page_size=page_size)

    layout = Layout()
    layout.split_column(
        Layout(name="header", size=1),
        Layout(name="body"),
        Layout(name="message", size=1),
        Layout(name="footer", size=1),
    )

    status_line = (
        " TianJi | j/k move | h/l focus | [/] step | a/s/d/f/v lens view"
        f" | {format_active_lens_summary(state)} | Tab/Enter zoom | ? help | q quit "
    )
    layout["header"].update(Text(status_line.ljust(width), style="reverse"))

    if state.show_help:
        help_text = build_help_text()
        layout["body"].update(Panel(help_text, title="TianJi TUI Help", expand=False))
    else:
        if width < 60 or (state.zoomed and state.focused_pane == "list"):
            layout["body"].update(build_list_panel(state, width, page_size))
        elif state.zoomed and state.focused_pane != "list":
            layout["body"].update(build_right_panel(state, width, page_size))
        else:
            left_width = min(width // 2 + 10, 80)
            layout["body"].split_row(
                Layout(name="left", size=left_width), Layout(name="right")
            )
            layout["left"].update(build_list_panel(state, left_width, page_size))
            layout["right"].update(
                build_right_panel(state, width - left_width, page_size)
            )

    if state.message and height > 3:
        msg = f" {state.message} "
        layout["message"].update(Text(msg.ljust(len(msg)), style="reverse"))
    else:
        layout["message"].update(Text(""))

    footer_text = format_status_footer(state, width)
    layout["footer"].update(Text(footer_text, style="reverse"))

    return layout


def build_list_panel(state: HistoryListState, width: int, page_size: int) -> Panel:
    inner_width = max(width - 2, 1)
    visible_rows = state.rows[state.scroll_offset : state.scroll_offset + page_size]
    lines = []
    for index, row in enumerate(visible_rows):
        absolute_index = state.scroll_offset + index
        run_id = coerce_int(row.get("run_id"))
        is_staged_left = run_id == state.staged_compare_left_run_id
        row_text = format_history_row(
            row, width=inner_width, is_staged_left=is_staged_left
        )
        style = ""
        if absolute_index == state.selected_index:
            if state.focused_pane == "list":
                style = "reverse"
            else:
                style = "underline"
        lines.append(Text(row_text, style=style))

    content = Text("\n").join(lines) if lines else Text("")
    title = (
        Text(" [ Runs ] ", style="reverse bold")
        if state.focused_pane == "list"
        else Text(" Runs ", style="bold")
    )
    return Panel(content, title=title, title_align="left")


def build_detail_panel(state: HistoryListState, width: int, page_size: int) -> Panel:
    if not state.rows:
        content = Text("")
    else:
        if state.cached_detail_lines:
            visible_lines = state.cached_detail_lines[
                state.detail_scroll_offset : state.detail_scroll_offset + page_size
            ]
            lines = [Text(line) for line in visible_lines]
            content = Text("\n").join(lines)
        else:
            content = Text("")

    title = (
        Text(" [ Details ] ", style="reverse bold")
        if state.focused_pane == "detail"
        else Text(" Details ", style="bold")
    )
    return Panel(content, title=title, title_align="left")


def build_help_text() -> Text:
    help_lines = [
        " Navigation:",
        "   j / Down    : Move down",
        "   k / Up      : Move up",
        "   PgDn / PgUp : Page down / up",
        "   g / G       : Jump to top / bottom",
        "",
        " Panes & Zoom:",
        "   h / l       : Focus List / Detail pane",
        "   Tab         : Toggle pane focus",
        "   [ / ]       : Previous / Next run",
        "   Enter / z   : Toggle zoom on focused pane",
        "",
        " Compare:",
        "   c           : Stage/activate compare",
        "   C           : Clear compare",
        "",
        " Lenses:",
        "   a           : Cycle scored-event field lens",
        "   s           : Cycle scored-event limit lens",
        "   d           : Cycle event-group field lens",
        "   f           : Cycle event-group limit lens",
        "   v           : Toggle intervention-match lens",
        "   Active lens  : Projects detail/compare, list stays persisted",
        "",
        " General:",
        "   ?           : Toggle this help",
        "   q           : Quit / Close help / Unzoom",
    ]
    return Text("\n".join(help_lines))


def format_status_footer(state: HistoryListState, width: int) -> str:
    total = len(state.rows)
    if total == 0:
        return " 0/0 ".ljust(width)

    current = state.selected_index + 1
    run_id = state.rows[state.selected_index].get("run_id", "?")

    if total == 1:
        bounds = "[only]"
    elif current == 1:
        bounds = "[first]"
    elif current == total:
        bounds = "[last]"
    else:
        bounds = "[-]"

    focus = state.focused_pane.upper()
    zoom = "ZOOM" if state.zoomed else ""

    compare_state = ""
    if state.staged_compare_left_run_id is not None:
        if state.active_view == "compare":
            compare_state = f"COMPARE L:{state.staged_compare_left_run_id} R:{run_id}"
        else:
            compare_state = f"COMPARE L:{state.staged_compare_left_run_id}"

    lens_state = format_active_lens_summary(state)

    left = f" run {current}/{total} | id:{run_id} | {bounds} "

    right_parts = []
    if compare_state:
        right_parts.append(compare_state)
    if lens_state != "lens:all-runs":
        right_parts.append(f"VIEW {lens_state.upper()}")
    if zoom:
        right_parts.append(zoom)
    right_parts.append(focus)

    right = " | ".join(right_parts)
    right = f" {right} "

    if len(left) + len(right) <= width:
        return left + " " * (width - len(left) - len(right)) + right

    compact_lens = "" if lens_state == "lens:all-runs" else lens_state
    compact = (
        f" run {current}/{total} id:{run_id} {bounds}"
        f" {compare_state} {compact_lens} {zoom} {focus} "
    )
    compact = " ".join(compact.split())
    return shorten_text(compact.strip(), width).ljust(width)


def format_history_row(
    row: dict[str, object], *, width: int, is_staged_left: bool = False
) -> str:
    run_id = coerce_int(row.get("run_id"))
    generated_at = str(row.get("generated_at", ""))[:16].replace("T", " ")
    mode = str(row.get("mode", ""))
    dominant_field = str(row.get("dominant_field", "uncategorized"))
    risk_level = str(row.get("risk_level", "low"))
    event_group_count = coerce_int(row.get("event_group_count"))
    top_event_group_dominant_field = str(
        row.get("top_event_group_dominant_field") or ""
    )
    top_event_group_member_count = row.get("top_event_group_member_count")
    top_scored_event_dominant_field = str(
        row.get("top_scored_event_dominant_field") or ""
    )
    top_impact_score = format_optional_score(row.get("top_impact_score"))
    top_divergence_score = format_optional_score(row.get("top_divergence_score"))
    headline = str(row.get("headline", ""))

    marker = "*" if is_staged_left else " "
    prefix = (
        f"{marker}{run_id:>3} {generated_at:<16} {mode:<8.8} {dominant_field:<10.10} "
        f"{risk_level:<4.4}"
    )

    triage_segments = [
        f"G:{event_group_count}",
        f"Dv:{top_divergence_score}",
        f"Im:{top_impact_score}",
    ]
    if top_scored_event_dominant_field:
        triage_segments.append(f"Top:{top_scored_event_dominant_field[:8]}")
    if event_group_count > 0 and top_event_group_dominant_field:
        member_count = (
            coerce_int(top_event_group_member_count)
            if isinstance(top_event_group_member_count, int | float | str)
            else 0
        )
        triage_segments.append(
            f"Grp:{top_event_group_dominant_field[:8]}/{member_count}"
        )

    min_headline_width = 12
    for segment in triage_segments:
        candidate = f"{prefix} {segment}"
        if len(candidate) + min_headline_width <= width:
            prefix = candidate

    prefix = f"{prefix} "
    if len(prefix) >= width:
        return shorten_text(prefix.rstrip(), width).ljust(width)
    available_headline_width = max(width - len(prefix), 0)
    text = prefix + shorten_text(headline, available_headline_width)
    return text.ljust(width)


def format_active_lens_summary(state: HistoryListState) -> str:
    parts = []
    if state.dominant_field is not None:
        parts.append(f"ev={state.dominant_field}")
    if state.limit_scored_events is not None:
        parts.append(f"top={state.limit_scored_events}")
    if state.group_dominant_field is not None:
        parts.append(f"grp={state.group_dominant_field}")
    if state.limit_event_groups is not None:
        parts.append(f"groups={state.limit_event_groups}")
    if state.only_matching_interventions:
        parts.append("matching-interventions")
    if not parts:
        return "lens:all-runs"
    return "lens:" + ",".join(parts)


def has_active_projection_lens(state: HistoryListState) -> bool:
    return format_active_lens_summary(state) != "lens:all-runs"


def format_projected_empty_message(state: HistoryListState, subject: str) -> str:
    lens_summary = format_active_lens_summary(state)
    if lens_summary == "lens:all-runs":
        return f"No persisted {subject} view is available."
    return f"No {subject} rows match the active lens. Persisted run data is unchanged."


def format_projected_empty_slice_message(slice_name: str) -> str:
    return (
        f"No {slice_name} rows match the active lens. Persisted run data is unchanged."
    )


def build_detail_projected_empty_messages(
    summary: dict[str, object], *, state: HistoryListState
) -> dict[str, str]:
    messages: dict[str, str] = {}
    if not has_active_projection_lens(state):
        return messages

    scenario = summary.get("scenario_summary")
    event_groups = scenario.get("event_groups") if isinstance(scenario, dict) else None
    scored_events = summary.get("scored_events")
    interventions = summary.get("intervention_candidates")

    if (
        (state.group_dominant_field is not None or state.limit_event_groups is not None)
        and isinstance(event_groups, list)
        and not event_groups
    ):
        messages["event_groups"] = format_projected_empty_slice_message("event-group")

    if (
        (state.dominant_field is not None or state.limit_scored_events is not None)
        and isinstance(scored_events, list)
        and not scored_events
    ):
        messages["scored_events"] = format_projected_empty_slice_message("scored-event")

    if (
        state.only_matching_interventions
        and isinstance(interventions, list)
        and not interventions
    ):
        messages["interventions"] = format_projected_empty_slice_message("intervention")

    return messages


def build_compare_projected_empty_messages(
    compare_result: dict[str, object], *, state: HistoryListState
) -> dict[str, list[str]]:
    messages = {"left": [], "right": []}
    if not has_active_projection_lens(state):
        return messages

    left = compare_result.get("left")
    right = compare_result.get("right")
    if isinstance(left, dict):
        messages["left"] = build_compare_side_projected_empty_messages(
            left, state=state
        )
    if isinstance(right, dict):
        messages["right"] = build_compare_side_projected_empty_messages(
            right, state=state
        )
    return messages


def build_compare_side_projected_empty_messages(
    side: dict[str, object], *, state: HistoryListState
) -> list[str]:
    messages: list[str] = []

    if (
        (state.group_dominant_field is not None or state.limit_event_groups is not None)
        and side.get("top_event_group") is None
        and side.get("event_group_count") == 0
    ):
        messages.append(format_projected_empty_slice_message("event-group"))

    if (
        state.dominant_field is not None or state.limit_scored_events is not None
    ) and side.get("top_scored_event") is None:
        messages.append(format_projected_empty_slice_message("scored-event"))

    intervention_event_ids = side.get("intervention_event_ids")
    if (
        state.only_matching_interventions
        and side.get("top_intervention") is None
        and isinstance(intervention_event_ids, list)
        and not intervention_event_ids
    ):
        messages.append(format_projected_empty_slice_message("intervention"))

    return messages


def format_event_group_preview_lines(
    group: dict[str, object],
    *,
    rank: int,
    width: int,
) -> list[str]:
    dominant_field = str(group.get("dominant_field", "uncategorized"))
    member_count = group.get("member_count", 0)
    headline_title = str(group.get("headline_title", ""))
    causal_summary = str(group.get("causal_summary", ""))

    summary_line = shorten_text(
        f" {rank}. {dominant_field} ({member_count} members)", width
    )
    snippet = causal_summary if causal_summary else headline_title
    if not snippet:
        snippet = "No summary available."
    snippet_line = shorten_text(f"    {snippet}", width)
    return [summary_line, snippet_line]


def format_compare_side_summaries(
    side: dict[str, object],
    *,
    width: int,
    projected_empty_messages: list[str] | None = None,
) -> list[str]:
    lines = []
    top_group = side.get("top_event_group")
    if isinstance(top_group, dict):
        dominant_field = str(top_group.get("dominant_field", "uncategorized"))
        member_count = top_group.get("member_count", 0)
        lines.append(
            shorten_text(
                f"  Top Group: {dominant_field} ({member_count} members)", width
            )
        )

    top_event = side.get("top_scored_event")
    if isinstance(top_event, dict):
        dominant_field = str(top_event.get("dominant_field", "uncategorized"))
        dv = format_optional_score(top_event.get("divergence_score"))
        im = format_optional_score(top_event.get("impact_score"))
        lines.append(
            shorten_text(f"  Top Event: {dominant_field} Dv {dv} Im {im}", width)
        )

    top_intervention = side.get("top_intervention")
    if isinstance(top_intervention, dict):
        target = str(top_intervention.get("target", "unknown"))
        itype = str(top_intervention.get("intervention_type", "unknown"))
        lines.append(shorten_text(f"  Top Action: [{itype}] {target}", width))

    if projected_empty_messages:
        for message in projected_empty_messages:
            lines.extend(wrap_text(f"  {message}", width))

    return lines


def format_top_group_evidence_diff_lines(
    evidence_diff: dict[str, object], *, width: int
) -> list[str]:
    lines = []
    comparable = evidence_diff.get("comparable", False)
    mode_text = "Comparable" if comparable else "Contrast"
    lines.append(shorten_text(f"  • Top group evidence ({mode_text}):", width))

    member_delta = evidence_diff.get("member_count_delta", 0)
    link_delta = evidence_diff.get("evidence_chain_link_count_delta", 0)
    if isinstance(member_delta, int) and isinstance(link_delta, int):
        lines.append(
            shorten_text(
                f"      Members: {member_delta:+d}, Links: {link_delta:+d}", width
            )
        )

    added_ids = evidence_diff.get("right_only_member_event_ids", [])
    if isinstance(added_ids, list) and added_ids:
        lines.append(
            shorten_text(f"      Added IDs: {', '.join(map(str, added_ids))}", width)
        )

    removed_ids = evidence_diff.get("left_only_member_event_ids", [])
    if isinstance(removed_ids, list) and removed_ids:
        lines.append(
            shorten_text(
                f"      Removed IDs: {', '.join(map(str, removed_ids))}", width
            )
        )

    added_kw = evidence_diff.get("shared_keywords_added", [])
    if isinstance(added_kw, list) and added_kw:
        lines.append(
            shorten_text(
                f"      Added keywords: {', '.join(map(str, added_kw))}", width
            )
        )

    removed_kw = evidence_diff.get("shared_keywords_removed", [])
    if isinstance(removed_kw, list) and removed_kw:
        lines.append(
            shorten_text(
                f"      Removed keywords: {', '.join(map(str, removed_kw))}", width
            )
        )

    if evidence_diff.get("chain_summary_changed"):
        lines.append(shorten_text("      Chain summary changed", width))

    return lines


def get_compare_similarity_summary(diff: dict[str, object]) -> str | None:
    if not diff:
        return None

    major_flags = [
        diff.get("dominant_field_changed"),
        diff.get("risk_level_changed"),
        diff.get("top_event_group_changed"),
        diff.get("top_scored_event_changed"),
        diff.get("top_intervention_changed"),
    ]
    if any(major_flags):
        return None

    item_delta = diff.get("raw_item_count_delta", 0)
    norm_delta = diff.get("normalized_event_count_delta", 0)
    group_delta = diff.get("event_group_count_delta", 0)
    has_count_changes = bool(item_delta or norm_delta or group_delta)

    dv_delta = diff.get("top_divergence_score_delta")
    im_delta = diff.get("top_impact_score_delta")
    fa_delta = diff.get("top_field_attraction_delta")

    def is_nonzero(val: object) -> bool:
        if not isinstance(val, int | float):
            return False
        return abs(float(val)) > 0.001

    has_score_changes = (
        is_nonzero(dv_delta) or is_nonzero(im_delta) or is_nonzero(fa_delta)
    )

    evidence_diff = diff.get("top_event_group_evidence_diff")
    has_evidence_changes = False
    if isinstance(evidence_diff, dict):
        has_evidence_changes = (
            bool(evidence_diff.get("member_count_delta"))
            or bool(evidence_diff.get("evidence_chain_link_count_delta"))
            or bool(evidence_diff.get("right_only_member_event_ids"))
            or bool(evidence_diff.get("left_only_member_event_ids"))
            or bool(evidence_diff.get("shared_keywords_added"))
            or bool(evidence_diff.get("shared_keywords_removed"))
            or bool(evidence_diff.get("chain_summary_changed"))
        )

    if not has_count_changes and not has_score_changes and not has_evidence_changes:
        return "Effectively identical: no meaningful differences found."

    return "No major differences: top signals and fields remain stable."


def format_compare_detail(
    compare_result: dict[str, object],
    *,
    width: int,
    projected_empty_messages: dict[str, list[str]] | None = None,
) -> list[str]:
    lines = []
    left = compare_result.get("left", {})
    right = compare_result.get("right", {})
    diff = compare_result.get("diff", {})

    if (
        not isinstance(left, dict)
        or not isinstance(right, dict)
        or not isinstance(diff, dict)
    ):
        return ["Invalid compare result."]

    left_id = left.get("run_id")
    right_id = right.get("run_id")
    lines.append(
        shorten_text(
            f"Compare: Run #{left_id} (Left) vs Run #{right_id} (Right)", width
        )
    )

    similarity_summary = get_compare_similarity_summary(diff)
    if similarity_summary:
        lines.append("")
        lines.append(shorten_text(f"Summary: {similarity_summary}", width))

    lines.append("")

    left_field = left.get("dominant_field", "uncategorized")
    left_risk = left.get("risk_level", "low")
    lines.append(
        shorten_text(
            f"[Left] {left.get('mode')} • {left_field} • Risk: {left_risk}", width
        )
    )
    left_headline = str(left.get("headline", ""))
    if left_headline:
        lines.extend(wrap_text(f"       {left_headline}", width))
    left_empty_messages = (
        projected_empty_messages.get("left", [])
        if isinstance(projected_empty_messages, dict)
        else []
    )
    lines.extend(
        format_compare_side_summaries(
            left,
            width=width,
            projected_empty_messages=left_empty_messages,
        )
    )
    lines.append("")

    right_field = right.get("dominant_field", "uncategorized")
    right_risk = right.get("risk_level", "low")
    lines.append(
        shorten_text(
            f"[Right] {right.get('mode')} • {right_field} • Risk: {right_risk}", width
        )
    )
    right_headline = str(right.get("headline", ""))
    if right_headline:
        lines.extend(wrap_text(f"        {right_headline}", width))
    right_empty_messages = (
        projected_empty_messages.get("right", [])
        if isinstance(projected_empty_messages, dict)
        else []
    )
    lines.extend(
        format_compare_side_summaries(
            right,
            width=width,
            projected_empty_messages=right_empty_messages,
        )
    )
    lines.append("")

    lines.append(shorten_text("Diff Highlights:", width))

    if diff.get("dominant_field_changed"):
        lines.append(
            shorten_text(f"  • Field changed: {left_field} -> {right_field}", width)
        )
    if diff.get("risk_level_changed"):
        lines.append(
            shorten_text(f"  • Risk changed: {left_risk} -> {right_risk}", width)
        )

    item_delta = diff.get("raw_item_count_delta", 0)
    norm_delta = diff.get("normalized_event_count_delta", 0)
    if isinstance(item_delta, int) and isinstance(norm_delta, int):
        lines.append(
            shorten_text(
                f"  • Items: {item_delta:+d} raw, {norm_delta:+d} normalized", width
            )
        )

    group_delta = diff.get("event_group_count_delta", 0)
    if isinstance(group_delta, int):
        lines.append(shorten_text(f"  • Event Groups: {group_delta:+d}", width))

    if diff.get("top_event_group_changed"):
        lines.append(shorten_text("  • Top event group changed", width))

    evidence_diff = diff.get("top_event_group_evidence_diff")
    if isinstance(evidence_diff, dict):
        lines.extend(format_top_group_evidence_diff_lines(evidence_diff, width=width))

    if diff.get("top_scored_event_changed"):
        lines.append(shorten_text("  • Top scored event changed", width))
    elif diff.get("top_scored_event_comparable"):
        dv_delta = format_delta(diff.get("top_divergence_score_delta"))
        im_delta = format_delta(diff.get("top_impact_score_delta"))
        fa_delta = format_delta(diff.get("top_field_attraction_delta"))
        lines.append(
            shorten_text(
                f"  • Top event score deltas: Dv {dv_delta} Im {im_delta} Fa {fa_delta}",
                width,
            )
        )

    if diff.get("top_intervention_changed"):
        lines.append(shorten_text("  • Top intervention changed", width))

    return lines


def format_delta(value: object) -> str:
    if not isinstance(value, int | float):
        return "N/A"
    return f"{float(value):+.2f}"


def format_run_detail(
    summary: dict[str, object],
    *,
    width: int,
    projected_empty_messages: dict[str, str] | None = None,
) -> list[str]:
    lines = []
    run_id = summary.get("run_id")
    generated_at = str(summary.get("generated_at", ""))[:19].replace("T", " ")
    mode = summary.get("mode")
    lines.append(shorten_text(f"Run #{run_id} • {generated_at} • {mode}", width))
    lines.append("")

    input_summary = summary.get("input_summary")
    if isinstance(input_summary, dict):
        raw_count = input_summary.get("raw_item_count", 0)
        norm_count = input_summary.get("normalized_event_count", 0)
        lines.append(
            shorten_text(f"Items: {raw_count} raw -> {norm_count} normalized", width)
        )

    scenario = summary.get("scenario_summary")
    if isinstance(scenario, dict):
        dominant_field = scenario.get("dominant_field", "uncategorized")
        risk_level = scenario.get("risk_level", "low")
        lines.append(
            shorten_text(f"Scenario: {dominant_field} • Risk: {risk_level}", width)
        )

        headline = str(scenario.get("headline", ""))
        if headline:
            lines.append("")
            lines.extend(wrap_text(headline, width))

        event_groups = scenario.get("event_groups")
        if isinstance(event_groups, list):
            lines.append("")
            lines.append(shorten_text(f"Event Groups: {len(event_groups)}", width))
            event_groups_empty_message = (
                projected_empty_messages.get("event_groups")
                if isinstance(projected_empty_messages, dict)
                else None
            )
            if event_groups_empty_message:
                lines.extend(wrap_text(event_groups_empty_message, width))
            for group_index, group in enumerate(event_groups[:3], start=1):
                if not isinstance(group, dict):
                    continue
                lines.extend(
                    format_event_group_preview_lines(
                        group,
                        rank=group_index,
                        width=width,
                    )
                )

    scored_events = summary.get("scored_events")
    if isinstance(scored_events, list):
        lines.append("")
        lines.append(shorten_text(f"Scored Events: {len(scored_events)}", width))
        scored_events_empty_message = (
            projected_empty_messages.get("scored_events")
            if isinstance(projected_empty_messages, dict)
            else None
        )
        if scored_events_empty_message:
            lines.extend(wrap_text(scored_events_empty_message, width))
        for event_index, event in enumerate(scored_events[:3], start=1):
            if not isinstance(event, dict):
                continue
            lines.extend(
                format_scored_event_preview_lines(
                    event,
                    rank=event_index,
                    width=width,
                )
            )

    interventions = summary.get("intervention_candidates")
    if isinstance(interventions, list):
        lines.append("")
        lines.append(shorten_text(f"Interventions: {len(interventions)}", width))
        interventions_empty_message = (
            projected_empty_messages.get("interventions")
            if isinstance(projected_empty_messages, dict)
            else None
        )
        if interventions_empty_message:
            lines.extend(wrap_text(interventions_empty_message, width))
        for intervention_index, intervention in enumerate(interventions[:3], start=1):
            if not isinstance(intervention, dict):
                continue
            lines.extend(
                format_intervention_preview_lines(
                    intervention,
                    rank=intervention_index,
                    width=width,
                )
            )

    return lines


def wrap_text(text: str, width: int) -> list[str]:
    if width <= 0:
        return []
    words = text.split()
    lines = []
    current_line: list[str] = []
    current_length = 0
    for word in words:
        if current_length + len(word) + len(current_line) > width:
            if current_line:
                lines.append(" ".join(current_line))
                current_line = [word]
                current_length = len(word)
            else:
                lines.append(word[:width])
                current_line = []
                current_length = 0
        else:
            current_line.append(word)
            current_length += len(word)
    if current_line:
        lines.append(" ".join(current_line))
    return lines


def format_scored_event_preview_lines(
    event: dict[str, object],
    *,
    rank: int,
    width: int,
) -> list[str]:
    dominant_field = str(event.get("dominant_field", "uncategorized"))
    divergence_score = format_optional_score(event.get("divergence_score"))
    impact_score = format_optional_score(event.get("impact_score"))
    field_attraction = format_optional_score(event.get("field_attraction"))
    title = str(event.get("title", ""))

    summary_line = shorten_text(
        f" {rank}. {dominant_field} Dv {divergence_score} Im {impact_score} Fa {field_attraction}",
        width,
    )
    title_line = shorten_text(f"    {title}", width)
    return [summary_line, title_line]


def format_intervention_preview_lines(
    intervention: dict[str, object],
    *,
    rank: int,
    width: int,
) -> list[str]:
    target = str(intervention.get("target", "unknown"))
    intervention_type = str(intervention.get("intervention_type", "unknown"))
    snippet = str(intervention.get("expected_effect", ""))
    if not snippet:
        snippet = str(intervention.get("reason", ""))

    summary_line = shorten_text(f" {rank}. [{intervention_type}] {target}", width)
    snippet_line = shorten_text(f"    {snippet}", width)
    return [summary_line, snippet_line]


def format_optional_score(value: object) -> str:
    if not isinstance(value, int | float):
        return "-"
    return f"{float(value):.2f}"


def shorten_text(value: str, max_width: int) -> str:
    if max_width <= 0:
        return ""
    if len(value) <= max_width:
        return value
    if max_width == 1:
        return "…"
    return value[: max_width - 1] + "…"
