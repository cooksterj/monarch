#!/usr/bin/env bash
# Enforces the small-file rule from CLAUDE.md: Rust source files may not
# exceed MAX non-test lines (everything before `#[cfg(test)]` counts).
set -euo pipefail

MAX="${MAX_LINES:-300}"
status=0

while IFS= read -r file; do
  lines=$(awk '/^#\[cfg\(test\)\]/ { exit } { n++ } END { print n+0 }' "$file")
  if (( lines > MAX )); then
    echo "error: $file has $lines non-test lines (max $MAX); split it by responsibility"
    status=1
  fi
done < <(find src crates tools -name '*.rs' -not -path '*/target/*' 2>/dev/null)

exit $status
