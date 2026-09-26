#!/usr/bin/env bash
# Enforces architecture/agent-rules.md: owner tests are inline; only the public
# facade crate may keep external integration tests under tests/.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

misplaced="$(find crates -path 'crates/*/tests/*.rs' ! -path 'crates/bohmian-code-review/tests/*' | sort)"
if [[ -n "$misplaced" ]]; then
  echo "external tests outside the facade crate (move inline, see architecture/agent-rules.md):" >&2
  echo "$misplaced" >&2
  exit 1
fi
