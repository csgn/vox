#!/usr/bin/env bash
# Core crates must depend only on other vox crates. Dev-dependencies are not checked.
# Third-party libraries belong in adapter crates (see docs/src/dev/dependencies.md).
set -euo pipefail
core=(vox-core vox-view vox-editor)
fail=0
for c in "${core[@]}"; do
  bad=$(cargo tree -p "$c" -e normal --prefix none --format '{p}' \
        | awk '{print $1}' | sort -u | grep -v '^vox-' || true)
  if [ -n "$bad" ]; then
    echo "error: $c depends on third-party crates:"
    echo "$bad" | sed 's/^/  /'
    fail=1
  fi
done
if [ "$fail" -eq 0 ]; then
  echo "ok: ${core[*]} depend only on vox crates"
fi
exit "$fail"
