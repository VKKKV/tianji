const state = {
  apiBaseUrl: new URLSearchParams(window.location.search).get("api_base_url") || "",
  queuePath: "/queue-run",
  selectedRunId: null,
  compareSelection: { left: null, right: null },
};

const elements = {
  metaApiVersion: document.getElementById("meta-api-version"),
  metaSchemaVersion: document.getElementById("meta-schema-version"),
  metaCliSource: document.getElementById("meta-cli-source"),
  historyForm: document.getElementById("history-form"),
  historyLimit: document.getElementById("history-limit"),
  historyStatus: document.getElementById("history-status"),
  runHistoryList: document.getElementById("run-history-list"),
  runHistoryItemTemplate: document.getElementById("run-history-item-template"),
  loadLatestRun: document.getElementById("load-latest-run"),
  runDetailView: document.getElementById("run-detail-view"),
  compareForm: document.getElementById("compare-form"),
  compareLeftRunId: document.getElementById("compare-left-run-id"),
  compareRightRunId: document.getElementById("compare-right-run-id"),
  compareStatus: document.getElementById("compare-status"),
  compareView: document.getElementById("compare-view"),
  queueRunForm: document.getElementById("queue-run-form"),
  queueFixturePath: document.getElementById("queue-fixture-path"),
  queueRunStatus: document.getElementById("queue-run-status"),
};

document.addEventListener("DOMContentLoaded", () => {
  wireEvents();
  bootstrap();
});

function wireEvents() {
  elements.historyForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    await loadHistory();
  });

  elements.loadLatestRun.addEventListener("click", async () => {
    await loadLatestRun();
  });

  elements.compareForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    const leftRunId = Number(elements.compareLeftRunId.value);
    const rightRunId = Number(elements.compareRightRunId.value);
    state.compareSelection = { left: leftRunId, right: rightRunId };
    await loadCompare(leftRunId, rightRunId);
  });

  elements.queueRunForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    await queueRun();
  });
}

async function bootstrap() {
  renderEmptyDetail("Select a run from history or open the latest persisted run.");
  renderEmptyCompare("Compare two persisted runs through the explicit-pair route.");
  setStatus(elements.queueRunStatus, "Queue proxy is idle.", "success");
  await Promise.all([loadMeta(), loadHistory()]);
}

async function loadMeta() {
  try {
    const payload = await apiGet("/api/v1/meta");
    const data = payload.data;
    elements.metaApiVersion.textContent = payload.api_version;
    elements.metaSchemaVersion.textContent = data.artifact_schema_version;
    elements.metaCliSource.textContent = data.cli_source_of_truth ? "CLI is source-of-truth" : "Unavailable";
  } catch (error) {
    elements.metaApiVersion.textContent = "Unavailable";
    elements.metaSchemaVersion.textContent = "Unavailable";
    elements.metaCliSource.textContent = getErrorMessage(error);
  }
}

async function loadHistory() {
  const limit = Number(elements.historyLimit.value) || 20;
  setStatus(elements.historyStatus, "Loading persisted run history…", "success");
  try {
    const payload = await apiGet(`/api/v1/runs?limit=${encodeURIComponent(limit)}`);
    const data = payload.data;
    const items = Array.isArray(data.items) ? data.items : [];
    renderRunHistory(items);
    if (items.length === 0) {
      setStatus(elements.historyStatus, "No persisted runs found yet.", "success");
      return;
    }
    setStatus(elements.historyStatus, `Loaded ${items.length} persisted run${items.length === 1 ? "" : "s"}.`, "success");
    if (state.selectedRunId === null) {
      await loadRunDetail(items[0].run_id);
    }
  } catch (error) {
    elements.runHistoryList.innerHTML = "";
    renderEmptyDetail("Run detail will appear here once a persisted run loads.");
    setStatus(elements.historyStatus, getErrorMessage(error), "error");
  }
}

function renderRunHistory(items) {
  elements.runHistoryList.innerHTML = "";
  for (const item of items) {
    const fragment = elements.runHistoryItemTemplate.content.cloneNode(true);
    const listItem = fragment.querySelector(".run-card");
    const button = fragment.querySelector(".run-card-button");
    listItem.setAttribute("data-testid", `run-row-${item.run_id}`);
    fragment.querySelector(".run-card-topline").textContent = `Run #${item.run_id} • ${item.generated_at}`;
    fragment.querySelector(".run-card-headline").textContent = item.headline;
    fragment.querySelector(".run-card-metrics").textContent = `${item.dominant_field} • risk ${item.risk_level} • top Δ ${formatNumber(item.top_divergence_score)}`;
    if (item.run_id === state.selectedRunId) {
      button.classList.add("is-selected");
    }
    button.addEventListener("click", async () => {
      await loadRunDetail(item.run_id);
    });
    elements.runHistoryList.appendChild(fragment);
  }
}

async function loadRunDetail(runId) {
  state.selectedRunId = runId;
  renderRunHistorySelection(runId);
  renderEmptyDetail(`Loading run #${runId}…`);
  try {
    const payload = await apiGet(`/api/v1/runs/${encodeURIComponent(runId)}`);
    const data = payload.data;
    renderRunDetail(data);
    primeCompareForm(runId);
  } catch (error) {
    renderEmptyDetail(getErrorMessage(error));
  }
}

async function loadLatestRun() {
  renderEmptyDetail("Loading the latest persisted run…");
  try {
    const payload = await apiGet("/api/v1/runs/latest");
    const data = payload.data;
    state.selectedRunId = data.run_id;
    renderRunHistorySelection(data.run_id);
    renderRunDetail(data);
    primeCompareForm(data.run_id);
  } catch (error) {
    renderEmptyDetail(getErrorMessage(error));
  }
}

function renderRunHistorySelection(runId) {
  for (const button of elements.runHistoryList.querySelectorAll(".run-card-button")) {
    button.classList.remove("is-selected");
    if (button.textContent.includes(`Run #${runId}`)) {
      button.classList.add("is-selected");
    }
  }
}

function renderRunDetail(data) {
  const scenarioSummary = data.scenario_summary || {};
  const eventGroups = Array.isArray(scenarioSummary.event_groups) ? scenarioSummary.event_groups : [];
  const scoredEvents = Array.isArray(data.scored_events) ? data.scored_events : [];
  const interventions = Array.isArray(data.intervention_candidates) ? data.intervention_candidates : [];
  const topInterventions = interventions.slice(0, 6).map((item) => `
    <li class="intervention-card">
      <h3>${escapeHtml(item.intervention_type)} → ${escapeHtml(item.target || "unknown")}</h3>
      <p class="detail-meta">Priority ${escapeHtml(String(item.priority))} • event ${escapeHtml(item.event_id)}</p>
      <p>${escapeHtml(item.expected_effect || "")}</p>
      <p class="mini-text">${escapeHtml(item.reason || "")}</p>
    </li>
  `).join("");
  const topEvents = scoredEvents.slice(0, 4).map((item) => `
    <li class="event-card">
      <h3>${escapeHtml(item.title)}</h3>
      <p class="detail-meta">${escapeHtml(item.dominant_field)} • Δ ${formatNumber(item.divergence_score)} • Im ${formatNumber(item.impact_score)} • Fa ${formatNumber(item.field_attraction)}</p>
      <p class="mini-text">${escapeHtml(item.source || "")} • ${escapeHtml(item.published_at || "")}</p>
      ${item.link ? `<p><a href="${escapeAttribute(item.link)}" target="_blank" rel="noreferrer">Open source link</a></p>` : ""}
    </li>
  `).join("");
  const groupMarkup = eventGroups.length === 0
    ? '<div class="empty-state">No grouped intervention clusters are present for this run.</div>'
    : eventGroups.map((group) => `
      <div class="detail-card">
        <h3>${escapeHtml(group.headline_title || group.headline_event_id || "Event group")}</h3>
        <p class="detail-meta">${escapeHtml(group.dominant_field || "unknown field")} • members ${escapeHtml(String(group.member_count ?? 0))} • score ${formatNumber(group.group_score)}</p>
        <p>${escapeHtml(group.chain_summary || group.causal_summary || "")}</p>
      </div>
    `).join("");

  elements.runDetailView.innerHTML = `
    <div class="detail-stack">
      <div class="detail-card">
        <p class="section-kicker">Run #${escapeHtml(String(data.run_id))}</p>
        <h2>${escapeHtml(scenarioSummary.headline || "No headline available")}</h2>
        <p class="detail-meta">${escapeHtml(data.mode || "unknown")} • generated ${escapeHtml(data.generated_at || "unknown")}</p>
      </div>

      <div class="summary-grid">
        ${renderMetricCard("Dominant field", scenarioSummary.dominant_field)}
        ${renderMetricCard("Risk level", scenarioSummary.risk_level)}
        ${renderMetricCard("Raw items", data.input_summary?.raw_item_count)}
        ${renderMetricCard("Normalized", data.input_summary?.normalized_event_count)}
      </div>

      <div class="detail-grid">
        <section class="detail-card">
          <p class="section-kicker">Top actors</p>
          <div class="chip-row">${renderChipRow(scenarioSummary.top_actors)}</div>
        </section>
        <section class="detail-card">
          <p class="section-kicker">Top regions</p>
          <div class="chip-row">${renderChipRow(scenarioSummary.top_regions)}</div>
        </section>
      </div>

      <section class="detail-stack">
        <div class="panel-header">
          <div>
            <p class="section-kicker">Interventions</p>
            <h3>Candidate interventions</h3>
          </div>
        </div>
        <ul class="intervention-list" data-testid="intervention-list">${topInterventions || '<li class="empty-state">No interventions available.</li>'}</ul>
      </section>

      <section class="detail-stack">
        <div class="panel-header">
          <div>
            <p class="section-kicker">Scored events</p>
            <h3>Top event signals</h3>
          </div>
        </div>
        <ul class="event-list">${topEvents || '<li class="empty-state">No scored events available.</li>'}</ul>
      </section>

      <section class="detail-stack">
        <div class="panel-header">
          <div>
            <p class="section-kicker">Grouped analysis</p>
            <h3>Event groups</h3>
          </div>
        </div>
        ${groupMarkup}
      </section>
    </div>
  `;
}

function renderMetricCard(label, value) {
  return `
    <div class="summary-metric">
      <div class="metric-label">${escapeHtml(label)}</div>
      <div class="metric-value">${escapeHtml(value == null ? "—" : String(value))}</div>
    </div>
  `;
}

function renderChipRow(values) {
  if (!Array.isArray(values) || values.length === 0) {
    return '<span class="chip">None</span>';
  }
  return values.map((value) => `<span class="chip">${escapeHtml(String(value))}</span>`).join("");
}

async function loadCompare(leftRunId, rightRunId) {
  setStatus(elements.compareStatus, `Loading compare pair ${leftRunId} → ${rightRunId}…`, "success");
  renderEmptyCompare("Loading compare results…");
  try {
    const payload = await apiGet(`/api/v1/compare?left_run_id=${encodeURIComponent(leftRunId)}&right_run_id=${encodeURIComponent(rightRunId)}`);
    renderCompare(payload.data);
    setStatus(elements.compareStatus, `Loaded compare pair ${leftRunId} and ${rightRunId}.`, "success");
  } catch (error) {
    setStatus(elements.compareStatus, getErrorMessage(error), "error");
    renderEmptyCompare(getErrorMessage(error));
  }
}

async function queueRun() {
  const fixturePath = elements.queueFixturePath.value.trim();
  if (!fixturePath) {
    setStatus(elements.queueRunStatus, "Fixture path is required.", "error");
    return;
  }
  setStatus(elements.queueRunStatus, `Queueing local run for ${fixturePath}…`, "success");
  try {
    const payload = await postJson(state.queuePath, {
      fixture_path: fixturePath,
    });
    setStatus(
      elements.queueRunStatus,
      `Queued ${payload.data.job_id} with state ${payload.data.state}.`,
      "success",
    );
  } catch (error) {
    setStatus(elements.queueRunStatus, getErrorMessage(error), "error");
  }
}

function renderCompare(data) {
  elements.compareView.innerHTML = `
    <div class="compare-grid">
      ${renderCompareSide("Left run", data.left)}
      ${renderCompareSide("Right run", data.right)}
    </div>
    <section class="detail-card compare-diff">
      <p class="section-kicker">Diff summary</p>
      <h3>Frozen compare payload signals</h3>
      <ul class="list-reset">
        <li>Top scored event changed: ${escapeHtml(String(data.diff?.top_scored_event_changed))}</li>
        <li>Top intervention changed: ${escapeHtml(String(data.diff?.top_intervention_changed))}</li>
        <li>Top event group changed: ${escapeHtml(String(data.diff?.top_event_group_changed))}</li>
        <li>Dominant field changed: ${escapeHtml(String(data.diff?.dominant_field_changed))}</li>
        <li>Risk level changed: ${escapeHtml(String(data.diff?.risk_level_changed))}</li>
        <li>Top divergence delta: ${escapeHtml(String(data.diff?.top_divergence_score_delta ?? "—"))}</li>
        <li>Event group count delta: ${escapeHtml(String(data.diff?.event_group_count_delta ?? "—"))}</li>
      </ul>
    </section>
  `;
}

function renderCompareSide(label, side) {
  const topEvent = side?.top_scored_event || null;
  const topIntervention = side?.top_intervention || null;
  const topGroup = side?.top_event_group || null;
  return `
    <section class="compare-card">
      <p class="section-kicker">${escapeHtml(label)}</p>
      <h3>Run #${escapeHtml(String(side?.run_id ?? "?"))}</h3>
      <p>${escapeHtml(side?.headline || "No headline available")}</p>
      <p class="detail-meta">${escapeHtml(side?.dominant_field || "unknown")} • risk ${escapeHtml(side?.risk_level || "unknown")} • raw ${escapeHtml(String(side?.raw_item_count ?? "—"))}</p>
      <div class="detail-stack">
        <div>
          <p class="section-kicker">Top scored event</p>
          <p>${escapeHtml(topEvent?.title || "None")}</p>
        </div>
        <div>
          <p class="section-kicker">Top intervention</p>
          <p>${escapeHtml(topIntervention ? `${topIntervention.intervention_type} → ${topIntervention.target}` : "None")}</p>
        </div>
        <div>
          <p class="section-kicker">Top event group</p>
          <p>${escapeHtml(topGroup?.headline_title || topGroup?.headline_event_id || "None")}</p>
        </div>
      </div>
    </section>
  `;
}

function renderEmptyDetail(message) {
  elements.runDetailView.innerHTML = `<div class="empty-state detail-empty">${escapeHtml(message)}</div>`;
}

function renderEmptyCompare(message) {
  elements.compareView.innerHTML = `<div class="empty-state">${escapeHtml(message)}</div>`;
}

function primeCompareForm(selectedRunId) {
  const currentLeft = elements.compareLeftRunId.value.trim();
  const currentRight = elements.compareRightRunId.value.trim();
  if (!currentLeft) {
    elements.compareLeftRunId.value = String(selectedRunId);
    return;
  }
  if (!currentRight) {
    elements.compareRightRunId.value = String(selectedRunId);
  }
}

async function apiGet(pathname) {
  const basePath = state.apiBaseUrl || "";
  const response = await fetch(`${basePath}${pathname}`, {
    headers: {
      Accept: "application/json",
    },
  });
  const payload = await response.json();
  if (!response.ok || payload.error) {
    throw new Error(payload.error?.message || `Request failed: ${response.status}`);
  }
  return payload;
}

async function postJson(pathname, body) {
  const response = await fetch(pathname, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(body),
  });
  const payload = await response.json();
  if (!response.ok || payload.error) {
    throw new Error(payload.error?.message || `Request failed: ${response.status}`);
  }
  return payload;
}

function setStatus(element, message, stateName) {
  element.textContent = message;
  element.dataset.state = stateName;
}

function formatNumber(value) {
  return typeof value === "number" ? value.toFixed(2) : "—";
}

function getErrorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function escapeAttribute(value) {
  return escapeHtml(value);
}
