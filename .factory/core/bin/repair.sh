#!/usr/bin/env bash
# Repair of the agent factory: fixes, on the base branch, the cause of a technical stop of an issue, then resumes it.
#
#   .factory/core/bin/repair.sh <issue>
#
# Started by factory-repair.yml when the supervisor posts a repair comment (supervisor.mjs, REPAIRED and spent
# retries): a failed gate, a blocked environment, an error that relaunches did not cure. A repair agent
# reads the stop, the factory's agent logs and the run log, and fixes the cause on a branch from the base
# branch: this project's factory extensions (.factory/local/), code, tests or documentation. The fix must
# pass the whole suite (run_all_tests); then this script merges it and relaunches the issue with a
# supervisor comment, which starts the factory workflow at the stage where it stopped. When the agent finds
# no fix, or the fix fails, the supervisor gives up on the issue (one comment and factory:needs-human).
#
# Never changed by a repair: CI, dependencies, the factory core (.factory/core/, managed by the installer: a
# bug there is reported for the factory's own repository), the project's skills and agent instructions
# (AGENTS.md, CLAUDE.md, skill folders). A recurring agent mistake is fixed with a rule in
# .factory/local/prompts/<stage>.md, which adds to the core prompt and to the skills without removing anything.
set -euo pipefail

FACTORY_BIN="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$FACTORY_BIN/lib.sh"
# shellcheck source=gates.sh
source "$FACTORY_BIN/gates.sh"

ISSUE=${1:-}
[[ "$ISSUE" =~ ^[0-9]+$ ]] || { sed -n '2,4p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
cd "$(git rev-parse --show-toplevel)"
load_config
for cmd in claude gh git jq ${FACTORY_REQUIRED_TOOLS[@]+"${FACTORY_REQUIRED_TOOLS[@]}"}; do command -v "$cmd" >/dev/null || die "$cmd is required"; done

MODEL=${FACTORY_MODEL:-claude-opus-5-5}
EFFORT=${FACTORY_EFFORT:-high}
MAX_TURNS=${FACTORY_REPAIR_MAX_TURNS:-200}
ATTEMPTS=${FACTORY_REPAIR_ATTEMPTS:-2}
REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
TRUSTED_ACTORS=${FACTORY_TRUSTED_ACTORS:-${REPO%%/*}}
LOG_DIR=${FACTORY_LOG_DIR:-$(mktemp -d)}; mkdir -p "$LOG_DIR"
CTX=.factory-repair # context for the agent and its summary; never committed (.gitignore)
# Everything sensitive except this project's own factory extensions, which are where a repair belongs.
forbidden() { local p; while IFS= read -r p; do case $p in .factory/local/*) continue ;; esac; ! is_sensitive "$p" || echo "$p"; done; }
FAIL_REASON=error FAIL_DETAIL=''
export ISSUE FACTORY_BASE_BRANCH LOG_DIR

[ -z "$(git status --porcelain)" ] || die "the working tree is not clean"

# The repair being answered: the last supervisor repair comment on the issue.
REPAIR=$(gh api --paginate "repos/$REPO/issues/$ISSUE/comments?per_page=100" | jq -s -c --arg t "$TRUSTED_ACTORS" '
  ($t | split(",")) as $trusted | [.[][] | select(.user.login as $a | $trusted | index($a)) |
  .body | capture("<!-- factory-supervisor (?<m>\\{\"action\":\"repair\".*?\\}) -->").m | fromjson] | last // {}')
PROGRESS=$(jq -r '.progress // ""' <<<"$REPAIR")
REASON=$(jq -r '.reason // ""' <<<"$REPAIR")
log "repair of #$ISSUE: stop $REASON at ${PROGRESS:-?}, logs $LOG_DIR"

supervisor_comment() { # supervisor_comment <marker JSON> <with factory marker: 0|1> < body
  { [ "$2" = 0 ] || printf '%s\n' "$FACTORY_MARKER"
    printf '<!-- factory-supervisor %s -->\n' "$1"; cat
    [ -z "${GITHUB_RUN_ID:-}" ] || printf '\nRepair run: %s/%s/actions/runs/%s\n' "$GITHUB_SERVER_URL" "$GITHUB_REPOSITORY" "$GITHUB_RUN_ID"
  } | gh issue comment "$ISSUE" --body-file - >/dev/null
}

give_up() { # give_up <why>: the supervisor stops on this issue until the owner answers
  local meta
  meta=$(jq -cn --arg p "$PROGRESS" '{action: "give-up", reason: "repair-failed", progress: $p}')
  printf '%s\n\nAnswer in a comment to resume the factory.\n' "The repair could not fix the stop above: $1" | supervisor_comment "$meta" 1
  set_stage_label needs-human
  log "gave up: $1"
}

# Any unexpected failure ends the repair as a give-up, so the supervisor does not wait for it.
on_exit() {
  local code=$?
  [ "$code" -ne 0 ] || return 0
  set +e
  # A usage limit is no reason to give up: the supervisor starts another repair once this one has timed out.
  [ "$FAIL_REASON" != usage-limit ] || { log "stopped by the usage limit"; return 0; }
  give_up "${FAIL_DETAIL:-the repair script failed (exit status $code)}"
}
trap on_exit EXIT
trap 'trap - EXIT; exit 130' INT TERM

# ---- context for the agent ----

gather_context() {
  local run branch="ai-dev/issue-$ISSUE"
  rm -rf "$CTX"; mkdir -p "$CTX/agent-logs"
  gh issue view "$ISSUE" --json number,title,body,author,comments | jq -r --arg t "$TRUSTED_ACTORS" '
    ($t | split(",")) as $trusted |
    "# Issue #\(.number) \(.title)\n\n" +
    (if (.author.login as $a | $trusted | index($a)) then .body else "_Body by an untrusted author, not shown._" end) +
    "\n\n## Last comments by trusted authors (factory stops, supervisor decisions, owner answers)\n" +
    ([.comments[] | select(.author.login as $a | $trusted | index($a))] | .[-12:] |
      map("\n### \(.author.login) at \(.createdAt)\n\n\(.body)\n") | join(""))' >"$CTX/issue.md"
  run=$(gh run list --workflow "${FACTORY_WORKFLOW:-factory.yml}" --limit 200 --json databaseId,displayTitle,conclusion |
    jq -r --arg t "factory #$ISSUE" '[.[] | select(.displayTitle == $t and .conclusion != "skipped")][0].databaseId // empty')
  if [ -n "$run" ]; then
    echo "$run" >"$CTX/run-id"
    gh run download "$run" -D "$CTX/agent-logs" >/dev/null 2>&1 || true
    gh run view "$run" --log 2>/dev/null | gate_log | tail -800 >"$CTX/run.log" || true
  fi
  if git fetch -q origin "$branch" 2>/dev/null; then
    {
      echo "# Branch $branch"
      echo; echo "## Commits not in $FACTORY_BASE_BRANCH"; git log --oneline "origin/$FACTORY_BASE_BRANCH..origin/$branch" | head -40
      echo; echo "## Files changed against $FACTORY_BASE_BRANCH"; git diff --stat "origin/$FACTORY_BASE_BRANCH...origin/$branch" | tail -60
      echo; echo "## state.json"; git show "origin/$branch:$(git ls-tree -r --name-only "origin/$branch" -- "$CHANGES_DIR" |
        grep -E "issue-$ISSUE-[^/]*/state\.json$" | head -1)" 2>/dev/null || echo "(none)"
    } >"$CTX/branch.md"
  fi
}

run_repair_agent() { # run_repair_agent <attempt> <feedback>
  local tools prompt
  CHANGE_DIR=$CTX # deletions listed by the repair agent go through apply_deletions like any stage's
  tools="Read,Glob,Grep$(edit_rules "$CTX/**" ".factory/local/**" "$FACTORY_DOCS/**" "${FACTORY_CODE_PATHS[@]}")"
  [ -z "$FACTORY_API_CONTRACT" ] || tools+=$(edit_rules "$FACTORY_API_CONTRACT")
  [ "${#FACTORY_CODE_COMMANDS[@]}" -eq 0 ] || tools+=$(join_rules "${FACTORY_CODE_COMMANDS[@]}")
  tools+=",Bash(bash -n:*),Bash(git diff:*),Bash(git status:*),Bash(git log:*),Bash(git show:*),Bash(date -u:*)"
  prompt="$(cat "$FACTORY_BIN/../prompts/repair.md")
$( [ ! -s "$FACTORY_HOME/local/prompts/repair.md" ] || printf '\n## Project rules for repairs\n\n%s\n' "$(cat "$FACTORY_HOME/local/prompts/repair.md")")

## Run context
- Issue: #$ISSUE, stopped with \`$REASON\` at stage/round \`$PROGRESS\`
- Context folder: $CTX (issue.md, branch.md, run.log, agent-logs/)
- Work branch of the issue: ai-dev/issue-$ISSUE (read it with \`git show origin/ai-dev/issue-$ISSUE:<path>\`)
- Base branch, checked out here: $FACTORY_BASE_BRANCH
- Factory core (read-only for you): .factory/core/ (workflow reference: .factory/core/docs/WORKFLOW.md)
- Stack: $FACTORY_STACK; tests: $FACTORY_TEST_HINT
- Write your summary to $CTX/summary.md
- Attempt $1 of $ATTEMPTS
$2"
  log "repair agent, attempt $1"
  env -u GH_TOKEN -u GITHUB_TOKEN -u FACTORY_GH_TOKEN CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1 \
  claude -p "$prompt" \
    --model "$MODEL" --effort "$EFFORT" --max-turns "$MAX_TURNS" \
    --allowedTools "$tools" \
    --output-format json >"$LOG_DIR/repair.json" 2>"$LOG_DIR/repair.err" \
    || { agent_log repair; agent_failed repair; }
  agent_log repair
  apply_deletions "$tools"
}

gate_log() { if [ -n "${FACTORY_LOG_NOISE:-}" ]; then grep -vE "$FACTORY_LOG_NOISE" || true; else cat; fi; }

changed() { git status --porcelain --untracked-files=all | cut -c4- | sed 's/.* -> //' | grep -v "^$CTX/" || true; }

# check_fix: prints why the fix cannot be merged, and fails; succeeds when it passes everything.
check_fix() {
  local files bad out
  files=$(changed)
  [ -n "$files" ] || { echo "the agent changed no file"; return 1; }
  bad=$(forbidden <<<"$files")
  [ -z "$bad" ] || { echo "a repair may not change these files: $bad"; return 1; }
  for f in $(grep -E '\.sh$' <<<"$files" || true); do [ ! -f "$f" ] || bash -n "$f" || { echo "$f has a syntax error"; return 1; }; done
  if ! out=$(run_all_tests 2>&1); then
    printf 'the whole suite fails with the fix:\n%s\n' "$(gate_log <<<"$out" | tail -60)"
    return 1
  fi
}

summary() { if [ -s "$CTX/summary.md" ]; then cat "$CTX/summary.md"; else tail -40 "$LOG_DIR/repair.log"; fi; }

# ---- repair ----

git fetch -q origin "$FACTORY_BASE_BRANCH"
BRANCH="factory-repair/issue-$ISSUE-$(date -u +%Y%m%d-%H%M)"
git switch -q -C "$BRANCH" "origin/$FACTORY_BASE_BRANCH"
gather_context

feedback="" fixed=""
for attempt in $(seq 1 "$ATTEMPTS"); do
  run_repair_agent "$attempt" "$feedback"
  if grep -qiE '^Result:[[:space:]]*cannot-fix' "$CTX/summary.md" 2>/dev/null; then
    break
  fi
  if why=$(check_fix); then fixed=1; break; fi
  log "attempt $attempt: $(tail -1 <<<"$why")"
  feedback="- The previous attempt cannot be merged; keep what is right and fix this:
\`\`\`
$why
\`\`\`"
done

if [ -z "$fixed" ]; then
  give_up "$( [ -n "${why:-}" ] && printf '%s\n\n' "Last check: ${why%%$'\n'*}")$(summary)"
  exit 0
fi

SUMMARY=$(summary)
rm -rf "$CTX"
git add -A
git commit -q -m "fix(factory): repair the stop of #$ISSUE ($REASON at $PROGRESS)" -m "$SUMMARY" -m "Refs: #$ISSUE"
git push -q -u origin "$BRANCH"
PR_URL=$(gh pr create --base "$FACTORY_BASE_BRANCH" --head "$BRANCH" --title "fix(factory): repair the stop of #$ISSUE ($REASON at $PROGRESS)" \
  --body "$SUMMARY

The repair passed the whole suite. Merged by the factory's repair (\`.factory/core/bin/repair.sh\`).

Refs: #$ISSUE

🤖 Generated with [Claude Code](https://claude.com/claude-code)")
PR=${PR_URL##*/}
out=$(gh pr merge "$PR" --merge --delete-branch --match-head-commit "$(git rev-parse HEAD)" 2>&1) ||
  die "the repair PR #$PR passed the suite but could not be merged: $out"
log "repair PR #$PR merged"

meta=$(jq -cn --arg p "$PROGRESS" '{action: "relaunch", reason: "repaired", progress: $p, counted: false, attempt: 0}')
# Without the factory marker: this comment starts the factory workflow, which resumes at the stage where it stopped.
printf 'The supervisor repaired the cause of the stop in #%s and resumes the factory.\n\n%s\n' "$PR" "$SUMMARY" | supervisor_comment "$meta" 0
