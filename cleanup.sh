#!/bin/bash
# TianJi repo cleanup — remove all Python/JS/agent bloat before Rust rewrite
# Run from repo root: cd /home/kita/code/tianji && bash cleanup.sh
set -e

echo "=== Removing tracked bloat from git ==="
git rm -r --cached .venv 2>/dev/null || echo "  .venv not tracked or already removed"
git rm -r --cached .agents 2>/dev/null || echo "  .agents not tracked"
git rm -r --cached .codex 2>/dev/null || echo "  .codex not tracked"
git rm -r --cached .gemini 2>/dev/null || echo "  .gemini not tracked"
git rm -r --cached .pytest_cache 2>/dev/null || echo "  .pytest_cache not tracked"
git rm -r --cached .ruff_cache 2>/dev/null || echo "  .ruff_cache not tracked"
git rm -r --cached .trellis/.backup-2026-05-13T04-45-49 2>/dev/null || echo "  .trellis/.backup-* not tracked"
git rm -r --cached .opencode/node_modules 2>/dev/null || echo "  .opencode/node_modules not tracked"
git rm -r --cached runs 2>/dev/null || echo "  runs not tracked"
git rm -r --cached tests/__pycache__ 2>/dev/null || echo "  tests/__pycache__ not tracked"
git rm -r --cached tianji/__pycache__ 2>/dev/null || echo "  tianji/__pycache__ not tracked"
git rm -r --cached .trellis/scripts/common/__pycache__ 2>/dev/null || echo "  .trellis __pycache__ not tracked"
git rm --cached pyproject.toml 2>/dev/null || echo "  pyproject.toml not tracked"
git rm --cached AGENTS.md 2>/dev/null || echo "  AGENTS.md not tracked"

# Remove .backup files from opencode
find .opencode -name '*.backup' -exec git rm --cached {} \; 2>/dev/null || true

echo ""
echo "=== Removing from disk ==="
rm -rf .venv .agents .codex .gemini .pytest_cache .ruff_cache
rm -rf .trellis/.backup-*
rm -rf .opencode/node_modules
rm -rf runs
rm -rf tests/__pycache__ tianji/__pycache__ .trellis/scripts/common/__pycache__
find .opencode -name '*.backup' -delete 2>/dev/null || true
[ -f pyproject.toml ] && rm pyproject.toml
[ -f AGENTS.md ] && rm AGENTS.md
[ -f dummy.sqlite3 ] && rm dummy.sqlite3

echo ""
echo "=== Staging .gitignore ==="
git add .gitignore

echo ""
echo "=== Status ==="
git status --short

echo ""
echo "Done. Review 'git status' above, then:"
echo "  git commit -m 'cleanup: remove Python/JS/agent bloat, add .gitignore'"
echo "  git push origin rust-cli  # if you want"
