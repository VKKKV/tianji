# Phase 3.3: More Actor Profiles

> Add realistic profiles for multi-agent simulation
> Target: 8+ profiles across 3 tiers for meaningful simulations
> Status: spec

## New Profiles

### nations/ (3 new)
- `usa.yaml` — USA: military 0.90, economic 0.95, technological 0.92
- `russia.yaml` — Russia: military 0.80, economic 0.55, technological 0.60
- `iran.yaml` — Iran: military 0.55, economic 0.30, technological 0.40

### organizations/ (1 new)
- `un.yaml` — UN: influence 0.70, diplomatic 0.85, no military

### corporations/ (1 new)
- `tsmc.yaml` — TSMC: market_share 0.90, supply_chain 0.85

## Profile Format

Same YAML format as existing examples. Use realistic interests, red_lines, behavior_patterns, and historical_analogues for each.

## Updated ProfileRegistry capability handling

Organization tier should handle missing military capability (set to 0.0).
Corporation tier uses market_share + supply_chain instead of military/diplomatic/cyber.

## Files

- `profiles/nations/usa.yaml`
- `profiles/nations/russia.yaml`
- `profiles/nations/iran.yaml`
- `profiles/organizations/un.yaml`
- `profiles/corporations/tsmc.yaml`

No code changes needed — just YAML data files.

## Verification

- `cargo build` passes (profiles loaded at runtime)
- `cargo test` — existing profile tests pass with new profiles loaded
