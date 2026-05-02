#!/bin/bash
# Starts the Python sidecar (if not already running) then runs the Rust integration test.
# Usage: ./test-backend.sh

set -e
ROOT="$(cd "$(dirname "$0")" && pwd)"

# ── 1. Sidecar ───────────────────────────────────────────────────────────────
if curl -sf http://127.0.0.1:8091/health > /dev/null 2>&1; then
    echo "[sidecar] already running"
else
    echo "[sidecar] starting…"
    cd "$ROOT/sidecar"
    "$ROOT/.venv/bin/uvicorn" server:app --host 127.0.0.1 --port 8091 \
        > /tmp/sidecar.log 2>&1 &
    SIDECAR_PID=$!
    echo "[sidecar] PID $SIDECAR_PID — waiting for /health…"

    until curl -sf http://127.0.0.1:8091/health > /dev/null 2>&1; do
        sleep 2
        # Bail if the process died
        if ! kill -0 $SIDECAR_PID 2>/dev/null; then
            echo "[sidecar] crashed — check /tmp/sidecar.log"
            cat /tmp/sidecar.log
            exit 1
        fi
    done
    echo "[sidecar] ready"
fi

# ── 2. Integration test ──────────────────────────────────────────────────────
echo ""
cd "$ROOT/src-tauri"
cargo run --bin integration
