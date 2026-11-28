# Technical specification: telemetry intake authenticated by fingerprint

Functional specification: `functional.md` (this folder). Origin: issue #12. Test cases:
`docs/changes/2026-11-28-1237-issue-12-server-alta-de-device-por-cli-y-primer-f/testcases.md`.
Decision: ADR 0003. Schema and migration: `docs/specifications/device-enrolment/technical.md`
(`devices` and `fixes` tables, one migration for both).

Sensitive change (AGENTS.md rule 6, device identification): the PR needs human approval.

## Tiers

| Tier | Change |
| --- | --- |
| Server HTTP, device API | New `POST /telemetry/{device}`; the router reads the configured fingerprint header. |
| Server HTTP, panel | Not affected: no telemetry route, so `POST /telemetry/{device}` there is `404` by construction. |
| Server persistence (SQLite) | Writes `fixes` rows and `devices.last_contact`; schema in the enrolment migration. |
| Server static files (`server/static/`) | Not affected: no map or panel UI in this change. |
| Locator app (`apps/locator/`) | Not affected: the request shape matches what the locator will send; the client is a later change. |
| Agent (`agent/`) | Not affected. |

No new dependency: `serde`/`serde_json` for the body, `std::time::SystemTime` for the clock.

## Technical requirements

| ID | FR | Requirement |
| --- | --- | --- |
| TS-TELEMETRY-001 | FR-TELEMETRY-001, FR-TELEMETRY-008 | `device_router(pool: SqlitePool, fingerprint_header: HeaderName) -> Router` serves `GET /healthz` and `POST /telemetry/{device}`. `Server` keeps `config.fingerprint_header` from `bind` and passes it in `run`. `panel_router(pool)` is unchanged. |
| TS-TELEMETRY-002 | FR-TELEMETRY-003, FR-TELEMETRY-007 | Identity check: the request carries exactly one value of the configured header, it is valid visible ASCII, and `SELECT 1 FROM devices WHERE id = ?path AND fingerprint = ?header` returns a row. The comparison is SQLite's default binary `=`, so only the exact lowercase value matches. Read at request time, no cache. |
| TS-TELEMETRY-003 | FR-TELEMETRY-003 | Every identity failure returns the same response: `401`, `content-type: text/plain; charset=utf-8`, body `unauthorized\n`, built by one function. |
| TS-TELEMETRY-004 | FR-TELEMETRY-004 | The body is taken as `axum::body::Body` (last extractor) and read only after the identity check passes. |
| TS-TELEMETRY-005 | FR-TELEMETRY-005 | Body parsing and validation into a `Fix`; any failure returns `400`, body `invalid fix\n`. |
| TS-TELEMETRY-006 | FR-TELEMETRY-001, FR-TELEMETRY-002, FR-TELEMETRY-006, FR-TELEMETRY-007 | Storage in one transaction that re-checks the fingerprint: the fix insert and the `last_contact` update both happen or neither does. |
| TS-TELEMETRY-007 | FR-TELEMETRY-001, FR-TELEMETRY-002 | Reception time `received_at` = Unix ms from `SystemTime::now()` taken once at the start of the handler, before any database access; it is written to `fixes.received_at` and `devices.last_contact`. |
| TS-TELEMETRY-008 | FR-TELEMETRY-001 | Success returns `201 Created` with an empty body. |

## Components

New module `server/src/telemetry.rs`, used by `device_router`:

- `struct DeviceState { pool: SqlitePool, fingerprint_header: HeaderName }` (`Clone`), with
  `FromRef<DeviceState> for SqlitePool` so the existing `health` handler keeps `State<SqlitePool>`.
- `async fn receive(State<DeviceState>, Path<String>, HeaderMap, Body) -> Response`.
- `struct Report { location: Fix }` and
  `struct Fix { lat: f64, lon: f64, acc: f64, ts: i64, prov: Provider }`, `Deserialize`, unknown
  fields ignored (serde default), so other sections and extra `location` fields (`alt`, `speed`) are
  never parsed.
- `enum Provider { Gps, Fused, Network }` with `#[serde(rename_all = "lowercase")]` and
  `as_str()` for storage. `GPS`, `""` and `wifi` fail to deserialize.
- `fn Fix::validate(&self) -> bool`: `-90.0 <= lat <= 90.0`, `-180.0 <= lon <= 180.0`,
  `acc >= 0.0`, `ts > 0`.

`lib.rs`: `pub mod telemetry;` and the new `device_router` signature; `Server` gains a
`fingerprint_header: HeaderName` field.

## Data flow

`POST /telemetry/{device}` on the device listener:

1. `received_at = now_ms()`.
2. Identity (TS-TELEMETRY-002). Header missing, repeated, not `to_str()`-able, or no matching row →
   `401` (TS-TELEMETRY-003). Covers: no header, unknown fingerprint, another device's fingerprint,
   unknown `{device}`, revoked device (`fingerprint IS NULL` never equals a value), wrong case,
   surrounding spaces, `sha256:` prefix, colon form. The `{device}` path segment is not validated
   against the id rules: an id that cannot exist simply finds no row.
3. Body: `axum::body::to_bytes(body, 64 * 1024)`; an error (including a body over 64 KiB) → `400`.
4. `serde_json::from_slice::<Report>`; error → `400`. This rejects non-JSON, an empty body, `{}`,
   `"location": null`, `"location": []`, a missing field, `ts` as a string or `1.5`, `lat` as a
   string, an unknown `prov`. JSON numbers outside the `f64` or `i64` range fail to parse. Integer
   `lat`/`lon` (`-90`, `180`) deserialize into `f64` and are accepted.
5. `fix.validate()`; false → `400` (bounds, `acc < 0`, `ts <= 0`, so `ts` `0` and `-5`).
6. Store, in one transaction:
   1. `INSERT INTO fixes (device_id, lat, lon, acc, ts, prov, received_at)
       SELECT id, ?, ?, ?, ?, ?, ? FROM devices WHERE id = ? AND fingerprint = ?`.
   2. `rows_affected() == 0` → roll back, `401`: the fingerprint was replaced or revoked between
      step 2 and now (FR-TELEMETRY-007).
   3. `UPDATE devices SET last_contact = ? WHERE id = ?`.
   4. `COMMIT` → `201`.
   The first statement is a write, so the transaction takes the write lock immediately and concurrent
   requests (TC-031) queue on the busy timeout instead of failing on a stale read snapshot.
7. A database error at any step → `503`, body `database unavailable\n`, as `GET /healthz` does;
   nothing is stored (the transaction rolls back on drop).

The header value is compared exactly as received; the server never normalises it. The edge forwards
lowercase hex (ADR 0003) and the CLI stores lowercase (device-enrolment TS-ENROL-002).

## Validation notes

- Content type is not checked: the edge and the locator send JSON, and the body is parsed regardless.
- `last_contact` is set to `received_at`, not to `ts`: the device clock is kept apart from reception
  time (TC-027).
- No deduplication: two identical valid requests store two fixes (TC-030).

## Errors

| Condition | Status | Body |
| --- | --- | --- |
| Accepted | `201` | empty |
| Identity failure (step 2 or 6.2) | `401` | `unauthorized\n` |
| Body unreadable, too large, not a valid report, out of bounds | `400` | `invalid fix\n` |
| Database error | `503` | `database unavailable\n` |
| Other method on `/telemetry/{device}` | `405` (axum) | axum default |
| Any telemetry request on the panel | `404` (no route) | axum default |

## Concurrency and compatibility

- Each request runs its own transaction; the pool and the busy timeout serialise writers. The CLI may
  change a fingerprint at any time; the re-check in step 6 makes the decision use the row as it is
  when the fix is written.
- Breaking change to the library API: `device_router` takes the fingerprint header. `tests/health.rs`
  is updated to pass `config.fingerprint_header` (or the default `HeaderName::from_static
  ("x-client-cert-sha256")`).
- The device API listener and the panel listener stay separate; nothing on the panel reads the
  fingerprint header.

## Files to change

| File | Change |
| --- | --- |
| `server/src/telemetry.rs` | New: state, handler, `Report`, `Fix`, `Provider`, storage. |
| `server/src/lib.rs` | `pub mod telemetry;`, `device_router` signature and route, `Server.fingerprint_header`. |
| `server/tests/health.rs` | Call the new `device_router` signature. |
| `server/README.md` | Document `POST /telemetry/{device}`, its body, the status codes, and that the fingerprint header is now used. |

## Test files

Shared helpers in `server/tests/common/mod.rs` (see device-enrolment). Devices are enrolled with the
CLI binary on the temporary database file; requests go through `tower::ServiceExt::oneshot` on the
routers, or over TCP for end-to-end cases.

| File | Test cases |
| --- | --- |
| `server/tests/telemetry.rs` | TC-027–TC-037 |
| `server/tests/routes.rs` | TC-025, TC-026 (both routers, no enrolment route) |
| `server/tests/operator_flows.rs` | TC-038–TC-041 (running server over TCP) |

TC-032 builds the router from `Config::from_lookup` with `ATALAYA_FINGERPRINT_HEADER=x-test-fingerprint`
and passes `config.fingerprint_header` to `device_router`. TC-031 sends ten `oneshot` calls on clones
of one router, each in `tokio::spawn`, and awaits every handle (no `futures` crate needed).

## Refactoring

None. `health` keeps its signature through `FromRef`.

## Manual follow-ups

None required for this change. The edge transform rule that sets `X-Client-Cert-Sha256` is the
operator's Cloudflare configuration (ADR 0003), outside this repository.
