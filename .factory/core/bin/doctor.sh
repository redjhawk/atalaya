#!/usr/bin/env bash
# Health check of the agent factory in this repository. Read-only: it reports and suggests, it changes nothing.
#
#   .factory/core/bin/factory doctor
#
# Exit status 1 when an error would stop a factory run, 0 otherwise (warnings included).
set -uo pipefail

FACTORY_BIN="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$FACTORY_BIN/lib.sh"
cd "$(git rev-parse --show-toplevel)" || exit 1

errors=0 warnings=0
ok()   { printf '  ok    %s\n' "$*"; }
warn() { printf '  warn  %s\n' "$*"; warnings=$((warnings + 1)); }
err()  { printf '  ERROR %s\n' "$*"; errors=$((errors + 1)); }
section() { printf '\n%s\n' "$*"; }

section "Configuration"
if ( load_config ) >/dev/null 2>&1; then
  load_config >/dev/null 2>&1
  ok "$FACTORY_HOME/config.sh loads; base branch $FACTORY_BASE_BRANCH, docs in $FACTORY_DOCS/"
else
  err "$(load_config 2>&1 | tail -1)"; printf '\n%s error(s)\n' "$errors"; exit 1
fi
if [ -f "$FACTORY_HOME/config.example.sh" ]; then
  missing=$(comm -23 <(grep -oE 'FACTORY_[A-Z_]+' "$FACTORY_HOME/config.example.sh" | sort -u) <(grep -oE 'FACTORY_[A-Z_]+' "$FACTORY_HOME/config.sh" | sort -u) | tr '\n' ' ')
  [ -z "${missing// }" ] || warn "settings documented in config.example.sh that config.sh leaves at their default: $missing"
fi
for p in "${FACTORY_CODE_PATHS[@]}"; do
  case $p in *\**) q="${p%%\**}x" ;; *) q=$p ;; esac # a glob is checked through a path inside it
  is_sensitive "$q" && err "FACTORY_CODE_PATHS entry $p is a sensitive path: agents would edit what the factory never merges"
done
declare -F project_start >/dev/null && ok "verify stage enabled (project_start defined)" || warn "verify stage disabled: define project_start (and project_ready) to drive the feature path on the running system"
[ -z "$FACTORY_SKIP_STAGES" ] || ok "skipped stages: $FACTORY_SKIP_STAGES"

section "Tools"
for t in $(printf '%s\n' git jq gh claude sha256sum node ${FACTORY_REQUIRED_TOOLS[@]+"${FACTORY_REQUIRED_TOOLS[@]}"} | awk '!seen[$0]++'); do
  command -v "$t" >/dev/null && ok "$t" || err "$t not found"
done
if [ "$(uname -s)" = Linux ]; then
  for t in bwrap socat; do command -v "$t" >/dev/null && ok "$t (Claude Code sandbox)" || err "$t not found: the agents' Bash tool cannot run"; done
fi
command -v gh >/dev/null && { gh auth status >/dev/null 2>&1 && ok "gh is authenticated" || warn "gh is not authenticated (needed to run the factory locally)"; }

section "Repository"
git ls-remote --exit-code --heads origin "$FACTORY_BASE_BRANCH" >/dev/null 2>&1 && ok "origin/$FACTORY_BASE_BRANCH exists" || err "origin/$FACTORY_BASE_BRANCH not found (FACTORY_BASE_BRANCH)"
for f in .github/workflows/factory.yml .github/workflows/factory-supervisor.yml .github/workflows/factory-repair.yml; do
  [ -f "$f" ] && ok "$f" || warn "$f missing: install with --with-github to run the factory from GitHub Actions"
done
grep -qF '.factory-repair/' .gitignore 2>/dev/null && ok ".gitignore ignores the repair context" || warn ".gitignore lacks .factory-repair/ (rerun the installer)"

section "Instructions and skills (the factory reads them; it never edits or removes them)"
grep -q '<!-- factory:begin' AGENTS.md 2>/dev/null && ok "AGENTS.md has the factory block" || warn "AGENTS.md has no factory block (rerun the installer)"
n=$(project_skills | grep -c . || true)
if [ "$n" -gt 0 ]; then ok "$n project skill(s) found, listed in every agent prompt:"; project_skills | sed 's/^/          /'
else warn "no project skill found in: $FACTORY_SKILL_DIRS (FACTORY_SKILL_DIRS)"; fi
for f in "$FACTORY_SKILLS_DIR"/factory-*/SKILL.md; do [ -f "$f" ] && ok "factory skill $f"; done
for d in "$FACTORY_HOME/local/prompts"; do
  [ -d "$d" ] && for f in "$d"/*.md; do [ -f "$f" ] || continue
    s=$(basename "$f" .md); [ -f "$FACTORY_BIN/../prompts/$s.md" ] && ok "local addition to prompt $s" || warn "$f extends no core prompt (no $s.md in core/prompts)"
  done
done

section "Installation"
manifest="$FACTORY_HOME/manifest.tsv"
if [ -f "$manifest" ]; then
  ok "factory $(cat "$FACTORY_BIN/../VERSION" 2>/dev/null) ($(sed -n 's/^# source //p' "$manifest"))"
  modified=$(awk -F'\t' '!/^#/ && $1 == "file" {print $2 "\t" $3}' "$manifest" | while IFS=$'\t' read -r p h; do
    [ -f "$p" ] && [ "$(sha256sum "$p" | cut -d' ' -f1)" != "$h" ] && echo "$p"; done)
  [ -z "$modified" ] && ok "no managed file was edited locally" ||
    warn "managed files edited locally (updates keep them and write <file>.factory-new): $(tr '\n' ' ' <<<"$modified")"
  pending=$(git ls-files -o --exclude-standard | grep '\.factory-new$' || true)
  [ -z "$pending" ] || warn "updates waiting for a human merge: $(tr '\n' ' ' <<<"$pending")"
else
  warn "no $manifest: not installed by install.sh?"
fi

printf '\n%s error(s), %s warning(s)\n' "$errors" "$warnings"
[ "$errors" -eq 0 ]
