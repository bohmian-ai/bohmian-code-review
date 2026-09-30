#!/usr/bin/env bash
# Enforces architecture/agent-rules.md: owner tests are inline; only the public
# facade crate and the outer pipeline library, whose journeys cross crate
# boundaries through public APIs, may keep external tests under tests/.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

misplaced="$(find crates -path 'crates/*/tests/*.rs' ! -path 'crates/bohmian-code-review/tests/*' ! -path 'crates/bohmian-code-review-pipeline/tests/*' | sort)"
if [[ -n "$misplaced" ]]; then
  echo "external tests outside the facade crate (move inline, see architecture/agent-rules.md):" >&2
  echo "$misplaced" >&2
  exit 1
fi
