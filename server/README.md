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

The server starts, creates and migrates its SQLite database, and answers a health check. It serves no device or panel functionality yet.

It runs two listeners, both on the loopback interface only:

- the **device API**, reached through the hostname that requires mTLS;
- the **panel**, reached through the hostname behind an interactive login.

They are separate because the certificate fingerprint header is only trusted on the device listener (ADR 0003). The server refuses to start if both are given the same port.

`GET /healthz` on either listener returns `200` when the database answers a query and `503` when it does not.

## Configuration

Environment variables, all optional:

| Variable | Default | Meaning |
| --- | --- | --- |
| `ATALAYA_DEVICE_PORT` | `8080` | Port of the device API listener. |
| `ATALAYA_PANEL_PORT` | `8081` | Port of the panel listener. |
| `ATALAYA_DATABASE_PATH` | `atalaya.db` | SQLite file. Created on startup when missing. |
| `ATALAYA_FINGERPRINT_HEADER` | `x-client-cert-sha256` | Request header carrying the client certificate fingerprint. Read, not used yet. |

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

Tests exercise public behavior at the HTTP boundary: the routers run in process against a temporary SQLite database, and a request goes in. New tests follow `tests/health.rs`. Schema changes go in `migrations/` as `<version>_<name>.sql` files and are applied on startup.
