# Phase 4.3: Nerd Font / ASCII Fallback

> Part of plan.md §5.4 Phase 4 TUI Completion
> Target: auto-detect terminal glyph support, fall back to ASCII
> Status: spec

## Goal

Replace hardcoded Unicode/Nerd Font glyphs with a runtime-selectable glyph set.
Auto-detect when to use ASCII fallback. Currently only `↑/↓` in the status bar
is affected; this also sets up the pattern for future glyph use (simulation view etc.).

## Detection

At TUI init, determine glyph mode:

1. If env `TIANJI_NERD_FONT=1` → use Nerd Font glyphs
2. Else if `TERM_PROGRAM` is `kitty`, `ghostty`, `wezterm`, `alacritty` → use Nerd Font
3. Else → fall back to ASCII

## GlyphSet struct

```rust
struct GlyphSet {
    up: &'static str,
    down: &'static str,
    nav_hint: &'static str,   // "[↑/↓]" or "[j/k]"
    bullet: &'static str,
    warning: &'static str,
}

const NERD_GLYPHS: GlyphSet = GlyphSet {
    up: "↑",
    down: "↓",
    nav_hint: "[↑/↓]",
    bullet: "•",
    warning: "!",
};

const ASCII_GLYPHS: GlyphSet = GlyphSet {
    up: "^",
    down: "v",
    nav_hint: "[j/k]",
    bullet: "-",
    warning: "!",
};
```

## TuiState change

Add `glyphs: &'static GlyphSet` field. Set at init time via `detect_glyph_mode()`.

## Rendering change

Replace hardcoded `"↑/↓"` string with `self.glyphs.nav_hint`.

In `render_status_bar` and `render_history`, use `state.glyphs` for any glyph strings.

## Files Changed

- `src/tui.rs` — GlyphSet, detection, TuiState field, rendering substitution

## Tests

- Unit: `detect_glyph_mode()` with env set returns NERD
- Unit: `detect_glyph_mode()` without env returns ASCII (in CI)
- Unit: ASCII glyph set renders without Unicode

## Verification

- `cargo build` zero error
- `cargo test` all pass
- Manual: `TIANJI_NERD_FONT=1 cargo run -- tui ...` shows Unicode arrows
