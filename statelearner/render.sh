#!/usr/bin/env bash
SRC="$1.dot"
OUT="$1.pdf"

while read -r directory events filename; do
  inotifywait -m -e modify "$SRC" |
  dot -Tpdf "$SRC" -o "$OUT"
done
