# Technical specification: device enrolment by CLI

Functional specification: `functional.md` (this folder). Origin: issue #12. Test cases:
`docs/changes/2026-11-28-1237-issue-12-server-alta-de-device-por-cli-y-primer-f/testcases.md`.
Decision: ADR 0003 (identity is the client certificate fingerprint, forwarded by the edge, trusted only
on the device listener). Companion: `docs/specifications/telemetry-intake/technical.md`, which uses
the `devices` table defined here.

Sensitive change (AGENTS.md rule 6, device identification): the PR needs human approval. It also
contains the first schema migration, so the merge is left to a human.

## Tiers

| Tier | Change |
| --- | --- |
| Server CLI (`atalaya-server` binary) | New `device add`, `device set-fingerprint`, `device revoke` subcommands. No arguments still starts the server. |
| Server persistence (SQLite) | New migration: `devices` table (and `fixes`, see telemetry-intake). |
| Server HTTP, device API | No enrolment route. Only `GET /healthz` and `POST /telemetry/{device}` (telemetry-intake). |
| Server HTTP, panel | Not affected: still only `GET /healthz`. |
| Server static files (`server/static/`) | Not affected: there is no panel UI in this change. |
| Locator app (`apps/locator/`) | Not affected: certificate enrolment in the locator is blocked on the Keystore check (ADR 0003). |
| Agent (`agent/`) | Not affected: the agent never holds the device key (ADR 0003). |

No new dependency: argument parsing uses `std::env::args_os`, time uses `std::time::SystemTime`.

## Technical requirements

| ID | FR | Requirement |
| --- | --- | --- |
| TS-ENROL-001 | FR-ENROL-001, FR-ENROL-014 | `main` parses the arguments into a `cli::Command` before anything else. No arguments → `Command::Serve` (today's start path, unchanged). Anything else that is not exactly one of the three subcommand forms → usage error. |
| TS-ENROL-002 | FR-ENROL-013 | Validated value types `DeviceId`, `DeviceName`, `Fingerprint` with `parse(&str) -> Result<Self, InputError>`. All arguments are validated before the database is opened. |
| TS-ENROL-003 | FR-ENROL-008, FR-ENROL-009 | A subcommand reads `Config::from_env()`, uses only `database_path`, opens it with `open_database` (create + migrate), runs one operation, closes the pool and exits. It never calls `Server::bind` or binds a socket. |
| TS-ENROL-004 | FR-ENROL-002, FR-ENROL-010, FR-ENROL-011 | `devices::add` inserts the device inside one `BEGIN IMMEDIATE` transaction after checking id and fingerprint. |
| TS-ENROL-005 | FR-ENROL-003, FR-ENROL-011, FR-ENROL-012, FR-ENROL-015 | `devices::set_fingerprint` updates only the `fingerprint` column, inside one `BEGIN IMMEDIATE` transaction. |
| TS-ENROL-006 | FR-ENROL-004, FR-ENROL-012 | `devices::revoke` sets `fingerprint` to `NULL` with one `UPDATE`; an unknown id is an error, an already revoked device is success. |
| TS-ENROL-007 | FR-ENROL-005, FR-ENROL-006, FR-ENROL-007 | No cache of devices anywhere in the server: every telemetry request reads the `devices` row at request time (TS-TELEMETRY-002). The CLI and the running server share the database file in WAL mode. |
| TS-ENROL-008 | FR-ENROL-011 | `devices.fingerprint` has a `UNIQUE` constraint and is stored lowercase. `NULL` (revoked) is not subject to uniqueness, so a revoked fingerprint is free. |
| TS-ENROL-009 | FR-ENROL-016 | No route in `device_router` or `panel_router` writes the `devices` table except the `last_contact` update of an accepted fix. |
| TS-ENROL-010 | FR-ENROL-002–004, FR-ENROL-010–015 | Exit codes and messages as in "Output and errors". |

## Components

New modules in `server/src/`, exported from `lib.rs`:

- `devices.rs`: the enrolment domain.
  - `DeviceId(String)`: 1 to 64 bytes, each `a-z`, `0-9` or `-`, first one not `-`. The check is on
    ASCII bytes, so `teléfono` fails.
  - `DeviceName(String)`: 1 to 100 `char`s (`chars().count()`, not bytes), and
    `!value.trim().is_empty()`. Stored as given, without trimming (`"  Móvil  "` keeps its spaces).
  - `Fingerprint(String)`: exactly 64 bytes, each an ASCII hex digit; stored with
    `to_ascii_lowercase()`. `as_str()` returns the lowercase form.
  - `InputError { field: Field }` with `Field::{Id, Name, Fingerprint}`; its message is fixed per
    field (see "Output and errors").
  - `EnrolError::{DeviceExists(DeviceId), UnknownDevice(DeviceId), FingerprintTaken, Database(sqlx::Error)}`.
  - `async fn add(pool, &DeviceId, &DeviceName, &Fingerprint) -> Result<(), EnrolError>`
  - `async fn set_fingerprint(pool, &DeviceId, &Fingerprint) -> Result<FingerprintChange, EnrolError>`
    with `FingerprintChange::{Replaced, Unchanged}`.
  - `async fn revoke(pool, &DeviceId) -> Result<(), EnrolError>`
- `cli.rs`: argument parsing and running a subcommand.
  - `enum Command { Serve, Device(DeviceCommand) }` and
    `enum DeviceCommand { Add { id, name, fingerprint }, SetFingerprint { id, fingerprint }, Revoke { id } }`
    holding the validated types.
  - `fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, CliError>` (arguments
    after the program name). `CliError::{Usage, Input(InputError)}`. A non-UTF-8 argument is a usage
    error.
  - `pub const USAGE: &str`, listing the three forms (see below).
  - `async fn run(DeviceCommand, &Config) -> Result<String, RunError>` returning the confirmation
    line; `RunError::{Start(StartError), Enrol(EnrolError)}` (opening or migrating the database, or
    the operation).
- `main.rs`: `cli::parse(std::env::args_os().skip(1))`; on `Serve` it runs today's start path
  unchanged; on `Device` it calls `cli::run` and prints; maps errors to exit codes.

`open_database` gains an explicit `.busy_timeout(Duration::from_secs(5))` so that a CLI write and a
server write wait for each other instead of failing with `SQLITE_BUSY`.

## Data flow

`device add phone-1 "Móvil de ejemplo" <FP>`:

1. `cli::parse` checks the arity, then `DeviceId::parse`, `DeviceName::parse`, `Fingerprint::parse`,
   in that order; the first failure is reported. Nothing is opened yet, so an invalid call never
   creates or touches the database file.
2. `Config::from_env()`; a configuration error exits 1 with the existing message.
3. `open_database(&config.database_path)`: creates the file when missing and applies migrations.
4. `devices::add`, in `BEGIN IMMEDIATE`:
   1. `SELECT 1 FROM devices WHERE id = ?` → found: `DeviceExists`.
   2. `SELECT 1 FROM devices WHERE fingerprint = ?` → found: `FingerprintTaken`.
   3. `INSERT INTO devices (id, name, fingerprint, last_contact) VALUES (?, ?, ?, NULL)`.
   4. `COMMIT`. Any error rolls back (drop of the transaction).
   A unique-constraint violation from the `INSERT` (defence in depth) maps to `DeviceExists` when the
   id now exists, else `FingerprintTaken`.
5. `pool.close().await`, print the line, exit 0.

`device set-fingerprint <id> <FP>`, in `BEGIN IMMEDIATE`:

1. `SELECT fingerprint FROM devices WHERE id = ?` → no row: `UnknownDevice`.
2. Current value equals the new one → `COMMIT`, `Unchanged` (no write).
3. `SELECT 1 FROM devices WHERE fingerprint = ? AND id <> ?` → found: `FingerprintTaken`.
4. `UPDATE devices SET fingerprint = ? WHERE id = ?`, `COMMIT`, `Replaced`.

`device revoke <id>`: `UPDATE devices SET fingerprint = NULL WHERE id = ?`; `rows_affected() == 0` →
`UnknownDevice`. SQLite counts matched rows, so an already revoked device returns 1 and succeeds.

`BEGIN IMMEDIATE` takes the write lock at the start, so two concurrent CLI processes (TC-016) or a CLI
and the server serialise: the second waits up to the busy timeout and then sees the first one's
commit. Use `pool.begin_with("BEGIN IMMEDIATE")` if the vendored sqlx offers it, otherwise execute
`BEGIN IMMEDIATE` / `COMMIT` / `ROLLBACK` on one acquired connection.

## Persistence and migration

New file `server/migrations/20261128000000_devices_and_fixes.sql` (sqlx `<version>_<name>.sql`),
applied by `open_database` on server start and on every subcommand:

```sql
CREATE TABLE devices (
    id           TEXT PRIMARY KEY NOT NULL,
    name         TEXT NOT NULL,
    fingerprint  TEXT UNIQUE,          -- lowercase hex; NULL when revoked
    last_contact INTEGER               -- Unix ms; NULL until the first accepted fix
) STRICT;

CREATE TABLE fixes (
    id          INTEGER PRIMARY KEY,
    device_id   TEXT NOT NULL REFERENCES devices (id),
    lat         REAL NOT NULL,
    lon         REAL NOT NULL,
    acc         REAL NOT NULL,
    ts          INTEGER NOT NULL,      -- Unix ms, device clock
    prov        TEXT NOT NULL CHECK (prov IN ('gps', 'fused', 'network')),
    received_at INTEGER NOT NULL       -- Unix ms, server clock
) STRICT;

CREATE INDEX fixes_by_device ON fixes (device_id, received_at);
```

The table and column names are part of this change's test contract: the tests read devices and fixes
from the database file (testcases.md, "Reading state"). Validation lives in Rust; the schema adds
`UNIQUE`, `NOT NULL`, the `prov` check and the foreign key as a backstop.

- **Data at risk**: none. This is the first migration; no table existed before.
- **Hard to undo**: once applied, a binary built before this change refuses to start on that file,
  because sqlx reports a previously applied migration it does not know.
- **Rollback**: stop the server and either restore the database file from before the upgrade, or
  remove it (losing enrolled devices and fixes), or run
  `DROP TABLE fixes; DROP TABLE devices; DELETE FROM _sqlx_migrations WHERE version = 20261128000000;`.
  Re-enrolling the device afterwards is the manual `device add`.
- **Migration race**: two processes opening a missing file at the same instant can both try to apply
  the migration; the loser exits 1 with "cannot migrate the database" and changes nothing (the
  migration runs in a transaction). Re-running the command succeeds. TC-016 therefore starts from a
  database already migrated by `open_database`.

## Output and errors

Program-name prefix `atalaya-server: ` on every stderr line, as `main` does today.

| Case | Stream | Text | Exit |
| --- | --- | --- | --- |
| add ok | stdout | `device <id> enrolled` | 0 |
| set-fingerprint replaced | stdout | `device <id> fingerprint replaced` | 0 |
| set-fingerprint same value | stdout | `device <id> fingerprint unchanged` | 0 |
| revoke ok (also repeated) | stdout | `device <id> revoked` | 0 |
| usage | stderr | `invalid arguments` + newline + `USAGE` | 2 |
| invalid id | stderr | `invalid device id: 1 to 64 characters a-z, 0-9 or -, not starting with -` | 2 |
| invalid name | stderr | `invalid name: 1 to 100 characters, not only spaces` | 2 |
| invalid fingerprint | stderr | `invalid fingerprint: 64 hexadecimal characters, no separators` | 2 |
| id exists | stderr | `device <id> already exists` | 1 |
| unknown id | stderr | `no device <id>` | 1 |
| fingerprint taken | stderr | `fingerprint already belongs to another device` | 1 |
| config, database or migration | stderr | existing `ConfigError` / `StartError` text | 1 |

`USAGE`:

```text
usage:
  atalaya-server                                         start the server
  atalaya-server device add <id> <name> <fingerprint>
  atalaya-server device set-fingerprint <id> <fingerprint>
  atalaya-server device revoke <id>
```

The invalid-id message never echoes the rejected value; the other messages that name an id use the
validated id only. Tests should match the specific prefixes (`invalid device id`, `invalid name`,
`invalid fingerprint`), because a bare `id` also matches inside `invalid`.

## Concurrency and compatibility

- The server and the CLI may run at the same time on the same file (WAL, busy timeout, immediate
  transactions). The server holds no device state in memory (TS-ENROL-007).
- Starting with no arguments behaves as before: same configuration, listeners, log line and shutdown.
- `device_router` gains a parameter (telemetry-intake TS-TELEMETRY-001); this is the only public API
  break, and the only callers are `Server::run` and the tests.

## Files to change

| File | Change |
| --- | --- |
| `server/src/devices.rs` | New: value types, validation, `add`, `set_fingerprint`, `revoke`. |
| `server/src/cli.rs` | New: `Command`, `parse`, `USAGE`, `run`. |
| `server/src/lib.rs` | `pub mod devices; pub mod cli;`, busy timeout in `open_database`. |
| `server/src/main.rs` | Dispatch on `cli::parse`; exit codes 0/1/2. |
| `server/migrations/20261128000000_devices_and_fixes.sql` | New migration (above). |
| `server/README.md` | "Current state", a "Device enrolment" section with the three commands and example fake values, schema summary; the fingerprint header row no longer says "not used yet". |

## Test files

Shared helpers in `server/tests/common/mod.rs` (included with `mod common;`): the example values of
testcases.md, a CLI runner that spawns `env!("CARGO_BIN_EXE_atalaya-server")` with every `ATALAYA_*`
variable removed and then set per case, readers of `devices` and `fixes` rows from the database file,
request builders for the routers, and a Unix-ms clock for `t0`/`t1`.

| File | Test cases |
| --- | --- |
| `server/tests/enrolment_cli.rs` | TC-001–TC-010, TC-016, TC-021, TC-023 |
| `server/tests/enrolment_http.rs` | TC-011–TC-015, TC-017–TC-020, TC-022 |
| `server/tests/routes.rs` | TC-025, TC-026 |
| `server/tests/operator_flows.rs` | TC-024, TC-038–TC-041 |

TC-024 sends `SIGTERM` with `std::process::Command::new("kill").args(["-TERM", &pid])`, because the
standard library only offers `SIGKILL` and no signal crate is a dependency. TC-004 and TC-023 hold two
`std::net::TcpListener`s on loopback port 0 and pass their ports; they wait on the child with a
10-second deadline (poll `try_wait`), kill it on timeout and fail.

## Refactoring

None beyond what the feature needs: `main.rs` gains the dispatch, and `open_database` the busy timeout
(reason: concurrent CLI and server writers; scope: one builder call; risk: a writer may now wait up to
5 s instead of failing at once).

## Manual follow-ups

None required. Production enrolment still needs the operator's `adb` and Cloudflare steps of ADR 0003,
outside this repository.
