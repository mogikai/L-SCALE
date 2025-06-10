#!/usr/bin/env bash
SRC="$1.dot"
OUT="$1.pdf"

while inotifywait -e close_write "$SRC"; do
    dot -Tpdf "$SRC" -o "$OUT";
done
