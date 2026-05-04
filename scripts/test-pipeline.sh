#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if curl -sf http://127.0.0.1:8091/health >/dev/null 2>&1; then
    echo "Sidecar already running — skipping start."
    SIDECAR_PID=""
else
    echo "Starting sidecar…"
    cd "$REPO_ROOT/sidecar"
    bash start.sh &
    SIDECAR_PID=$!
    echo "Waiting 10s for sidecar to become ready…"
    sleep 10
fi

echo ""
cd "$REPO_ROOT"
cargo run --bin test_pipeline --manifest-path src-tauri/Cargo.toml
STATUS=$?

# Leave the sidecar running so subsequent test runs are faster.
# Kill it explicitly with: kill $SIDECAR_PID
echo ""
if [ -n "$SIDECAR_PID" ]; then
    echo "Sidecar still running (pid $SIDECAR_PID) — kill manually when done."
fi
exit $STATUS
