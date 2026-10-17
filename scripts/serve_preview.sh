#!/usr/bin/env bash
# Serve the NeuroMesh preview dashboard locally.
# Usage: bash scripts/serve_preview.sh [port]   (default 8000)
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
port="${1:-8000}"
echo "NeuroMesh preview: http://localhost:${port}/"
echo "Serving ${root}/docs/preview (Ctrl+C to stop)"
exec python3 -m http.server "${port}" --bind 0.0.0.0 --directory "${root}/docs/preview"