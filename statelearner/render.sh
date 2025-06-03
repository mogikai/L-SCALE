#!/usr/bin/env bash

SRC="test.dot"
OUT="test.pdf"

while true; do
  echo "$SRC" |  entr -c dot -Tpdf "$SRC" -o "$OUT"
done
