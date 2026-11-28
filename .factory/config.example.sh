# Reference of every setting of the agent factory, with an example value. Managed by the installer: it is
# replaced on each update, so do not edit it. Copy what you need to .factory/config.sh.

# ---- repository ----
FACTORY_BASE_BRANCH=main            # the branch the factory merges into
FACTORY_LABEL=ai-dev                # the label that starts the factory on an issue
# FACTORY_TRUSTED_ACTORS=owner      # comma-separated GitHub logins whose issues and comments are instructions (default: the repository owner)

# ---- documentation layout (all under FACTORY_DOCS) ----
FACTORY_DOCS=docs                   # specifications/, use-cases/, changes/, adr/, GLOSSARY.md
# FACTORY_API_CONTRACT=API.md       # a canonical API contract file the spec stages keep current (optional)

# ---- the project, as the agents see it ----
FACTORY_STACK="describe the stack here, e.g. TypeScript + Node backend, React frontend, PostgreSQL"
FACTORY_TEST_HINT="npm test, npm run build"   # the commands the agents run, in words
FACTORY_REQUIRED_TOOLS=(node npm)  # checked before a run

# Paths the test author and the developer may edit (Claude Code Edit globs). Sensitive paths are refused.
FACTORY_CODE_PATHS=("src/**" "tests/**")
# Commands they may run (Claude Code Bash rules). Nothing else runs.
FACTORY_CODE_COMMANDS=("Bash(npm test:*)" "Bash(npm run build)" "Bash(npx vitest:*)")
# Test files (extended regex): locked after the TDD red gate.
FACTORY_TEST_FILE_PATTERN='(\.(test|spec)\.[cm]?[jt]sx?)$'
# Files the gates execute that must not change after the red gate either (globs), e.g. the test runner config.
FACTORY_LOCKED_EXTRA=("vitest.config.*" "playwright.config.*")

# ---- what the factory never merges on its own ----
# Always protected: CI, the factory, agent instructions and skills, dependency manifests. Add your own (regex):
FACTORY_SENSITIVE_PATTERN='^(deploy/|infra/|Dockerfile$)'
# FACTORY_DEPENDENCY_PATTERN='(^|/)(package(-lock)?\.json|go\.(mod|sum)|...)$'   # dependency manifests (default: the common ones of npm, Go, Rust, Python, Ruby, PHP)
# FACTORY_SENSITIVE_EXCEPT='^deploy/README\.md$'   # narrows your own patterns (never the core list or the skills)
# One-way doors: hard to undo after the merge, so a human merges them (regex on paths / on added lines).
# FACTORY_ONE_WAY_PATHS='(^|/)migrations?/'
# FACTORY_ONE_WAY_PATTERN='\b(CREATE|ALTER|DROP)[[:space:]]+(TABLE|INDEX|COLUMN|SCHEMA|DATABASE)\b'

# ---- stages ----
# FACTORY_SKIP_STAGES="usecases techspec"   # optional stages: usecases, techspec, verify
# FACTORY_SKILL_DIRS=".agents/skills .claude/skills project-skills skills"   # where the project's skills live (read, never edited)
# FACTORY_LOG_NOISE='^\[WebServer\]'        # gate output lines to hide in issue comments (regex)

# ---- commands (bash functions) ----

# Install dependencies (CI, and the base-branch check in a fresh worktree).
project_setup() { npm ci; }

# The whole suite: it must fail at the TDD red gate and pass at the green gate. Runs without credentials.
project_tests() { npm test && npm run build; }

# Extra checks on the new tests before they are locked (formatting, the runner lists them). $1: the test files.
# project_red_checks() { :; }

# Security scan before merging (FACTORY_SECURITY=0 skips it).
# project_security_scan() { npm audit --omit=dev --audit-level=high; }

# ---- verify stage: an agent drives the feature path on the running system ----
# Define project_start to enable it. It runs in the background, without credentials, and is stopped afterwards.
# project_start() { npm run build && PORT=4173 npm run preview; }
# project_ready() { curl -fs http://localhost:4173 >/dev/null; }
# FACTORY_VERIFY_URL=http://localhost:4173
# FACTORY_VERIFY_COMMANDS=("Bash(npx playwright-cli:*)" "Bash(curl:*)")
