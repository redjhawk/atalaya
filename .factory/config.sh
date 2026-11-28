# Agent factory configuration for Atalaya. Sourced (bash) by .factory/core/bin/*.
# This file is the project's: the installer creates it once and updates never touch it. Every setting is
# documented in .factory/config.example.sh.

# ---- repository ----
FACTORY_BASE_BRANCH=main
FACTORY_LABEL=ai-dev

# ---- documentation layout ----
FACTORY_DOCS=docs                   # specifications/, use-cases/, changes/, adr/, GLOSSARY.md

# ---- the project, as the agents see it ----
# Only the server is open to the factory for now. The locator (apps/locator/) has no build yet, and the
# agent (agent/) is a privileged Magisk module that always needs a human.
FACTORY_STACK="Rust server in server/ (Axum on Tokio, SQLx with SQLite, serde), built as a static x86_64-unknown-linux-musl binary. Integration tests in server/tests/ run the routers in process against a temporary SQLite database."
FACTORY_TEST_HINT="cargo fmt --check --manifest-path server/Cargo.toml; cargo clippy --manifest-path server/Cargo.toml --all-targets --locked --offline -- -D warnings; cargo test --manifest-path server/Cargo.toml --locked --offline. Always pass --manifest-path server/Cargo.toml instead of changing directory."
FACTORY_REQUIRED_TOOLS=(cargo curl)

# Paths the test author and the developer may edit. Dependency manifests (Cargo.toml, Cargo.lock) are
# sensitive and stay with a human.
FACTORY_CODE_PATHS=("server/src/**" "server/tests/**" "server/migrations/**" "server/static/**" "server/README.md")
# Commands they may run. Nothing else runs.
FACTORY_CODE_COMMANDS=("Bash(cargo test:*)" "Bash(cargo check:*)" "Bash(cargo build:*)" "Bash(cargo clippy:*)" "Bash(cargo fmt:*)" "Bash(cargo run:*)")
# Test files: locked after the TDD red gate. Tests live in server/tests/, never inside server/src/.
FACTORY_TEST_FILE_PATTERN='^server/tests/.*\.rs$'
FACTORY_LOCKED_EXTRA=("server/rust-toolchain.toml")

# ---- what the factory never merges on its own ----
# AGENTS.md requires a human for privileged Magisk changes, production deployment, authentication, secret
# handling and SOS behavior. Paths first: the privileged agent, the locator until it has its own gates, the
# project boundary, the manual setup guide and the runner manifests.
FACTORY_SENSITIVE_PATTERN='^(agent/|apps/|factory/|SCOPE\.md$|docs/SETUP\.md$|docs/agents/|server/rust-toolchain\.toml$)'
# Then one-way doors. Schema migrations are hard to undo. The content pattern also catches code that touches
# device identity, secrets or SOS, wherever it lives, so those changes wait for a human too.
FACTORY_ONE_WAY_PATHS='(^|/)migrations/'
FACTORY_ONE_WAY_PATTERN='\b(CREATE|ALTER|DROP)[[:space:]]+(TABLE|INDEX|COLUMN|SCHEMA|DATABASE)\b|fingerprint|certificate|authenticat|authoriz|credential|secret|token|\bsos\b'

# ---- stages ----
# All stages run. The verify stage is enabled by project_start below.

# ---- commands ----

# Fetch the crates once, with network, so that tests and builds run offline afterwards.
project_setup() { cargo fetch --locked --manifest-path server/Cargo.toml; }

# The whole suite: it must fail at the TDD red gate and pass at the green gate. Runs without credentials.
project_tests() {
  cargo fmt --check --manifest-path server/Cargo.toml &&
    cargo clippy --manifest-path server/Cargo.toml --all-targets --locked --offline -- -D warnings &&
    cargo test --manifest-path server/Cargo.toml --locked --offline
}

# New tests must be formatted before they are locked. $1: the test files.
project_red_checks() { cargo fmt --check --manifest-path server/Cargo.toml; }

# ---- verify stage: an agent drives the feature path on the running server ----
project_start() {
  local data; data=$(mktemp -d)
  ATALAYA_DATABASE_PATH="$data/atalaya.db" cargo run --locked --offline --manifest-path server/Cargo.toml
}
project_ready() { curl -fs http://127.0.0.1:8080/healthz >/dev/null; }
FACTORY_VERIFY_URL=http://127.0.0.1:8080
FACTORY_VERIFY_COMMANDS=("Bash(curl:*)" "Bash(cargo run:*)")
