#!/usr/bin/env sh
set -eu

# External Markdown is fetched by reveal.js, so the presentation must be served
# over HTTP instead of opened directly from the filesystem.
cd "$(dirname "$0")"
python3 -m http.server "${1:-8000}" --bind 127.0.0.1
