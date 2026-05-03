#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

if [ ! -d ".venv" ]; then
    echo "Creating venv…"
    python3.11 -m venv .venv
fi

source .venv/bin/activate

echo "Installing dependencies…"
pip install -q -r requirements.txt

echo "Starting sidecar on http://127.0.0.1:8091"
uvicorn server:app --host 127.0.0.1 --port 8091
