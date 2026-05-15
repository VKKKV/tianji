# Phase 3.1: CLI Subcommands — predict, backtrack, baseline, watch

> Connects Phase 2 Hongmeng/Nuwa to user-facing CLI
> Target: add `tianji predict`, `tianji backtrack`, `tianji baseline`, `tianji watch`
> Status: spec

## Goal

Add 4 CLI subcommands that expose the Hongmeng/Nuwa infrastructure to the user.
All use stub agent actions for now (real LLM calls work but need API keys).

## Commands

### `tianji predict`

Run forward simulation from current worldline state.

```
tianji predict
    --field <region.domain>        # e.g. "east-asia.conflict"
    --horizon <ticks>              # default 30
    [--profile-dir <path>]         # default "profiles/"
    [--config <path>]              # default ~/.tianji/config.yaml
    [--max-interventions <n>]      # for backward mode
```

Output: JSON `SimulationOutcome` with branches, probabilities, event sequences.

### `tianji backtrack`

Run backward constraint search to find intervention paths toward goal.

```
tianji backtrack
    --goal "<description>"         # e.g. "东亚区域稳定，贸易正常化"
    [--field-constraint <region.domain>:<min>:<max>]  # repeatable
    [--max-interventions <n>]      # default 5
    [--profile-dir <path>]
    [--config <path>]
```

Output: JSON `Vec<InterventionPath>` sorted by path_score desc.

### `tianji baseline`

Manage worldline baseline for divergence tracking.

```
tianji baseline --set              # lock current worldline as baseline
tianji baseline --show             # show current baseline info
tianji baseline --clear            # remove baseline
```

Baseline is stored in hot-memory JSON alongside delta data.

### `tianji watch`

Daemon mode: poll RSS feeds, run pipeline on new items, check triggers.

```
tianji watch
    --source-url <url>             # RSS/Atom feed URL
    [--interval <seconds>]         # default 300
    [--sqlite-path <path>]
    [--config <path>]
```

Stub implementation: loop with sleep, call `run_fixture_path` for now (live
feed fetching via reqwest deferred). The daemon already handles the watch loop.

## Implementation

### src/main.rs

Add 4 new clap subcommands to the existing `Cli` enum:

```rust
enum Cli {
    // ... existing ...
    Predict {
        #[arg(long)]
        field: String,
        #[arg(long, default_value = "30")]
        horizon: u64,
        #[arg(long, default_value = "profiles/")]
        profile_dir: String,
        #[arg(long)]
        config: Option<String>,
    },
    Backtrack {
        #[arg(long)]
        goal: String,
        #[arg(long = "field-constraint", value_parser = parse_field_constraint)]
        field_constraints: Vec<(FieldKey, f64, f64)>,
        #[arg(long, default_value = "5")]
        max_interventions: usize,
        #[arg(long, default_value = "profiles/")]
        profile_dir: String,
        #[arg(long)]
        config: Option<String>,
    },
    Baseline {
        #[arg(long)]
        set: bool,
        #[arg(long)]
        show: bool,
        #[arg(long)]
        clear: bool,
    },
    Watch {
        #[arg(long = "source-url")]
        source_url: String,
        #[arg(long, default_value = "300")]
        interval: u64,
        #[arg(long)]
        sqlite_path: Option<String>,
        #[arg(long)]
        config: Option<String>,
    },
}
```

### Handler functions (in main.rs or new src/cli_handlers.rs)

```rust
fn handle_predict(field: &str, horizon: u64, profile_dir: &str, config_path: Option<&str>) -> Result<String, TianJiError> {
    // 1. Load profiles
    // 2. Load config (or default)
    // 3. Create Worldline from current state (stub: empty worldline)
    // 4. Create agents from profiles
    // 5. Run NuwaSandbox::run_forward()
    // 6. Serialize outcome as JSON
}

fn handle_backtrack(goal: &str, constraints: &[(FieldKey, f64, f64)], max_interventions: usize, profile_dir: &str, config_path: Option<&str>) -> Result<String, TianJiError> {
    // Similar — run_backward()
}

fn handle_baseline(action: BaselineAction) -> Result<String, TianJiError> {
    // Read/write baseline from hot-memory
}

fn handle_watch(source_url: &str, interval: u64, sqlite_path: Option<&str>, config_path: Option<&str>) -> Result<String, TianJiError> {
    // Loop: fetch feed, run pipeline, check triggers, sleep
}
```

## Stub behavior

- `predict`: creates stub worldline + agents, runs forward sim (stub actions), outputs JSON
- `backtrack`: same stub agents, runs backward search, outputs intervention paths
- `baseline --set`: writes placeholder to hot-memory (worldline not persisted yet)
- `watch`: 3-iteration loop with sleep, uses fixture path for now

## Tests

- CLI parse test: `predict --field east-asia.conflict --horizon 10`
- CLI parse test: `backtrack --goal "test" --field-constraint conflict:0:0.5`
- CLI parse test: `baseline --set` / `baseline --show`
- CLI parse test: `watch --source-url https://example.com/feed.xml`
- Integration: predict output is valid JSON with branches
- Integration: backtrack output has intervention paths

## Files Changed

- `src/main.rs` — add Cli variants + handler dispatch + handler functions
- No new files (handlers stay in main.rs for now, sub-200 lines each)

## Verification

- `cargo build` zero error
- `cargo test` all pass (244+)
- `cargo clippy -- -D warnings` clean
