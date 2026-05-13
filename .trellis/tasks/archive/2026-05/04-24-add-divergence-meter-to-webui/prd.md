# Brainstorm: Add DivergenceMeter to Web UI

## Goal

Add a DivergenceMeter view or widget to TianJi's optional local web UI so operators can see a Steins;Gate-style divergence value alongside the existing TianJi run browser, without breaking the current thin-client and local-first web UI model.

## What I already know

* The current web UI is a static client in `tianji/webui/` served by `tianji/webui_server.py`.
* The current UI already supports persisted run history, run detail, explicit compare, and an optional queue-run proxy.
* The web UI contract says the browser UI should stay thin and should not redefine daemon/API/CLI contracts.
* DivergenceMeter provides a public HTTP API:
  * `GET http://divergence.nyarchlinux.moe/api/divergence` -> current divergence value
  * `GET /news` -> article list with impact/divergence/field data
  * `GET /plot` -> HTML plot output
* The existing TianJi web UI tests validate static HTML/test ids and a browser smoke flow.

## Assumptions (temporary)

* The first integration should be read-only.
* We should prefer a lightweight integration that fits the current no-build-tool static UI.
* We should avoid introducing a new mandatory dependency or a browser-heavy embed unless there is a strong reason.

## Open Questions

* Which integration level do you want for the MVP: simple value widget, value plus article/news panel, or a fuller embed/plot experience?

## Requirements (evolving)

* Add DivergenceMeter-related information to the optional TianJi web UI.
* Keep the web UI optional and local-first from TianJi's point of view.
* Preserve the current thin static-client architecture unless a stronger requirement justifies server-side proxying or richer embedding.

## Acceptance Criteria (evolving)

* [ ] The web UI shows DivergenceMeter content in a clearly identified section.
* [ ] Existing run-browser functionality continues to work.
* [ ] Web UI tests are updated to cover the new surface.

## Definition of Done (team quality bar)

* Tests added/updated (unit/integration where appropriate)
* Lint / typecheck / CI green
* Docs/notes updated if behavior changes
* Rollout/rollback considered if risky

## Out of Scope (explicit)

* Replacing TianJi's scoring model with DivergenceMeter logic
* Writing to DivergenceMeter or depending on it for core TianJi features
* Adding a frontend build toolchain

## Technical Notes

* Inspected files:
  * `tianji/webui/index.html`
  * `tianji/webui/app.js`
  * `tianji/webui/styles.css`
  * `tianji/webui_server.py`
  * `tests/test_webui.py`
  * `tests/test_webui_browser.py`
  * `.trellis/spec/backend/contracts/web-ui-contract.md`
* Current constraints:
  * static HTML/CSS/JS client
  * no frontend build pipeline
  * web UI is intentionally thin
  * existing server already proxies local API and queue-run only

## Research Notes

### What the external tool does

* DivergenceMeter exposes a current divergence number via a simple JSON endpoint.
* It also exposes article/news data and a plot endpoint.
* Its public website includes a stylized visual meter, but that appearance depends on external image assets and an external service.

### Constraints from our repo/project

* TianJi's web UI is local-only, thin, and currently centered on TianJi-owned persisted run data.
* The web UI contract explicitly avoids turning the browser into a separate backend domain model.
* The current UI stack favors small HTML/CSS/JS changes and testable DOM additions.

### Feasible approaches here

**Approach A: Lightweight divergence widget** (Recommended)

* How it works:
  * Add a new panel or hero metric in the current web UI.
  * Fetch the external `/api/divergence` value directly from the browser or through a tiny server-side proxy if needed.
  * Render the current divergence number with TianJi-owned styling.
* Pros:
  * Smallest change
  * Fits the thin-client UI contract
  * Easy to test and maintain
* Cons:
  * Only surfaces the headline value, not the richer DivergenceMeter context

**Approach B: Divergence widget plus news context**

* How it works:
  * Show the current divergence value plus a compact list of DivergenceMeter news/articles.
  * Keep the UI still TianJi-styled and read-only.
* Pros:
  * Gives users more context for the number
  * Still achievable in the current static UI model
* Cons:
  * More UI complexity
  * More reliance on an external API shape

**Approach C: Rich embed / plot-style integration**

* How it works:
  * Embed external plot/HTML or reproduce more of DivergenceMeter's meter experience.
* Pros:
  * Most visually impressive
  * Closest to the original project experience
* Cons:
  * Highest coupling to external assets/HTML
  * Harder to test, style, and keep aligned with TianJi's local-only thin UI model
