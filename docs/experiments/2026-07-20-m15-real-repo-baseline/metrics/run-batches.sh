#!/bin/sh
# M15 T24 sequential sub-batches — pre-registration §6. Do not run cargo build/test concurrently.
set -eu
cd /Users/sgj/develop/loco
METRICS=docs/experiments/2026-07-20-m15-real-repo-baseline/metrics
STAMP_LOG="$METRICS/stamps.txt"
: > "$STAMP_LOG"
echo "batch_start $(date -u +%Y-%m-%dT%H%M%SZ)" | tee -a "$STAMP_LOG"
echo "HEAD $(git rev-parse HEAD)" | tee -a "$STAMP_LOG"

run_one() {
  name="$1"
  shift
  echo "=== start $name $(date -u +%Y-%m-%dT%H%M%SZ) ===" | tee -a "$STAMP_LOG"
  # capture eval stamps that appear after this run by listing before/after
  before=$(ls -1 .loco/eval 2>/dev/null | sort | tr '\n' ' ')
  set +e
  cargo run --quiet -- eval tasks-real --repeats 3 --seed 0 "$@" \
    > "$METRICS/batch-${name}.log" 2>&1
  rc=$?
  set -e
  after=$(ls -1 .loco/eval 2>/dev/null | sort)
  new=$(echo "$after" | while read d; do echo " $before " | grep -q " $d " || echo "$d"; done)
  echo "=== end $name rc=$rc stamp(s)=[$new] $(date -u +%Y-%m-%dT%H%M%SZ) ===" | tee -a "$STAMP_LOG"
  if [ "$rc" -ne 0 ]; then
    echo "BATCH_DEATH $name rc=$rc" | tee -a "$STAMP_LOG"
    # one retry per pre-registration
    echo "=== retry $name $(date -u +%Y-%m-%dT%H%M%SZ) ===" | tee -a "$STAMP_LOG"
    before=$(ls -1 .loco/eval 2>/dev/null | sort | tr '\n' ' ')
    set +e
    cargo run --quiet -- eval tasks-real --repeats 3 --seed 0 "$@" \
      > "$METRICS/batch-${name}-retry.log" 2>&1
    rc=$?
    set -e
    after=$(ls -1 .loco/eval 2>/dev/null | sort)
    new=$(echo "$after" | while read d; do echo " $before " | grep -q " $d " || echo "$d"; done)
    echo "=== end retry $name rc=$rc stamp(s)=[$new] $(date -u +%Y-%m-%dT%H%M%SZ) ===" | tee -a "$STAMP_LOG"
    if [ "$rc" -ne 0 ]; then
      echo "STOP after failed retry $name" | tee -a "$STAMP_LOG"
      exit "$rc"
    fi
  fi
}

run_one B1 \
  --filter fd-1873-path-sep \
  --filter fd-404-min-exact-depth \
  --filter fd-535-prune \
  --filter fd-615-hidden-dot-pattern

run_one B2 \
  --filter fd-675-number-parse-error \
  --filter fd-898-strip-cwd-exec \
  --filter delta-1089-whole-file-commit

run_one B3 \
  --filter rg-1138-no-ignore-dot \
  --filter rg-1159-exit-status \
  --filter rg-1176-fixed-strings-file \
  --filter rg-1293-glob-case-insensitive

run_one B4 \
  --filter rg-1390-no-context-sep \
  --filter rg-1420-no-ignore-exclude \
  --filter rg-1466-no-ignore-files

run_one B5 \
  --filter rg-1868-passthru-context \
  --filter rg-568-leading-hyphen \
  --filter rg-740-passthru

echo "batch_all_done $(date -u +%Y-%m-%dT%H%M%SZ)" | tee -a "$STAMP_LOG"
