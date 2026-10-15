#!/usr/bin/env bash
# The execution modes must agree: bench audio with the JIT, with the block interpreter
# (FM1_NO_JIT=1) and a step at a time (FM1_NO_BLOCKS=1) is byte-identical over the common
# length (a bench can end an audio half earlier or later: the run loop checks its budget per
# block). The per-step mode runs single-core firmwares only (stock: n/a).
#   tools/check_modes.sh [ms=5000] [firmware ids...]   (default: felucca x0x ghoulbox stock)
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="${BIN:-target/release/fm1-live}"
ms="${1:-5000}"; [ $# -gt 0 ] && shift
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
failed=0
for f in ${*:-felucca x0x ghoulbox stock}; do
  src=(--fw "$f"); [ "$f" = stock ] && src=(--fwsc reference/FM-1-RE/firmware-images/v13/raw_fw/FM-1.fwsc)
  for mode in jit interp step; do
    if [ "$mode" = step ] && grep -q "^cpu1:" "$out/$f-jit.log"; then continue; fi
    e=(); [ $mode = interp ] && e=(FM1_NO_JIT=1); [ $mode = step ] && e=(FM1_NO_BLOCKS=1)
    if ! env ${e[@]+"${e[@]}"} FM1_BENCH_WAV="$out/$f-$mode.raw" "$BIN" "${src[@]}" --bench "$ms" >"$out/$f-$mode.log" 2>&1; then
      printf "%s %s FAILED\n" "$f" "$mode" >&2
      failed=1
    fi
  done
  printf "%-14s" "$f"
  for mode in interp step; do
    b="$out/$f-$mode.raw"
    if [ "$mode" = step ] && grep -q "^cpu1:" "$out/$f-jit.log"; then printf "  step=n/a (dual core)"; continue; fi
    if [ ! -s "$b" ] || [ ! -s "$out/$f-jit.raw" ]; then printf "  %s=EMPTY" "$mode"; failed=1; continue; fi
    c="$(cmp "$out/$f-jit.raw" "$b" 2>&1)"
    if [ -z "$c" ] || grep -q EOF <<<"$c"; then printf "  %s=same" $mode; else printf "  %s=DIFFERENT" $mode; failed=1; fi
  done
  echo
done
exit "$failed"
