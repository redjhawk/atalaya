# Deterministic gates between stages. An agent saying "done" is not enough: these checks decide.
# Each gate prints why it failed and returns non-zero. Sourced by factory.sh after lib.sh and load_config.
# A project adds checks to any gate with a function local_<gate> in .factory/local/gates.sh: it runs after
# the core gate passes, and can only make the gate stricter.
# shellcheck shell=bash

functional_files() { changed_files "$SPEC_DIR/*/functional.md"; }

# A line defines a requirement when an FR ID is its first token: a table row "| FR-X-001 |", a list
# item "- FR-X-001" / "- **FR-X-001**", or a heading "### FR-X-001". IDs cited elsewhere in a line
# (cross-references such as "see FR-OTHER-013") are not definitions.
FR_DEFINITION='^[[:space:]]*(\|[[:space:]]*|[-*][[:space:]]+(\*\*)?|#+[[:space:]]+)FR-[A-Z0-9-]+-[0-9]{3}'

requirement_ids() { # FR IDs this change defines or modifies: definition lines added since the base branch
  local base untracked
  base=$(git merge-base "origin/$FACTORY_BASE_BRANCH" HEAD)
  untracked=$(git ls-files -o --exclude-standard -- "$SPEC_DIR/*/functional.md")
  { git diff -U0 "$base" -- "$SPEC_DIR/*/functional.md" | sed -n 's/^+//p'
    # shellcheck disable=SC2086
    [ -z "$untracked" ] || cat $untracked
  } | grep -oE "$FR_DEFINITION" | grep -oE 'FR-[A-Z0-9-]+-[0-9]{3}$' | sort -u
}

test_files() { changed_files | grep -E "$FACTORY_TEST_FILE_PATTERN" || true; }

# Paths this change modifies outside documentation and tests, i.e. its implementation (and test stubs).
production_paths() {
  changed_paths | grep -Ev "^$FACTORY_DOCS/" | { if [ -n "$FACTORY_API_CONTRACT" ]; then grep -vxF "$FACTORY_API_CONTRACT"; else cat; fi; } |
    grep -Ev "$FACTORY_TEST_FILE_PATTERN" || true
}

# Tests run code written by agents: never with the GitHub or Claude credentials in the environment.
run_all_tests() {
  ( unset -v GH_TOKEN GITHUB_TOKEN FACTORY_GH_TOKEN CLAUDE_CODE_OAUTH_TOKEN ANTHROPIC_API_KEY DEV_TOKEN
    project_tests )
}

# Files the gates execute (test configuration): always locked with the tests, even when unchanged.
lock_files() {
  { test_files
    local g; for g in "${FACTORY_LOCKED_EXTRA[@]}"; do compgen -G "$g" || true; done
  } | sort -u
}

# After the factory merges the base branch (sync_base in factory.sh): the same locked files, plus the
# always-locked ones the merge may have added, hashed again with their merged content.
relock_tests() {
  local lock="$CHANGE_DIR/tests.lock" f
  { awk '{print $2}' "$lock"; lock_files | grep -vE "$FACTORY_TEST_FILE_PATTERN" || true; } | sort -u |
    while IFS= read -r f; do [ ! -e "$f" ] || printf '%s\n' "$f"; done | xargs -r sha256sum >"$lock.new"
  mv "$lock.new" "$lock"
}

# The commit the test files are compared with: the TDD red commit, or a later merge of the base branch that
# the factory made before an implementation round. Agents cannot commit, so only the factory (or a human)
# creates commits with these subjects.
test_ref() {
  git log -1 --format=%H --grep="^test: failing tests for #$ISSUE (TDD red)" \
    --grep="^chore(factory): merge $FACTORY_BASE_BRANCH into #$ISSUE (tests relocked)" HEAD
}

# run_gate <gate>: the core gate, then the project's stricter local_<gate> when it exists.
run_gate() {
  "$1" || return 1
  ! declare -F "local_$1" >/dev/null || "local_$1"
}

gate_spec() {
  [ -n "$(functional_files)" ] || { echo "no $SPEC_DIR/<subject>/functional.md changed"; return 1; }
  [ -n "$(requirement_ids)" ] || { echo "functional specs define no FR-<SUBJECT>-NNN requirement"; return 1; }
  if verify_enabled && [ ! -s "$CHANGE_DIR/feature-path.md" ] && [ ! -s "$CHANGE_DIR/no-feature-path.md" ]; then
    echo "$CHANGE_DIR/feature-path.md missing (or no-feature-path.md with the reason): the verify stage drives it"; return 1
  fi
}

gate_usecases() {
  [ -f "$CHANGE_DIR/no-use-cases.md" ] && return 0
  local files f
  files=$(changed_files "$USECASE_DIR/" | grep -v README.md || true)
  [ -n "$files" ] || { echo "no use case written under $USECASE_DIR/ (or $CHANGE_DIR/no-use-cases.md with the reason)"; return 1; }
  for f in $files; do grep -qE 'FR-[A-Z0-9-]+-[0-9]{3}' "$f" || { echo "$f does not reference any FR ID"; return 1; }; done
}

gate_testcases() {
  local cases="$CHANGE_DIR/testcases.md" id missing=0
  [ -f "$cases" ] || { echo "$cases missing"; return 1; }
  grep -qE 'TC-[0-9]{3}' "$cases" || { echo "no TC-NNN test case in $cases"; return 1; }
  for id in $(requirement_ids); do
    grep -q "$id" "$cases" || { echo "requirement $id has no test case"; missing=1; }
  done
  return $missing
}

gate_techspec() {
  local f missing=0
  for f in $(functional_files); do
    [ -f "$(dirname "$f")/technical.md" ] || { echo "$(dirname "$f")/technical.md missing"; missing=1; }
  done
  return $missing
}

# TDD red: every test case is automated and the suite fails before the implementation exists.
gate_red() {
  if [ -f "$CHANGE_DIR/no-tests.md" ]; then echo "no-tests.md present: red phase skipped"; return 0; fi
  local files id missing=0
  files=$(test_files)
  [ -n "$files" ] || { echo "no test file changed"; return 1; }
  for id in $(grep -oE 'TC-[0-9]{3}' "$CHANGE_DIR/testcases.md" | sort -u); do
    # shellcheck disable=SC2086
    grep -q "$id" $files || { echo "test case $id is not referenced by any changed test"; missing=1; }
  done
  [ $missing = 0 ] || return 1
  # Project checks on the new tests before they are locked (formatting, the test runner lists them...).
  project_red_checks "$files" || return 1
  if run_all_tests; then echo "the tests pass before implementing: they do not test the new behaviour"; return 1; fi
  lock_files | xargs -r sha256sum >"$CHANGE_DIR/tests.lock"
  echo "red confirmed; tests locked in tests.lock"
}

# TDD green: the locked tests are untouched and the whole suite passes.
# The lock is read from the test reference commit (test_ref), not from the working tree the agent can edit.
gate_green() {
  local commit lock
  commit=$(test_ref)
  [ -n "$commit" ] || { echo "no TDD red commit found for #$ISSUE"; return 1; }
  if lock=$(git show "$commit:$CHANGE_DIR/tests.lock" 2>/dev/null) && [ -n "$lock" ]; then
    sha256sum --quiet -c - <<<"$lock" || { echo "locked tests were modified by the implementation"; return 1; }
    cmp -s <(printf '%s\n' "$lock") "$CHANGE_DIR/tests.lock" || { echo "tests.lock was modified by the implementation"; return 1; }
  elif ! git cat-file -e "$commit:$CHANGE_DIR/no-tests.md" 2>/dev/null; then
    echo "tests.lock missing or empty in the TDD red commit $commit"; return 1
  fi
  # Any test file (new or pre-existing) changed after the reference commit fails: tests cannot be weakened.
  local touched
  touched=$({ git diff --name-only "$commit" --; git ls-files -o --exclude-standard; } | grep -E "$FACTORY_TEST_FILE_PATTERN" | sort -u || true)
  [ -z "$touched" ] || { echo "test files changed after the TDD red commit:"; echo "$touched"; return 1; }
  run_all_tests
}

# Verify: the agent's report is valid and holds no blocking finding.
gate_verify() {
  local report="$CHANGE_DIR/verify.json"
  jq -e 'type == "object" and (.steps | type == "array") and (.findings | type == "array") and
    all(.findings[]; (.blocking | type == "boolean") and (.body | type == "string"))' "$report" >/dev/null 2>&1 ||
    { echo "$report is not a valid verify report"; return 1; }
  if jq -e 'any(.steps[]; .status != "pass") or any(.findings[]; .blocking)' "$report" >/dev/null; then
    echo "the feature path failed or raised a blocking finding"; return 1
  fi
}

sensitive_files() { changed_paths | while IFS= read -r p; do ! is_sensitive "$p" || printf '%s\n' "$p"; done; }

# One-way doors: changes that are expensive or impossible to undo after the merge (schema migrations, data
# deletion). The factory never merges them on its own; a human looks first.
one_way_changes() {
  local base
  base=$(git merge-base "origin/$FACTORY_BASE_BRANCH" HEAD)
  changed_paths | grep -E "$FACTORY_ONE_WAY_PATHS" | sed 's/$/ (path)/' || true
  git diff -U0 "$base" -- . ":(exclude)$FACTORY_DOCS" | awk '/^\+\+\+ b\// {f=substr($0,7)} /^\+[^+]/ {print f "\t" substr($0,2)}' |
    grep -iE "$FACTORY_ONE_WAY_PATTERN" | cut -f1 | sort -u | grep -vE "$FACTORY_TEST_FILE_PATTERN" | sed 's/$/ (content)/' || true
}

# ---- reviews ----

# merge_reviews <round> <out> <axis file>...: one array with ids R<round>-NN and the axis of each finding.
merge_reviews() {
  local round=$1 out=$2; shift 2
  jq -s --arg r "$round" '[.[] | select(type == "array") | .[]] | to_entries |
    map(.value + {id: ("R" + $r + "-" + ((.key + 1) | tostring | if length < 2 then "0" + . else . end))})' "$@" >"$out"
}

# Fails closed: exactly one decision per review id, boolean blocking, safe ids, a follow-up only when it is
# worth one: an issue needs the concrete consequence of leaving it, a note needs its text.
validate_triage() { # validate_triage <review> <triage>
  jq -e --slurpfile r "$1" '
    type == "array" and
    ([.[].id] | sort) == ([$r[0][].id] | sort) and
    ([.[].id] | length) == ([.[].id] | unique | length) and
    all(.[]; (.id | type == "string" and test("^R[0-9]+-[0-9]+$")) and (.blocking | type == "boolean") and
             (.reason | type == "string") and
             (.blocking or
              ((.followup // "") as $f |
               ($f == "none") or
               ($f == "note" and (.todo | type == "string" and length > 0)) or
               ($f == "issue" and (.consequence | type == "string" and length > 0) and
                (.title | type == "string" and length > 0) and (.todo | type == "string" and length > 0)))))' \
    "$2" >/dev/null 2>&1 || { log "triage $2 must hold one valid decision for every id in $1"; return 1; }
}
