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
    echo "Waiting for sidecar to become ready…"
    until curl -sf http://127.0.0.1:8091/health >/dev/null 2>&1; do
        sleep 2
        if ! kill -0 "$SIDECAR_PID" 2>/dev/null; then
            echo "Sidecar crashed — check logs above."
            exit 1
        fi
    done
    echo "Sidecar ready."
fi

echo ""
cd "$REPO_ROOT"
cargo run --bin integration --manifest-path src-tauri/Cargo.toml
STATUS=$?

echo ""
if [ -n "$SIDECAR_PID" ]; then
    echo "Sidecar still running (pid $SIDECAR_PID) — kill manually when done."
fi
exit $STATUS
