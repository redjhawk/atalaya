# Atalaya Server

Rust/Axum API and web interface backed by SQLite.

Component responsibilities:

- telemetry and location persistence,
- command queue and acknowledgements,
- WebSocket delivery,
- map and device status UI,
- signed package release metadata.

The release target is `x86_64-unknown-linux-musl` for the T310.

## Current state

The server starts, creates and migrates its SQLite database, and answers a health check. The operator enrols a device from the command line, and the enrolled device stores fixes through `POST /telemetry/{device}`, authenticated by its certificate fingerprint (ADR 0003). The panel serves nothing but its health check yet.

It runs two listeners, both on the loopback interface only:

- the **device API**, reached through the hostname that requires mTLS;
- the **panel**, reached through the hostname behind an interactive login.

They are separate because the certificate fingerprint header is only trusted on the device listener (ADR 0003). The server refuses to start if both are given the same port.

`GET /healthz` on either listener returns `200` when the database answers a query and `503` when it does not.

## Device enrolment

Enrolment is manual and on site: there is no enrolment route on any listener. The same binary, with arguments, changes the database at `ATALAYA_DATABASE_PATH` (created and migrated when missing) and exits; it never opens a listener. A running server sees the change on its next request, without a restart.

```sh
atalaya-server device add phone-1 "Móvil de ejemplo" aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
atalaya-server device set-fingerprint phone-1 bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
atalaya-server device revoke phone-1
```

- **id**: 1 to 64 characters `a-z`, `0-9` or `-`, not starting with `-`.
- **name**: 1 to 100 characters, not only spaces.
- **fingerprint**: the SHA-256 of the client certificate, 64 hex digits without separators. Stored lowercase, the form the edge forwards. One fingerprint belongs to one device; a revoked one is free again.

`set-fingerprint` and `revoke` keep the device's name, fixes and last contact. A revoked device is rejected until it gets a fingerprint again.

Exit codes: `0` with one confirmation line on stdout; `2` for wrong arguments (the usage is printed) or invalid values; `1` for an existing id, an unknown id, a fingerprint taken by another device, or a configuration or database error.

## Telemetry intake

`POST /telemetry/{device}`, device listener only. The fix goes in `location`; every other field is ignored.

```json
{ "location": { "lat": 40.0, "lon": -3.0, "acc": 12.5, "ts": 1700000000000, "prov": "fused" } }
```

`lat` in [-90, 90], `lon` in [-180, 180], `acc` ≥ 0 (metres), `ts` a positive integer (Unix ms, device clock), `prov` one of `gps`, `fused`, `network`.

| Status | When |
| --- | --- |
| `201` | The fingerprint header is exactly the one enrolled for `{device}` and the fix is valid. The fix is stored with the reception time, which also becomes the device's last contact. |
| `401` | No header, unknown fingerprint, another device's fingerprint, unknown or revoked device. Always the same body, and checked before the body. |
| `400` | Right fingerprint, but the body is not JSON, has no `location`, or the fix breaks a rule. |
| `503` | The database fails. |

A rejected request stores nothing and leaves the last contact unchanged. The panel has no telemetry route: there it is `404`.

## Schema

Migrations in `migrations/`, applied on startup and by every subcommand:

- `devices`: `id`, `name`, `fingerprint` (unique, lowercase, `NULL` when revoked), `last_contact` (Unix ms, `NULL` until the first fix).
- `fixes`: `device_id`, `lat`, `lon`, `acc`, `ts` (device clock), `prov`, `received_at` (server clock).

## Configuration

Environment variables, all optional:

| Variable | Default | Meaning |
| --- | --- | --- |
| `ATALAYA_DEVICE_PORT` | `8080` | Port of the device API listener. |
| `ATALAYA_PANEL_PORT` | `8081` | Port of the panel listener. |
| `ATALAYA_DATABASE_PATH` | `atalaya.db` | SQLite file. Created on startup when missing. |
| `ATALAYA_FINGERPRINT_HEADER` | `x-client-cert-sha256` | Request header carrying the client certificate fingerprint, read on the device listener only. |

The server exits with an error message when a value is invalid.

## Build, test and run

The toolchain version is pinned in `rust-toolchain.toml`; `rustup` installs it on first use. The static build also needs the musl C compiler (`musl-tools` on Debian and Ubuntu), because SQLite is compiled from source.

```sh
cd server
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked --target x86_64-unknown-linux-musl
```

Run it locally and check it:

```sh
ATALAYA_DATABASE_PATH=/tmp/atalaya-dev.db cargo run
curl -i http://127.0.0.1:8080/healthz
curl -i http://127.0.0.1:8081/healthz
```

The server stops cleanly on Ctrl-C or `SIGTERM`.

## Tests

Tests exercise public behavior at the HTTP boundary and the command line: the routers run in process against a temporary SQLite database, the CLI runs as a child process on the same file, and the end-to-end cases run the whole server over TCP. Shared helpers are in `tests/common/mod.rs`. Schema changes go in `migrations/` as `<version>_<name>.sql` files and are applied on startup.
