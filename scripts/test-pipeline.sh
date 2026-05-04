#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "Starting sidecar…"
cd "$REPO_ROOT/sidecar"
bash start.sh &
SIDECAR_PID=$!

# Give models time to load
echo "Waiting 10s for sidecar to become ready…"
sleep 10

echo ""
cd "$REPO_ROOT"
cargo run --bin test_pipeline --manifest-path src-tauri/Cargo.toml
STATUS=$?

# Leave the sidecar running so subsequent test runs are faster.
# Kill it explicitly with: kill $SIDECAR_PID
echo ""
echo "Sidecar still running (pid $SIDECAR_PID) — kill manually when done."
exit $STATUS
