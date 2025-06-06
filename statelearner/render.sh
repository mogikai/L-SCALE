#!/usr/bin/env bash
SRC="$1.dot"
OUT="$1.pdf"

inotifywait -m -e modify "$SRC" |
  while read -r directory events filename; do
    dot -Tpdf "$SRC" -o "$OUT"
  done
