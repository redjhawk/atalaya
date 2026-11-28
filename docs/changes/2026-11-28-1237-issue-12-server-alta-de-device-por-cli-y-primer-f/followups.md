# Follow-ups of #12

Non-blocking review findings. Issues were opened only for those with a named consequence.

## R1-02 

server/tests/routes.rs:13-29 and 53-66 list the same malformed calls. Extract one shared helper (like `malformed_calls()` in server/tests/enrolment_cli.rs) and use it in both TC-025 tests.

## R1-03 

server/src/devices.rs:166-178: remove the defence-in-depth unique-violation branch and propagate the insert error with `?`, or map a unique violation straight to one error without the extra query.

## R1-04 

server/src/devices.rs:148-178, 203-211, 237-247: replace `exists(tx, sql, value)` with named helpers `id_taken(tx, id)` and `fingerprint_taken(tx, fingerprint, except_id)`, and use them in `add` and `set_fingerprint`.

## R1-05 

server/src/telemetry.rs:70: rename `Fix::validate` to `is_valid` (or `is_within_bounds`) and update the caller in `read_fix`.

## R1-06 

server/tests/health.rs:24: drop the local `fingerprint_header()` and use `mod common;` with `common::default_header()`. Check that pulling in the common module does not raise dead-code warnings under clippy -D warnings.

## R1-08 

server/tests/telemetry.rs:120-122: replace the `while now_ms() < l1 + 5 { yield_now }` loop with `tokio::time::sleep(Duration::from_millis(5)).await` (keep the comment explaining why).
