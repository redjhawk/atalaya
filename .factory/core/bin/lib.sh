# Shared helpers of the agent factory. Sourced by factory.sh, repair.sh and doctor.sh; not executable alone.
# shellcheck shell=bash

log() { printf '\n[factory %s] %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
# FAIL_DETAIL is read by the EXIT trap of factory.sh, which reports the stop on the issue.
die() { log "ERROR: $*"; FAIL_DETAIL=$*; exit 1; }

# ---- configuration ----
#
# Layout in the target repository:
#   .factory/core/      managed by the installer (never edit: updates replace it; local edits are kept and reported)
#   .factory/config.sh  this project's settings and commands (yours; updates never touch it)
#   .factory/local/     this project's extensions (yours): prompts/<stage>.md, gates.sh, hooks.sh, out-of-scope.md
#
# FACTORY_HOME is the .factory folder the running script belongs to. The workflows run a copy taken from the
# base branch, so a change cannot rewrite its own pipeline, its configuration or its extensions.

load_config() {
  FACTORY_HOME=${FACTORY_HOME:-$(cd -- "$FACTORY_BIN/../.." && pwd)}
  [ -f "$FACTORY_HOME/config.sh" ] || die "missing $FACTORY_HOME/config.sh (run the installer, then edit it)"
  # shellcheck source=/dev/null
  source "$FACTORY_HOME/config.sh"

  FACTORY_BASE_BRANCH=${FACTORY_BASE_BRANCH:-main}
  FACTORY_DOCS=${FACTORY_DOCS:-docs}
  FACTORY_API_CONTRACT=${FACTORY_API_CONTRACT:-}
  FACTORY_LABEL=${FACTORY_LABEL:-ai-dev}
  FACTORY_STACK=${FACTORY_STACK:-"(not described: read the README and the build files)"}
  FACTORY_TEST_HINT=${FACTORY_TEST_HINT:-"(see project_tests in .factory/config.sh)"}
  FACTORY_TEST_FILE_PATTERN=${FACTORY_TEST_FILE_PATTERN:-'(_test\.go|\.(test|spec)\.[cm]?[jt]sx?|_test\.py|test_[^/]*\.py)$'}
  FACTORY_SKIP_STAGES=${FACTORY_SKIP_STAGES:-}
  FACTORY_SKILL_DIRS=${FACTORY_SKILL_DIRS:-".agents/skills .claude/skills project-skills skills"}
  FACTORY_SENSITIVE_PATTERN=${FACTORY_SENSITIVE_PATTERN:-'^$'}
  FACTORY_DEPENDENCY_PATTERN=${FACTORY_DEPENDENCY_PATTERN:-'(^|/)(package(-lock)?\.json|pnpm-lock\.yaml|yarn\.lock|go\.(mod|sum)|Cargo\.(toml|lock)|requirements[^/]*\.txt|poetry\.lock|pyproject\.toml|Gemfile(\.lock)?|composer\.(json|lock))$'}
  FACTORY_ONE_WAY_PATHS=${FACTORY_ONE_WAY_PATHS:-'(^|/)migrations?/'}
  FACTORY_ONE_WAY_PATTERN=${FACTORY_ONE_WAY_PATTERN:-'\b(CREATE|ALTER|DROP)[[:space:]]+(TABLE|INDEX|COLUMN|SCHEMA|DATABASE)\b'}
  FACTORY_VERIFY_URL=${FACTORY_VERIFY_URL:-}
  FACTORY_SKILLS_DIR=${FACTORY_SKILLS_DIR:-.agents/skills} # where the installer put the factory-* skills
  declare -p FACTORY_CODE_PATHS >/dev/null 2>&1 || FACTORY_CODE_PATHS=("src/**" "tests/**")
  declare -p FACTORY_CODE_COMMANDS >/dev/null 2>&1 || FACTORY_CODE_COMMANDS=()
  declare -p FACTORY_VERIFY_COMMANDS >/dev/null 2>&1 || FACTORY_VERIFY_COMMANDS=("Bash(curl:*)")
  declare -p FACTORY_LOCKED_EXTRA >/dev/null 2>&1 || FACTORY_LOCKED_EXTRA=()

  declare -F project_tests >/dev/null || die "$FACTORY_HOME/config.sh must define project_tests (the whole suite)"
  declare -F project_setup >/dev/null || project_setup() { :; }
  declare -F project_security_scan >/dev/null || project_security_scan() { :; }
  declare -F project_red_checks >/dev/null || project_red_checks() { :; }

  SPEC_DIR="$FACTORY_DOCS/specifications"
  USECASE_DIR="$FACTORY_DOCS/use-cases"
  CHANGES_DIR="$FACTORY_DOCS/changes"
  GLOSSARY="$FACTORY_DOCS/GLOSSARY.md"
  ADR_DIR="$FACTORY_DOCS/adr"

  # Extensions: optional, never required.
  # shellcheck source=/dev/null
  [ ! -f "$FACTORY_HOME/local/gates.sh" ] || source "$FACTORY_HOME/local/gates.sh"
  # shellcheck source=/dev/null
  [ ! -f "$FACTORY_HOME/local/hooks.sh" ] || source "$FACTORY_HOME/local/hooks.sh"
}

verify_enabled() { declare -F project_start >/dev/null && ! stage_skipped verify; }
stage_skipped() { [[ " $FACTORY_SKIP_STAGES " == *" $1 "* ]]; }

# Files the factory never merges on its own. The core list protects the pipeline, the agents' instructions
# (a prompt injection that persists) and CI; the project adds its own (FACTORY_SENSITIVE_PATTERN).
CORE_SENSITIVE='^(\.github/|\.gitlab-ci\.yml$|\.factory/|\.claude/|\.agents/|\.cursor/|\.codex/|AGENTS\.md$|CLAUDE\.md$|GEMINI\.md$|CODEOWNERS$|\.gitignore$|\.gitattributes$)'

skill_dir_pattern() { # the configured skill folders, as an anchored regex
  local d out=""
  for d in ${FACTORY_SKILL_DIRS:-.agents/skills .claude/skills}; do out+="${out:+|}$(sed 's/[.[\*^$()+?{|]/\\&/g' <<<"${d%/}")/"; done
  if [ -n "$out" ]; then printf '^(%s)' "$out"; else printf '^/never/$'; fi
}

# ---- project skills ----
#
# The factory adapts to what the repository already has: every skill found in FACTORY_SKILL_DIRS is listed in
# each agent's prompt with its description, and its rules apply. The factory's own skills (factory-*) are
# listed apart. Nothing here edits or removes a skill.
project_skills() {
  local d f name desc
  for d in $FACTORY_SKILL_DIRS; do
    [ -d "$d" ] || continue
    find "$d" -mindepth 2 -maxdepth 3 -name SKILL.md -not -path '*/node_modules/*' 2>/dev/null
  done | sort -u | while IFS= read -r f; do
    name=$(basename "$(dirname "$f")")
    case $name in factory-*) continue ;; esac
    desc=$(awk 'NR==1 && $0!="---" {exit} NR>1 && $0=="---" {exit} /^description:/ {sub(/^description:[[:space:]]*/, ""); gsub(/^["'\'']|["'\'']$/, ""); print; exit}' "$f")
    printf -- '- `%s`: %s\n' "$f" "${desc:-(no description)}"
  done | awk '!seen[$0]++'
}

# ---- state (<change folder>/state.json on the work branch) ----

state_get() { jq -r --arg k "$1" '.[$k] // empty' "$STATE"; }

state_set() { # state_set key value  (value stored as string, or as JSON when it parses)
  local tmp; tmp=$(mktemp)
  if jq -e . >/dev/null 2>&1 <<<"$2"; then
    jq --arg k "$1" --argjson v "$2" '.[$k] = $v' "$STATE" >"$tmp"
  else
    jq --arg k "$1" --arg v "$2" '.[$k] = $v' "$STATE" >"$tmp"
  fi
  mv "$tmp" "$STATE"
}

# ---- git ----

checkpoint() { # commit everything the stage produced and push it, so a later run can resume here
  git add -A
  if ! git diff --cached --quiet; then
    git commit -q -m "$1" -m "Refs: #$ISSUE"
  fi
  if [ "${FACTORY_PUSH:-1}" = 1 ]; then git push -q -u origin HEAD; fi
}

# Paths changed by this change: committed on the branch, modified in the working tree and untracked.
# Gates run before the stage checkpoint, so uncommitted work must count. Includes deleted paths.
changed_paths() {
  { git diff --name-only "origin/$FACTORY_BASE_BRANCH...HEAD" -- "$@"
    git diff --name-only HEAD -- "$@"
    git ls-files -o --exclude-standard -- "$@"; } | sort -u
}

changed_files() { # changed_paths that still exist
  local f; changed_paths "$@" | while IFS= read -r f; do [ -e "$f" ] && printf '%s\n' "$f"; done; return 0
}

# ---- GitHub ----

# Every factory comment starts with this marker; the factory workflow ignores comments containing it, so the
# factory's own comments never restart it.
FACTORY_MARKER='<!-- agent-factory -->'
issue_comment() { # issue_comment [marker line] < body
  { printf '%s\n' "$FACTORY_MARKER"; [ -z "${1:-}" ] || printf '%s\n' "$1"; cat; } | gh issue comment "$ISSUE" --body-file - >/dev/null
}

# Every stop is reported on the issue with a machine-readable line that supervisor.mjs reads:
#   <!-- factory-stop {"reason":"usage-limit","stage":"implement","round":1,"resets_at":"2026-11-20T23:10:00Z"} -->
stop_marker() { # stop_marker <reason> [extra JSON object]
  local extra=${2:-}
  [ -n "$extra" ] || extra='{}'
  jq -rcn --arg reason "$1" --arg stage "$(state_get stage)" --arg round "$(state_get round)" --argjson extra "$extra" \
    '"<!-- factory-stop " + ({reason: $reason, stage: $stage, round: ($round | tonumber? // 0)} + $extra | tojson) + " -->"'
}

# limit_resets_at <usage-limit message>: the reset time as UTC ISO 8601, or nothing when it cannot be read.
# "You've hit your session limit · resets 11:10pm (UTC)" names a time of day: today's, or tomorrow's once past.
limit_resets_at() {
  local re='resets ([^()]+) \(([^()]+)\)' spec tz when
  [[ $1 =~ $re ]] || return 0
  spec=${BASH_REMATCH[1]//,/} tz=${BASH_REMATCH[2]}
  when=$(TZ=$tz date -d "$spec" +%s 2>/dev/null) || return 0
  [ "$when" -gt "$(date +%s)" ] || when=$(TZ=$tz date -d "tomorrow $spec" +%s 2>/dev/null) || return 0
  date -u -d "@$when" +%Y-%m-%dT%H:%M:%SZ
}

set_stage_label() { # mirrors the stage on the issue, for humans; failures never stop the factory
  local label="factory:$1" old
  gh label create "$label" --color BFD4F2 --force >/dev/null 2>&1 || true
  for old in $(gh issue view "$ISSUE" --json labels -q '.labels[].name | select(startswith("factory:"))'); do
    [ "$old" = "$label" ] || gh issue edit "$ISSUE" --remove-label "$old" >/dev/null 2>&1 || true
  done
  gh issue edit "$ISSUE" --add-label "$label" >/dev/null 2>&1 || true
}

# Writes the issue and the comments of trusted authors to the change folder. Only trusted authors give
# instructions: the body of an issue opened by anyone else is dropped (the owner must restate it in a comment).
# The factory's and the supervisor's own comments are not instructions either.
refresh_issue_context() {
  gh issue view "$ISSUE" --json number,title,body,author,comments | jq -r --arg trusted "$TRUSTED_ACTORS" '
    ($trusted | split(",")) as $t |
    "# Issue #\(.number) \(.title)\n\n" +
    (if (.author.login as $a | $t | index($a)) then .body
     else "_Issue opened by \(.author.login), who is not a trusted author: its body is not shown and gives no instructions._" end) +
    "\n\n## Comments by trusted authors\n" +
    ([.comments[] | select(.author.login as $a | $t | index($a)) |
      select(.body | (contains("<!-- agent-factory -->") or contains("<!-- factory-supervisor ")) | not) |
      "\n### \(.author.login) at \(.createdAt)\n\n\(.body)\n"] | join(""))' >"$CHANGE_DIR/issue.md"
}

# ---- agents ----

edit_rules() { local p; for p in "$@"; do printf ',Edit(%s)' "$p"; done; }
join_rules() { local IFS=,; printf ',%s' "$*"; }

# tools_for <profile>: the --allowedTools list of a profile. Edit rules (which also cover Write) are limited to
# the paths each stage produces; Bash to fixed commands. No profile can delete a file (see apply_deletions).
tools_for() {
  local contract="" t
  [ -z "$FACTORY_API_CONTRACT" ] || contract=$(edit_rules "$FACTORY_API_CONTRACT")
  case "$1" in
    # spec is the single writer of the glossary and the decision records; the other stages read them.
    spec)   t="Read,Glob,Grep$(edit_rules "$FACTORY_DOCS/**")$contract" ;;
    docs)   t="Read,Glob,Grep$(edit_rules "$FACTORY_DOCS/**")$contract" ;;
    code)   t="Read,Glob,Grep$(edit_rules "$SPEC_DIR/**" "${FACTORY_CODE_PATHS[@]}" "$CHANGE_DIR/**")"
            [ "${#FACTORY_CODE_COMMANDS[@]}" -eq 0 ] || t+=$(join_rules "${FACTORY_CODE_COMMANDS[@]}")
            t+=",Bash(git diff:*),Bash(git status:*),Bash(git log:*),Bash(git show:*),Bash(date -u:*)" ;;
    review) t="Read,Glob,Grep$(edit_rules "$CHANGE_DIR/**"),Bash(git diff:*),Bash(git log:*),Bash(git show:*)" ;;
    verify) t="Read,Glob,Grep$(edit_rules "$CHANGE_DIR/**")$(join_rules "${FACTORY_VERIFY_COMMANDS[@]}")" ;;
    # Exactly the files of a failed merge (MERGE_FILES, set by resolve_conflicts in factory.sh).
    # shellcheck disable=SC2086
    merge)  t="Read,Glob,Grep$(edit_rules $MERGE_FILES),Bash(git diff:*),Bash(git log:*),Bash(git show:*),Bash(git status:*)" ;;
    *) die "unknown tool profile $1" ;;
  esac
  printf '%s' "$t"
}

# Deny rules win over allow rules: only the spec stage may write the glossary and the decision records.
denied_for() {
  [ "$1" = spec ] && return 0
  printf 'Edit(%s),Edit(%s/**)' "$GLOSSARY" "$ADR_DIR"
}

# stage_prompt <name>: the core prompt, then this project's additions for the same stage. A local addition
# extends the core prompt; it never replaces it, so a core update still reaches the project.
stage_prompt() {
  local name=$1 f
  cat "$FACTORY_BIN/../prompts/common.md"
  f="$FACTORY_HOME/local/prompts/common.md"
  [ ! -s "$f" ] || printf '\n## Project rules for every stage\n\n%s\n' "$(cat "$f")"
  printf '\n# Your stage\n\n'
  cat "$FACTORY_BIN/../prompts/$name.md"
  f="$FACTORY_HOME/local/prompts/$name.md"
  [ ! -s "$f" ] || printf '\n## Project rules for this stage\n\n%s\n' "$(cat "$f")"
}

run_context() {
  cat <<EOF

## Run context
- Issue: #$ISSUE
- Change folder: $CHANGE_DIR (issue text and trusted comments in $CHANGE_DIR/issue.md)
- Base branch: $FACTORY_BASE_BRANCH
- Stack: $FACTORY_STACK
- Test and build commands: $FACTORY_TEST_HINT
- Documentation root: $FACTORY_DOCS (specifications in $SPEC_DIR, use cases in $USECASE_DIR, glossary $GLOSSARY, decisions $ADR_DIR)
- API contract: ${FACTORY_API_CONTRACT:-none}
- Paths the code stages may edit: ${FACTORY_CODE_PATHS[*]}

## Project skills (their rules apply to your work)
$(project_skills)

## Factory skills
$(for f in "$FACTORY_SKILLS_DIR"/factory-*/SKILL.md; do [ -f "$f" ] && printf -- '- `%s`\n' "$f"; done)
EOF
}

# run_agent <prompt name> <tool profile> [extra context]
# Every stage is a fresh, independent Claude process: no shared context between roles.
run_agent() {
  local name=$1 profile=$2 extra=${3:-} tools denied prompt
  tools=$(tools_for "$profile")
  denied=$(denied_for "$profile")
  prompt="$(stage_prompt "$name")
$(run_context)
$extra"
  log "agent $name ($profile)"
  # The agent never sees the GitHub token, and Claude Code scrubs its own credentials from the environment
  # of the commands it runs (CLAUDE_CODE_SUBPROCESS_ENV_SCRUB). The scrub forces the default permission mode,
  # so the agent only gets what --allowedTools grants.
  env -u GH_TOKEN -u GITHUB_TOKEN -u FACTORY_GH_TOKEN CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1 \
  claude -p "$prompt" \
    --model "$MODEL" --effort "$EFFORT" --max-turns "$MAX_TURNS" \
    --allowedTools "$tools" ${denied:+--disallowedTools "$denied"} \
    --output-format json >"$LOG_DIR/$name.json" 2>"$LOG_DIR/$name.err" \
    || { agent_log "$name"; agent_failed "$name"; }
  agent_log "$name"
  apply_deletions "$tools"
  [ "$profile" = spec ] || restore_single_writer
}

# The glossary and the decision records have one writer, the spec stage: whatever another stage left in them
# is put back as it was at the stage start (the last checkpoint). The deny rules already refuse the edit;
# this also covers a file written through another tool.
restore_single_writer() {
  local p
  for p in $(git diff --name-only HEAD -- "$GLOSSARY" "$ADR_DIR"; git ls-files -o --exclude-standard -- "$GLOSSARY" "$ADR_DIR"); do
    if git cat-file -e "HEAD:$p" 2>/dev/null; then git checkout -q HEAD -- "$p"; else rm -f -- "$p"; fi
    log "restored $p: only the spec stage writes the glossary and the decision records"
  done
}

# apply_deletions <tools>: no tool profile can delete a file (rm and git rm need an approval nobody gives in a
# non-interactive run), so an agent lists the files to delete in $CHANGE_DIR/delete.txt, one path per line.
# The factory deletes those its profile's Edit rules cover and keeps the others; the list is never committed.
# Sensitive paths (skills, agent instructions, the factory, CI) are never deleted, whatever the rules say.
apply_deletions() {
  local list="$CHANGE_DIR/delete.txt" p rule ok rules
  [ -f "$list" ] || return 0
  IFS=, read -ra rules <<<"$1"
  while IFS= read -r p || [ -n "$p" ]; do
    p=${p%$'\r'}; p=${p#"$PWD"/}; p=${p#./}
    [ -n "$p" ] || continue
    ok=0
    if [[ $p != /* && /$p/ != */../* ]] && { [ -f "$p" ] || [ -L "$p" ]; } && ! is_sensitive "$p"; then
      for rule in "${rules[@]}"; do
        case "$rule" in Edit\(*\)) rule=${rule#Edit(}; rule=${rule%)} ;; *) continue ;; esac
        # shellcheck disable=SC2053 # the rule is a glob; in [[ ]], * also matches /
        [[ $p == $rule ]] && ok=1
      done
    fi
    if [ $ok = 1 ]; then
      git rm -q -f --ignore-unmatch -- "$p"; rm -f -- "$p"
      log "deleted $p, listed in delete.txt"
    else
      log "kept $p, listed in delete.txt: not a file the stage may change"
    fi
  done <"$list"
  rm -f -- "$list"
}

# The project's exceptions (FACTORY_SENSITIVE_EXCEPT) can narrow its own patterns, never the core list or the skills.
is_sensitive() {
  grep -qE "$CORE_SENSITIVE|$(skill_dir_pattern)" <<<"$1" && return 0
  [ -z "${FACTORY_SENSITIVE_EXCEPT:-}" ] || ! grep -qE "$FACTORY_SENSITIVE_EXCEPT" <<<"$1" || return 1
  grep -qE "${FACTORY_SENSITIVE_PATTERN:-^/never/$}|${FACTORY_DEPENDENCY_PATTERN:-^/never/$}" <<<"$1"
}

# agent_log <prompt name>: $LOG_DIR/<name>.log holds the agent's final answer, followed by every tool call
# the permissions refused (tool and input), so a stage that could not write its file shows why.
agent_log() {
  local name=$1
  if jq -e 'type == "object" and has("result")' "$LOG_DIR/$name.json" >/dev/null 2>&1; then
    jq -r '.result + (if (.permission_denials // []) == [] then "" else
      "\n\n## Permission denials\n" + ([.permission_denials[] |
        "- \(.tool_name): \(.tool_input | tostring | .[0:300])"] | join("\n")) end)' "$LOG_DIR/$name.json" >"$LOG_DIR/$name.log"
  else
    cat "$LOG_DIR/$name.json" >"$LOG_DIR/$name.log"
  fi
  cat "$LOG_DIR/$name.err" >>"$LOG_DIR/$name.log" 2>/dev/null || true
  if grep -q '^## Permission denials' "$LOG_DIR/$name.log"; then
    log "agent $name: tool calls refused by its permissions:"; sed -n '/^## Permission denials/,$p' "$LOG_DIR/$name.log" | tail -n +2 >&2
  fi
}

# json_from_log <prompt name> <file>: when an agent could not write its JSON file but printed it, take the
# last ```json block of its answer. Leaves the file alone when it already holds valid JSON.
json_from_log() {
  local name=$1 file=$2 block
  ! jq -e . "$file" >/dev/null 2>&1 || return 0
  block=$(awk '/^```json[[:space:]]*$/ {b=""; on=1; next} on && /^```[[:space:]]*$/ {last=b; on=0; next} on {b=b $0 "\n"} END {printf "%s", last}' "$LOG_DIR/$name.log")
  [ -n "$block" ] && jq -e . >/dev/null 2>&1 <<<"$block" || return 0
  printf '%s\n' "$block" >"$file"
  log "agent $name did not write $file; taken from the JSON block of its answer"
}

# agent_failed <prompt name>: report a failed agent, telling a usage limit apart from an agent error.
agent_failed() {
  local name=$1 limit
  tail -40 "$LOG_DIR/$name.log" >&2
  limit="$(grep -m1 -iE "hit your (session|usage|weekly) limit" "$LOG_DIR/$name.log" || true)"
  if [ -n "$limit" ]; then
    FAIL_REASON=usage-limit
    FAIL_EXTRA=$(jq -cn --arg r "$(limit_resets_at "$limit")" 'if $r == "" then {} else {resets_at: $r} end')
    die "agent $name stopped by the Claude usage limit (${limit}); the factory resumes at the current stage after the reset"
  fi
  die "agent $name failed (log: $LOG_DIR/$name.log)"
}

# run_hook <before|after> <stage>: optional functions from .factory/local/hooks.sh.
run_hook() { ! declare -F "hook_$1_$2" >/dev/null || "hook_$1_$2"; }
