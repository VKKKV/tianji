#!/usr/bin/env bash
set -euo pipefail

cargo run --quiet -- eval --manifest tests/fixtures/eval/corpus.yaml
