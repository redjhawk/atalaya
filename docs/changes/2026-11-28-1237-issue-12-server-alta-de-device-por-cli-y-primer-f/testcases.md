# Test cases: issue #12, enrolment by CLI and first fix authenticated by fingerprint

Specifications: `docs/specifications/device-enrolment/functional.md`,
`docs/specifications/telemetry-intake/functional.md`. Use cases: `docs/use-cases/device-enrolment/`,
`docs/use-cases/telemetry-intake/`.

## Conventions

- **Example values** (fake, never real): fingerprints `FP_A` = 64 × `a`, `FP_B` = 64 × `b`,
  `FP_C` = 64 × `c`, `FP_F` = 64 × `f` (enrolled to no device), `FP_A_UPPER` = 64 × `A`. Device ids
  `phone-1`, `phone-2`, `phone-3`; `phone-9` is never enrolled. Name `Móvil de ejemplo`.
- **Valid fix body** `VALID`:
  `{"location":{"lat":40.0,"lon":-3.0,"acc":12.5,"ts":1700000000000,"prov":"fused"}}`.
- **Header**: the default fingerprint header `x-client-cert-sha256`, unless a case says otherwise.
- **Levels**:
  - *integration (CLI)*: the built `atalaya-server` binary runs as a child process with
    `ATALAYA_DATABASE_PATH` pointing to a temporary file; the test reads its exit code, stdout and
    stderr.
  - *integration (HTTP)*: the real `device_router` / `panel_router` in process against a temporary
    SQLite database migrated by `open_database`; devices are enrolled with the CLI binary on that file.
  - *end-to-end*: the operator's interfaces as deployed: the CLI binary and a running server
    (`Server::bind` + `run`, or the binary with no arguments) on the same database file, requests over
    real TCP to the bound listeners.
- **Reading state**: this change has no read interface for devices or fixes (listing and querying are
  excluded), so "stored fixes" and "last contact" are read from the temporary database file. Last
  contact and reception time are checked against the wall clock bracketed by the test: `t0` taken just
  before sending, `t1` just after the response, both Unix milliseconds; the expected value is
  `t0 <= value <= t1`.
- **"Database unchanged"** is proven through behaviour where possible (the old fingerprint still
  works, the id can still be enrolled) and otherwise by comparing the device rows before and after.

## Enrolment CLI

| ID | FR | UC | Level | Given | When | Then | Corner cases |
| --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | FR-ENROL-002 | UC-ENROL-001 | integration (CLI) | An empty, migrated database. | `device add phone-1 "Móvil de ejemplo" FP_A` | Exit code 0; stdout is one line that contains `phone-1`; the database holds one device `phone-1` named `Móvil de ejemplo` with fingerprint `FP_A`, no fixes and no last contact. | Non-ASCII name. |
| TC-002 | FR-ENROL-008 | UC-ENROL-001 | integration (CLI) | `ATALAYA_DATABASE_PATH` points to a file that does not exist in an empty temporary directory. | `device add phone-1 "Móvil de ejemplo" FP_A` | Exit code 0; the file now exists; a server opened on it accepts `POST /telemetry/phone-1` with `FP_A` and `VALID` with `201`. | Missing database is created and migrated. |
| TC-003 | FR-ENROL-008 | UC-ENROL-001 | integration (CLI) | `ATALAYA_DATABASE_PATH` unset; the child's working directory is an empty temporary directory. | `device add phone-1 "Móvil de ejemplo" FP_A` | Exit code 0; `atalaya.db` exists in that directory. | Default path, same as the server's. |
| TC-004 | FR-ENROL-009 | UC-ENROL-001, UC-ENROL-002, UC-ENROL-003 | integration (CLI) | The test holds open TCP listeners on two loopback ports and sets `ATALAYA_DEVICE_PORT` and `ATALAYA_PANEL_PORT` to them; `phone-1` is enrolled with `FP_A`. | Run, one at a time: `device add phone-2 "Otro" FP_B`, `device set-fingerprint phone-2 FP_C`, `device revoke phone-2`. | Each exits with code 0 within 10 seconds, without the test sending any signal (it neither binds a port nor stays serving). | Ports already taken. |
| TC-005 | FR-ENROL-013 | UC-ENROL-001 | integration (CLI) | An empty database. | `device add` with valid name and fingerprint and each id: `a`, `0`, `1phone`, 64 × `a`. | Each exits with code 0. | Id boundaries: 1 and 64 characters, leading digit. |
| TC-006 | FR-ENROL-013 | UC-ENROL-001 E3 | integration (CLI) | An empty database. | `device add` with valid name and fingerprint and each id: `""`, 65 × `a`, `Phone-1`, `-phone`, `phone_1`, `phone 1`, `teléfono`. | Each exits non-zero; stderr contains `id`; afterwards `device add phone-1 "Móvil de ejemplo" FP_A` exits 0 (no device was created by the failed calls). | Empty, too long, uppercase, leading hyphen, underscore, space, non-ASCII. |
| TC-007 | FR-ENROL-013 | UC-ENROL-001 | integration (CLI) | An empty database. | `device add` with ids `n1`…`n3`, distinct fingerprints and names `x` (1 char), 100 × `ó` (100 characters, 200 bytes), `  Móvil  ` (surrounded by spaces). | Each exits with code 0. | Name boundaries counted in characters, not bytes. |
| TC-008 | FR-ENROL-013 | UC-ENROL-001 E3 | integration (CLI) | An empty database. | `device add phone-1 <name> FP_A` with names `""`, `"   "`, 101 × `a`. | Each exits non-zero; stderr contains `name`; afterwards `device add phone-1 "Móvil de ejemplo" FP_A` exits 0. | Empty, only spaces, too long. |
| TC-009 | FR-ENROL-013 | UC-ENROL-001 E3, UC-ENROL-002 E3 | integration (CLI) | An empty database for `device add`; `phone-1` enrolled with `FP_A` for `set-fingerprint`. | `device add phone-2 "Otro" <fp>` and `device set-fingerprint phone-1 <fp>` with: 63 × `b`, 65 × `b`, `""`, 63 × `b` + `g`, 32 pairs `bb` joined by `:`, 32 × `b` + ` ` + 31 × `b`. | Each exits non-zero; stderr contains `fingerprint`; `phone-2` can still be enrolled with `FP_B`; `phone-1` still gets `201` with `FP_A`. | Length 63/65, empty, non-hex, colon form, space. |
| TC-010 | FR-ENROL-013, FR-ENROL-012 | UC-ENROL-002 E1/E3, UC-ENROL-003 E1/E2 | integration (CLI) | `phone-1` enrolled with `FP_A`. | `device set-fingerprint phone-9 FP_B`; `device revoke phone-9`; `device revoke Phone-1`; `device set-fingerprint -x FP_B`. | Each exits non-zero; stderr of the `phone-9` calls contains `phone-9`; stderr of the invalid ids contains `id`; `phone-1` still gets `201` with `FP_A`; `device add phone-9 "Nueve" FP_B` then exits 0. | Unknown id, invalid id on both subcommands. |
| TC-011 | FR-ENROL-013, FR-ENROL-011 | UC-ENROL-001 A1 | integration (HTTP) | An empty database. | `device add phone-1 "Móvil de ejemplo" FP_A_UPPER`, then `POST /telemetry/phone-1` with header `FP_A` (lowercase) and `VALID`. | The CLI exits 0; the database stores the fingerprint as `FP_A` (64 × `a`); the request gets `201`; `device add phone-2 "Otro" FP_A` then exits non-zero (same fingerprint, other case). | Uppercase input stored lowercase; uniqueness is case-insensitive. Mixed case (`Aa` × 32) also exits 0. |
| TC-012 | FR-ENROL-010 | UC-ENROL-001 E1 | integration (HTTP) | `phone-1` enrolled with `FP_A` as `Móvil de ejemplo`. | `device add phone-1 "Otro nombre" FP_B` | Exit non-zero; stderr contains `phone-1`; `phone-1` keeps name `Móvil de ejemplo`; `POST /telemetry/phone-1` with `FP_A` gets `201`, with `FP_B` gets `401`. | Repeated action. |
| TC-013 | FR-ENROL-011 | UC-ENROL-001 E2 | integration (HTTP) | `phone-1` enrolled with `FP_A`. | `device add phone-2 "Otro" FP_A` | Exit non-zero; stderr contains `fingerprint`; `device add phone-2 "Otro" FP_B` then exits 0 (no `phone-2` was created). | Duplicate fingerprint on add. |
| TC-014 | FR-ENROL-011 | UC-ENROL-002 E2 | integration (HTTP) | `phone-1` with `FP_A`, `phone-2` with `FP_B`. | `device set-fingerprint phone-2 FP_A` | Exit non-zero; stderr contains `fingerprint`; `POST /telemetry/phone-2` with `FP_B` gets `201`; `POST /telemetry/phone-1` with `FP_A` gets `201`. | Duplicate fingerprint on replace. |
| TC-015 | FR-ENROL-011 | UC-ENROL-001 A2, UC-ENROL-002 A3, UC-ENROL-003 A3 | integration (HTTP) | `phone-1` enrolled with `FP_A` and then revoked; `phone-3` enrolled with `FP_C`. | `device add phone-2 "Otro" FP_A`; then `device revoke phone-2`; then `device set-fingerprint phone-3 FP_A`. | All exit 0; `POST /telemetry/phone-3` with `FP_A` gets `201`; `POST /telemetry/phone-1` and `/telemetry/phone-2` with `FP_A` get `401`. | A revoked fingerprint is free for add and for replace. |
| TC-016 | FR-ENROL-011 | UC-ENROL-001 E2 | integration (CLI) | An empty database. | Two `device add` processes start at the same time: `phone-1 "Uno" FP_A` and `phone-2 "Dos" FP_A`. | Exactly one exits 0 and the other exits non-zero; the database holds exactly one device with `FP_A`. | Concurrency. |
| TC-017 | FR-ENROL-003 | UC-ENROL-002 | integration (HTTP) | `phone-1` enrolled with `FP_A` as `Móvil de ejemplo`, with one accepted fix (`VALID`); its last contact `L` is read from the database. | `device set-fingerprint phone-1 FP_B` | Exit 0; stdout is one line that contains `phone-1`; the device has fingerprint `FP_B`, name `Móvil de ejemplo`, one fix with `lat` 40.0, and last contact still `L`. | History preserved. |
| TC-018 | FR-ENROL-003 | UC-ENROL-002 A1 | integration (HTTP) | `phone-1` enrolled with `FP_A` and revoked. | `device set-fingerprint phone-1 FP_B`, then `POST /telemetry/phone-1` with `FP_B` and `VALID`. | Exit 0; the request gets `201`. | Replace on a revoked device. |
| TC-019 | FR-ENROL-015 | UC-ENROL-002 A2 | integration (HTTP) | `phone-1` enrolled with `FP_A`, with one fix and last contact `L`. | `device set-fingerprint phone-1 FP_A`, then the same with `FP_A_UPPER`. | Both exit 0; the device still has fingerprint `FP_A`, one fix and last contact `L`; `POST /telemetry/phone-1` with `FP_A` gets `201`. | Same fingerprint, also in another case. |
| TC-020 | FR-ENROL-004 | UC-ENROL-003 | integration (HTTP) | `phone-1` enrolled with `FP_A` as `Móvil de ejemplo`, with one accepted fix and last contact `L`. | `device revoke phone-1` | Exit 0; stdout is one line that contains `phone-1`; the device has no fingerprint, name `Móvil de ejemplo`, one fix with `lat` 40.0 and last contact `L`. | Fixes kept after revoke. |
| TC-021 | FR-ENROL-004 | UC-ENROL-003 A1 | integration (CLI) | `phone-1` enrolled with `FP_A` and revoked. | `device revoke phone-1` again. | Exit 0; stdout is one line that contains `phone-1`; the device still has no fingerprint. | Repeated action. |
| TC-022 | FR-ENROL-006, FR-TELEMETRY-003 | UC-ENROL-003, UC-TELEMETRY-001 E1 | integration (HTTP) | `phone-1` enrolled with `FP_A` and revoked; `phone-2` enrolled with `FP_B`. | `POST /telemetry/phone-1` with `VALID` and, in turn, header `FP_A`, `FP_B`, `FP_F`, and no header. | All four get `401`; `phone-1` has no fixes. | Any fingerprint after revoke, including another device's. |
| TC-023 | FR-ENROL-014 | UC-ENROL-001 E4, UC-ENROL-002 E4, UC-ENROL-003 E3, UC-ENROL-004 E1 | integration (CLI) | Ports set to two loopback ports held open by the test; an empty database. | Run, in turn: `devices`; `device`; `device list`; `device add phone-1 "Uno"`; `device add phone-1 "Uno" FP_A extra`; `device set-fingerprint phone-1`; `device set-fingerprint phone-1 FP_A extra`; `device revoke`; `device revoke phone-1 extra`. | Each exits non-zero within 10 seconds without any signal; stderr contains `device add`, `device set-fingerprint` and `device revoke`; afterwards `device add phone-1 "Uno" FP_A` exits 0 (nothing was enrolled). | Unknown subcommand, missing subcommand, too few and too many arguments for each. |

## Server start and no enrolment over the network

| ID | FR | UC | Level | Given | When | Then | Corner cases |
| --- | --- | --- | --- | --- | --- | --- | --- |
| TC-024 | FR-ENROL-001 | UC-ENROL-004 | end-to-end | Two free loopback ports chosen by the test; `ATALAYA_DATABASE_PATH` points to a missing file. | Run the binary with no arguments. | `GET /healthz` on the device port and on the panel port both return `200`; the database file exists; after `SIGTERM` the process exits with code 0. | Missing database. |
| TC-025 | FR-ENROL-016 | UC-ENROL-004 A1 | integration (HTTP) | `phone-1` enrolled with `FP_A`. | On both routers, with header `FP_B` and body `{"id":"phone-9","name":"Nueve","fingerprint":"bbbb…"}` (64 × `b`): `POST`, `PUT`, `PATCH`, `DELETE` on `/devices`, `/devices/phone-9`, `/devices/phone-1`, `/devices/phone-1/fingerprint`, `/enrol`, `/admin/devices`; `PUT` and `DELETE` on `/telemetry/phone-1`. | No response is 2xx (each is `404` or `405`); afterwards `POST /telemetry/phone-1` with `FP_A` gets `201`, with `FP_B` gets `401`, and `POST /telemetry/phone-9` with `FP_B` gets `401`. | Any method, guessed paths, both listeners. |
| TC-026 | FR-ENROL-016 | UC-ENROL-004 | integration (HTTP) | An empty database. | `GET /healthz` and `POST /telemetry/phone-1` (no header) on the device router; `GET /healthz` and `POST /telemetry/phone-1` on the panel router. | Device router: `200` and `401`. Panel router: `200` and `404`. | Route set of each listener. |

## Telemetry intake

| ID | FR | UC | Level | Given | When | Then | Corner cases |
| --- | --- | --- | --- | --- | --- | --- | --- |
| TC-027 | FR-TELEMETRY-001, FR-TELEMETRY-002 | UC-TELEMETRY-001 | integration (HTTP) | `phone-1` enrolled with `FP_A`, `phone-2` with `FP_B`, neither with fixes. | `POST /telemetry/phone-1` with header `FP_A` and `VALID`, between `t0` and `t1`. | `201 Created`; `phone-1` has exactly one fix with `lat` 40.0, `lon` -3.0, `acc` 12.5, `ts` 1700000000000, `prov` `fused`, and a reception time in `[t0, t1]`; `phone-1`'s last contact is in `[t0, t1]`; `phone-2` has no fixes and no last contact. | Device time `ts` is kept apart from reception time. |
| TC-028 | FR-TELEMETRY-001 | UC-TELEMETRY-001 | integration (HTTP) | `phone-1` enrolled with `FP_A`. | Four requests with header `FP_A`, each with one boundary fix: (`lat` -90, `lon` -180, `acc` 0, `ts` 1, `gps`), (`lat` 90, `lon` 180, `acc` 0.0, `ts` 1700000000000, `network`), (`lat` 0, `lon` 0, `acc` 5, `ts` 1700000000001, `fused`), integers instead of decimals for `lat`/`lon`. | Each gets `201`; `phone-1` has as many fixes as requests, and the one with `lat` -90 has `lon` -180, `acc` 0, `ts` 1, `prov` `gps`. | Inclusive bounds, `acc` 0, smallest `ts`, every `prov`. |
| TC-029 | FR-TELEMETRY-001 | UC-TELEMETRY-001 A1 | integration (HTTP) | `phone-1` enrolled with `FP_A`. | `POST /telemetry/phone-1` with `FP_A` and `{"battery":{"level":"oops"},"network":[1,2],"location":{"lat":40.0,"lon":-3.0,"acc":12.5,"ts":1700000000000,"prov":"fused","alt":650,"speed":"x"}}`. | `201`; one fix stored with `lat` 40.0 and `prov` `fused`. | Extra and malformed sections are ignored, not validated. |
| TC-030 | FR-TELEMETRY-001, FR-TELEMETRY-002 | UC-TELEMETRY-001 | integration (HTTP) | `phone-1` enrolled with `FP_A`; one request with `VALID` already accepted, last contact `L1`. | A second identical request, sent at least 5 ms after `L1`, between `t0` and `t1`. | `201`; `phone-1` has two fixes, both with `ts` 1700000000000; last contact is in `[t0, t1]` and greater than `L1`. | Repeated identical fix (no deduplication in this change). |
| TC-031 | FR-TELEMETRY-001 | UC-TELEMETRY-001 | integration (HTTP) | `phone-1` enrolled with `FP_A`. | 10 requests with `FP_A` and `VALID` sent concurrently. | All get `201`; `phone-1` has exactly 10 fixes. | Concurrency. |
| TC-032 | FR-TELEMETRY-001 | UC-TELEMETRY-001 | integration (HTTP) | `ATALAYA_FINGERPRINT_HEADER=x-test-fingerprint` in the config the router is built from; `phone-1` enrolled with `FP_A`. | `POST /telemetry/phone-1` with `VALID`, once with `x-test-fingerprint: FP_A`, once with `x-client-cert-sha256: FP_A`. | First `201`, second `401`. | Configured header name is the only one read. |
| TC-033 | FR-TELEMETRY-003, FR-TELEMETRY-006 | UC-TELEMETRY-001 E1 | integration (HTTP) | `phone-1` with `FP_A` and one accepted fix (last contact `L1`); `phone-2` with `FP_B` and one accepted fix (last contact `L2`); `phone-3` enrolled with `FP_C` and revoked. | With `VALID`: (a) `/telemetry/phone-1` without header; (b) `/telemetry/phone-1` with `FP_F`; (c) `/telemetry/phone-1` with `FP_B`; (d) `/telemetry/phone-9` with `FP_A`; (e) `/telemetry/phone-3` with `FP_C`. | All five get `401 Unauthorized` with byte-identical bodies; `phone-1` and `phone-2` still have one fix each, `phone-3` none; last contacts are still `L1`, `L2` and none. | The five identity failures look the same. |
| TC-034 | FR-TELEMETRY-003 | UC-TELEMETRY-001 E1 | integration (HTTP) | `phone-1` enrolled with `FP_A`. | `POST /telemetry/phone-1` with `VALID` and header: empty value; `FP_A_UPPER`; `FP_A` with a trailing space; `FP_A` with a leading `sha256:`; 32 pairs `aa` joined by `:`. | All get `401`; `phone-1` has no fixes. | The header must be exactly the registered lowercase fingerprint. |
| TC-035 | FR-TELEMETRY-004 | UC-TELEMETRY-001 E1 | integration (HTTP) | `phone-1` enrolled with `FP_A`. | `POST /telemetry/phone-1` with body `not json`, once without header and once with `FP_F`; `POST /telemetry/phone-9` with `FP_A` and `{}`. | All get `401`, not `400`; no fixes stored. | Identity checked before the body. |
| TC-036 | FR-TELEMETRY-005, FR-TELEMETRY-006 | UC-TELEMETRY-001 E2 | integration (HTTP) | `phone-1` enrolled with `FP_A`, one accepted fix, last contact `L1`. | With header `FP_A`, each body in turn: `not json`; empty; `{}`; `{"location":null}`; `{"location":[]}`; `VALID` with `lat` 91; `lat` -90.5; `lon` 181; `lon` -181; `acc` -1; `ts` 0; `ts` -5; `ts` 1.5; `ts` `"1700000000000"`; `lat` `"40.0"`; `prov` `wifi`; `prov` `GPS`; `prov` `""`; and `VALID` without each of the five fields. | Each gets `400 Bad Request`; `phone-1` still has exactly one fix; last contact is still `L1`. | Non-JSON, empty, missing section, null, wrong types, each bound, each missing field, `prov` case. |
| TC-037 | FR-TELEMETRY-008, FR-TELEMETRY-006, FR-ENROL-016 | UC-TELEMETRY-002 | integration (HTTP) | `phone-1` enrolled with `FP_A`, no fixes, no last contact. | On the panel router: `POST /telemetry/phone-1` with header `FP_A` and `VALID`; the same without header; the same with header `FP_A` and `not json`. | All get `404 Not Found`; `phone-1` has no fixes and no last contact. | With and without header, invalid body. |

## Operator flows end to end

| ID | FR | UC | Level | Given | When | Then | Corner cases |
| --- | --- | --- | --- | --- | --- | --- | --- |
| TC-038 | FR-ENROL-002, FR-ENROL-007, FR-TELEMETRY-001, FR-TELEMETRY-002 | UC-ENROL-001, UC-ENROL-004, UC-TELEMETRY-001 | end-to-end | A server running on an empty database file, both listeners bound; `POST /telemetry/phone-1` with `FP_A` and `VALID` over TCP to the device listener gets `401`. | The CLI binary runs `device add phone-1 "Móvil de ejemplo" FP_A` on the same file; then the same request is sent again between `t0` and `t1`. | The CLI exits 0 with one stdout line containing `phone-1`; the second request gets `201` without restarting the server; `phone-1` has one fix with `lat` 40.0, `lon` -3.0, `acc` 12.5, `ts` 1700000000000, `prov` `fused`; its last contact is in `[t0, t1]`. | Enrolment applied to a running server. |
| TC-039 | FR-ENROL-003, FR-ENROL-005, FR-ENROL-007, FR-TELEMETRY-007 | UC-ENROL-002, UC-TELEMETRY-001 A2 | end-to-end | A running server; `phone-1` enrolled with `FP_A` and one accepted fix over TCP. | The CLI runs `device set-fingerprint phone-1 FP_B`; then `POST /telemetry/phone-1` with `VALID` and `FP_B`, then with `FP_A`. | The CLI exits 0 with one stdout line containing `phone-1`; the `FP_B` request gets `201`, the `FP_A` request gets `401`; `phone-1` has two fixes. | Replacement without restart; old fingerprint rejected. |
| TC-040 | FR-ENROL-004, FR-ENROL-006, FR-ENROL-007, FR-TELEMETRY-007 | UC-ENROL-003, UC-ENROL-002 A2, UC-TELEMETRY-001 | end-to-end | A running server; `phone-1` enrolled with `FP_A` and one accepted fix over TCP; last contact `L1`. | The CLI runs `device revoke phone-1`; `POST /telemetry/phone-1` with `FP_A` and `VALID`; then the CLI runs `device set-fingerprint phone-1 FP_B` and the request is sent with `FP_B`. | Revoke exits 0 with one stdout line containing `phone-1`; the `FP_A` request gets `401` and `phone-1` still has one fix with last contact `L1`; after `set-fingerprint` the `FP_B` request gets `201` and `phone-1` has two fixes. | Revocation without restart; fixes kept; reassignment. |
| TC-041 | FR-TELEMETRY-008, FR-ENROL-016 | UC-TELEMETRY-002 | end-to-end | A running server; `phone-1` enrolled with `FP_A`. | `POST /telemetry/phone-1` with header `FP_A` and `VALID` over TCP to the panel listener. | Status line `HTTP/1.1 404 Not Found`; `phone-1` has no fixes and no last contact. | Real panel listener. |

## Coverage

| Requirement | Cases |
| --- | --- |
| FR-ENROL-001 | TC-024 |
| FR-ENROL-002 | TC-001, TC-038 |
| FR-ENROL-003 | TC-017, TC-018, TC-039 |
| FR-ENROL-004 | TC-020, TC-021, TC-040 |
| FR-ENROL-005 | TC-039 |
| FR-ENROL-006 | TC-022, TC-040 |
| FR-ENROL-007 | TC-038, TC-039, TC-040 |
| FR-ENROL-008 | TC-002, TC-003 |
| FR-ENROL-009 | TC-004 |
| FR-ENROL-010 | TC-012 |
| FR-ENROL-011 | TC-011, TC-013, TC-014, TC-015, TC-016 |
| FR-ENROL-012 | TC-010 |
| FR-ENROL-013 | TC-005, TC-006, TC-007, TC-008, TC-009, TC-010, TC-011 |
| FR-ENROL-014 | TC-023 |
| FR-ENROL-015 | TC-019 |
| FR-ENROL-016 | TC-025, TC-026, TC-037, TC-041 |
| FR-TELEMETRY-001 | TC-027, TC-028, TC-029, TC-030, TC-031, TC-032, TC-038 |
| FR-TELEMETRY-002 | TC-027, TC-030, TC-038 |
| FR-TELEMETRY-003 | TC-022, TC-033, TC-034 |
| FR-TELEMETRY-004 | TC-035 |
| FR-TELEMETRY-005 | TC-036 |
| FR-TELEMETRY-006 | TC-033, TC-036, TC-037 |
| FR-TELEMETRY-007 | TC-039, TC-040 |
| FR-TELEMETRY-008 | TC-037, TC-041 |

| Use case | Cases |
| --- | --- |
| UC-ENROL-001 | TC-001–TC-016, TC-023, TC-038 |
| UC-ENROL-002 | TC-004, TC-009, TC-010, TC-014, TC-015, TC-017–TC-019, TC-023, TC-039, TC-040 |
| UC-ENROL-003 | TC-004, TC-010, TC-015, TC-020–TC-023, TC-040 |
| UC-ENROL-004 | TC-023–TC-026, TC-038 |
| UC-TELEMETRY-001 | TC-022, TC-027–TC-036, TC-038–TC-040 |
| UC-TELEMETRY-002 | TC-037, TC-041 |
