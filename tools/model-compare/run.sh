#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# One comparison run: bash tools/model-compare/run.sh <arm> <issue> <base-sha>
# Arms: A-opus | B-opus-haiku | C-codex | D-opencode. Appends one JSON line to results/runs.jsonl.
set -uo pipefail
ARM=$1; N=$2; BASE=$3
MC=$(cd "$(dirname "$0")" && pwd); REPO=$(cd "$MC/../.." && git rev-parse --show-toplevel)
WT=$(dirname "$REPO")/mc-$N-$ARM; BR=mc/issue-$N/$ARM; LOG=$MC/results/logs/$N-$ARM; mkdir -p "$LOG"
export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH
git -C "$REPO" worktree remove --force "$WT" 2>/dev/null; git -C "$REPO" branch -D "$BR" 2>/dev/null
git -C "$REPO" worktree add -q -b "$BR" "$WT" "$BASE" || exit 1
cd "$WT" && git config user.name jgridifier && git config user.email jgridifier@users.noreply.github.com
git submodule update --init -q 2>/dev/null
# The gate and prompts come from the comparison branch, identical for every arm.
mkdir -p tools/model-compare .claude/agents && cp -R "$MC/gates" "$MC/prompts" tools/model-compare/ && cp "$REPO/.claude/agents/"*.md .claude/agents/ && git add tools/model-compare .claude && git commit -qm "model-compare: gate and prompts for #$N (fixed before run)" && BASE_RUN=$(git rev-parse HEAD)
PROMPT="$(sed "s/<N>/$N/g" "$MC/prompts/common.md")

$(cat "$MC/prompts/issue-$N.md")"
case $ARM in
  A-opus) PROMPT="$PROMPT

Work alone: write all the code yourself (no subagents).";;
  B-opus-haiku) PROMPT="$PROMPT

You are the orchestrator. Do not write the crate code yourself: plan the work, then delegate the implementation to the \`implementer\` subagent (Haiku, defined in .claude/agents/implementer.md) with precise briefs, review what it returns, and send it back with corrections until the gate passes. You may run commands and read files yourself.";;
esac
echo "$PROMPT" > "$LOG/prompt.txt"
start=$(date +%s); status=ok; session=""
run_agent() { # $1 = prompt text, $2 = round
  case $ARM in
    A-opus|B-opus-haiku)
      extra=(); [ "$ARM" = A-opus ] && extra=(--disallowedTools Task)
      [ -n "$session" ] && extra+=(--resume "$session")
      claude -p --model opus --dangerously-skip-permissions --output-format json "${extra[@]}" "$1" </dev/null > "$LOG/agent-$2.json" 2> "$LOG/agent-$2.err"
      session=$(python3 -c "import json;print(json.load(open('$LOG/agent-$2.json')).get('session_id',''))" 2>/dev/null);;
    C-codex)
      if [ -z "${CODEX_MODEL:-}" ] || ! codex debug models 2>/dev/null | grep -q "\"slug\": *\"$CODEX_MODEL\""; then echo "6.1 sol not in Codex catalog" > "$LOG/agent-$2.err"; status=unavailable; return; fi
      if [ -n "$session" ]; then codex exec resume "$session" --dangerously-bypass-approvals-and-sandbox -m "$CODEX_MODEL" --json "$1" </dev/null > "$LOG/agent-$2.jsonl" 2> "$LOG/agent-$2.err"
      else codex exec --dangerously-bypass-approvals-and-sandbox -m "$CODEX_MODEL" --json "$1" </dev/null > "$LOG/agent-$2.jsonl" 2> "$LOG/agent-$2.err"; fi
      session=$(grep -o '"thread_id":"[^"]*"' "$LOG/agent-$2.jsonl" | head -1 | cut -d'"' -f4);;
    D-opencode)
      set -a; . /home/box/.config/nvidia-nim.env; set +a
      extra=(); [ -n "$session" ] && extra=(--session "$session")
      opencode run --auto --print-logs --log-level INFO --format json -m nvidia/z-ai/glm-5.3 --title "issue-$N" "${extra[@]}" "$1" </dev/null > "$LOG/agent-$2.jsonl" 2> "$LOG/agent-$2.err"
      session=$(grep -o '"sessionID":"[^"]*"' "$LOG/agent-$2.jsonl" | head -1 | cut -d'"' -f4);;
  esac
}
rounds=0; first_try=false; pass=false
run_agent "$PROMPT" 0
if [ "$status" = ok ]; then
  bash tools/model-compare/gates/issue-$N.sh > "$LOG/gate-0.txt" 2>&1 && { first_try=true; pass=true; }
  while [ "$pass" = false ] && [ $rounds -lt 2 ]; do
    rounds=$((rounds+1))
    run_agent "The acceptance gate failed. Fix the problems and commit. Gate output:
$(tail -40 "$LOG/gate-$((rounds-1)).txt")" $rounds
    bash tools/model-compare/gates/issue-$N.sh > "$LOG/gate-$rounds.txt" 2>&1 && pass=true
  done
fi
wall=$(( $(date +%s) - start ))
git add -A >/dev/null 2>&1; git diff --cached --quiet || git commit -qm "model-compare: uncommitted leftovers ($ARM)"
python3 "$MC/metrics.py" --arm "$ARM" --issue "$N" --base "$BASE" --base-run "$BASE_RUN" --wt "$WT" --log "$LOG" --wall "$wall" --rounds "$rounds" --first-try "$first_try" --pass "$pass" --status "$status" >> "$MC/results/runs.jsonl"
tail -1 "$MC/results/runs.jsonl"
