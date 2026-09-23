#!/usr/bin/env sh
set -eu

talk_dir=$(CDPATH= cd -- "$(dirname "$0")" && pwd)

case "${1:-revealjs}" in
  revealjs)
    if [ "$#" -gt 0 ]; then shift; fi
    exec python3 "$talk_dir/build.py" "$@"
    ;;
  pandoc)
    shift
    if [ "$#" -ne 0 ]; then
      echo "Usage: $0 pandoc" >&2
      exit 2
    fi
    cd "$talk_dir"
    mkdir -p dist
    exec pandoc presentation.md --standalone --math-method=mathjax -o dist/pandoc.html
    ;;
  -h|--help)
    echo "Usage: $0 [revealjs [--output PATH] | pandoc]"
    echo "Default: revealjs. Pandoc writes talk/dist/pandoc.html."
    ;;
  --output|--output=*)
    # Preserve the original Reveal.js output option without an explicit mode.
    exec python3 "$talk_dir/build.py" "$@"
    ;;
  *)
    echo "Unknown build mode: $1 (choose revealjs or pandoc)" >&2
    exit 2
    ;;
esac
