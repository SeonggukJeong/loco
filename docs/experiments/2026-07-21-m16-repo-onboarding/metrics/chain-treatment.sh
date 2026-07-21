#!/bin/sh
# Wait for control arm to finish, then run treatment arm (M16 pre-reg order).
set -eu
cd /Users/sgj/develop/loco
METRICS=docs/experiments/2026-07-21-m16-repo-onboarding/metrics
CTRL_LOG="$METRICS/stamps-control.txt"
CHAIN_LOG="$METRICS/chain.log"

log() { echo "$(date -u +%Y-%m-%dT%H%M%SZ) $*" | tee -a "$CHAIN_LOG"; }

log "chain_wait_control_start"
# Poll until control done or failure stop
while true; do
  if [ -f "$CTRL_LOG" ]; then
    if grep -q 'arm=control batch_all_done' "$CTRL_LOG"; then
      log "control_done"
      break
    fi
    if grep -q 'STOP after failed retry' "$CTRL_LOG"; then
      log "control_STOP — not starting treatment"
      exit 1
    fi
  fi
  # also exit if no control process and no done line after launch
  sleep 60
done

# switch config to treatment
cat > .loco/config.toml << 'EOF'
# M16 treatment arm — pre-registration §2-2
context_tokens = 8192
max_output_tokens = 4096
command_timeout_secs = 60
base_url = "http://localhost:8080/v1"
repo_notes = true
EOF
cp .loco/config.toml "$METRICS/preflight-config-treatment.toml"
log "config_switched_to_treatment"
grep repo_notes .loco/config.toml | tee -a "$CHAIN_LOG"

# quick model still up
if ! curl -sf http://127.0.0.1:8080/v1/models >/dev/null; then
  log "server_down — restarting"
  GGUF=~/.lmstudio/models/deepreinforce-ai/Ornith-1.0-9B-GGUF/ornith-1.0-9b-Q4_K_M.gguf
  pgrep -x llama-server | xargs kill 2>/dev/null || true
  sleep 2
  LOCO_MODEL_GGUF="$GGUF" LOCO_CTX=37632 scripts/serve.sh \
    >> "$METRICS/serve-37632.log" 2>&1 &
  for i in $(seq 1 60); do
    curl -sf http://127.0.0.1:8080/v1/models >/dev/null && break
    sleep 2
  done
fi

# treatment preflight note
{
  echo "treatment_start $(date -u +%Y-%m-%dT%H%M%SZ)"
  echo "HEAD $(git rev-parse HEAD)"
  curl -s http://127.0.0.1:8080/v1/models | head -c 300
  echo
  grep n_ctx_slot "$METRICS/serve-37632.log" | tail -3
} | tee "$METRICS/preflight-treatment.txt"

log "treatment_batch_launch"
ARM=treatment /bin/sh "$METRICS/run-batches.sh" \
  > "$METRICS/run-batches-treatment.wrapper.log" 2>&1
rc=$?
log "treatment_batch_exit rc=$rc"
exit "$rc"
