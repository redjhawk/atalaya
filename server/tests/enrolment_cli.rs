//! The enrolment subcommands of the `atalaya-server` binary, run as a child
//! process on a temporary database file.

mod common;

use atalaya_server::{device_router, open_database};
use axum::http::StatusCode;
use common::*;
use tempfile::TempDir;

/// TC-001
#[tokio::test]
async fn tc_001_add_enrols_a_device_without_fixes_or_last_contact() {
    let db = Db::migrated().await;

    let outcome = db.cli().run(&["device", "add", "phone-1", NAME, FP_A]);

    assert!(outcome.succeeded(), "{outcome:?}");
    assert!(outcome.confirms("phone-1"), "{outcome:?}");
    assert_eq!(
        read_devices(&db.path).await,
        vec![DeviceRow {
            id: "phone-1".into(),
            name: NAME.into(),
            fingerprint: Some(FP_A.into()),
            last_contact: None,
        }]
    );
    assert_eq!(count_all_fixes(&db.path).await, 0);
}

/// TC-002
#[tokio::test]
async fn tc_002_add_creates_and_migrates_a_missing_database() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("atalaya.db");

    let outcome = Cli::database(&path).run(&["device", "add", "phone-1", NAME, FP_A]);

    assert!(outcome.succeeded(), "{outcome:?}");
    assert!(path.exists());
    let router = device_router(open_database(&path).await.unwrap(), default_header());
    assert_eq!(
        post_fix(&router, "phone-1", Some(FP_A), VALID).await,
        StatusCode::CREATED
    );
}

/// TC-003
#[tokio::test]
async fn tc_003_add_uses_the_default_database_path_of_the_server() {
    let dir = TempDir::new().unwrap();

    let outcome = Cli::new()
        .current_dir(dir.path())
        .run(&["device", "add", "phone-1", NAME, FP_A]);

    assert!(outcome.succeeded(), "{outcome:?}");
    assert!(dir.path().join("atalaya.db").exists());
}

/// TC-004
#[tokio::test]
async fn tc_004_subcommands_exit_without_opening_a_listener() {
    let db = Db::migrated().await;
    let ports = HeldPorts::new();
    let cli = ports.apply(db.cli());
    cli.enrol("phone-1", NAME, FP_A);

    let failures: Vec<_> = [
        vec!["device", "add", "phone-2", "Otro", FP_B],
        vec!["device", "set-fingerprint", "phone-2", FP_C],
        vec!["device", "revoke", "phone-2"],
    ]
    .into_iter()
    .map(|args| {
        let outcome = cli.run(&args);
        (args, outcome)
    })
    .filter(|(_, outcome)| !outcome.succeeded())
    .collect();

    assert!(failures.is_empty(), "{failures:#?}");
}

/// TC-005
#[tokio::test]
async fn tc_005_ids_at_the_boundaries_are_accepted() {
    let db = Db::migrated().await;
    let long_id = "a".repeat(64);
    let cases = [
        ("a", hex64('1')),
        ("0", hex64('2')),
        ("1phone", hex64('3')),
        (long_id.as_str(), hex64('4')),
    ];

    let failures: Vec<_> = cases
        .iter()
        .map(|(id, fp)| (*id, db.cli().run(&["device", "add", id, NAME, fp])))
        .filter(|(_, outcome)| !outcome.succeeded())
        .collect();

    assert!(failures.is_empty(), "{failures:#?}");
}

/// TC-006
#[tokio::test]
async fn tc_006_invalid_ids_are_rejected_without_enrolling() {
    let db = Db::migrated().await;
    let too_long = "a".repeat(65);
    let ids = [
        "",
        too_long.as_str(),
        "Phone-1",
        "-phone",
        "phone_1",
        "phone 1",
        "teléfono",
    ];

    let wrong: Vec<_> = ids
        .iter()
        .map(|id| (*id, db.cli().run(&["device", "add", id, NAME, FP_A])))
        .filter(|(_, outcome)| !(outcome.failed() && outcome.stderr.contains("invalid device id")))
        .collect();

    assert!(wrong.is_empty(), "{wrong:#?}");
    let after = db.cli().run(&["device", "add", "phone-1", NAME, FP_A]);
    assert!(after.succeeded(), "{after:?}");
}

/// TC-007
#[tokio::test]
async fn tc_007_names_at_the_boundaries_are_accepted_counting_characters() {
    let db = Db::migrated().await;
    let longest = "ó".repeat(100);
    let cases = [
        ("n1", "x", hex64('1')),
        ("n2", longest.as_str(), hex64('2')),
        ("n3", "  Móvil  ", hex64('3')),
    ];

    let failures: Vec<_> = cases
        .iter()
        .map(|(id, name, fp)| (*id, db.cli().run(&["device", "add", id, name, fp])))
        .filter(|(_, outcome)| !outcome.succeeded())
        .collect();

    assert!(failures.is_empty(), "{failures:#?}");
}

/// TC-008
#[tokio::test]
async fn tc_008_invalid_names_are_rejected_without_enrolling() {
    let db = Db::migrated().await;
    let too_long = "a".repeat(101);
    let names = ["", "   ", too_long.as_str()];

    let wrong: Vec<_> = names
        .iter()
        .map(|name| {
            (
                *name,
                db.cli().run(&["device", "add", "phone-1", name, FP_A]),
            )
        })
        .filter(|(_, outcome)| !(outcome.failed() && outcome.stderr.contains("invalid name")))
        .collect();

    assert!(wrong.is_empty(), "{wrong:#?}");
    let after = db.cli().run(&["device", "add", "phone-1", NAME, FP_A]);
    assert!(after.succeeded(), "{after:?}");
}

fn invalid_fingerprints() -> Vec<String> {
    vec![
        "b".repeat(63),
        "b".repeat(65),
        String::new(),
        format!("{}g", "b".repeat(63)),
        vec!["bb"; 32].join(":"),
        format!("{} {}", "b".repeat(32), "b".repeat(31)),
    ]
}

/// TC-009
#[tokio::test]
async fn tc_009_invalid_fingerprints_are_rejected_by_add_and_set_fingerprint() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);

    let wrong: Vec<_> = invalid_fingerprints()
        .iter()
        .flat_map(|fp| {
            [
                db.cli().run(&["device", "add", "phone-2", "Otro", fp]),
                db.cli().run(&["device", "set-fingerprint", "phone-1", fp]),
            ]
            .map(|outcome| (fp.clone(), outcome))
        })
        .filter(|(_, outcome)| {
            !(outcome.failed() && outcome.stderr.contains("invalid fingerprint"))
        })
        .collect();

    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// TC-009
#[tokio::test]
async fn tc_009_invalid_fingerprints_change_nothing() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);

    for fp in invalid_fingerprints() {
        db.cli().run(&["device", "add", "phone-2", "Otro", &fp]);
        db.cli().run(&["device", "set-fingerprint", "phone-1", &fp]);
    }

    let add = db.cli().run(&["device", "add", "phone-2", "Otro", FP_B]);
    assert!(add.succeeded(), "{add:?}");
    assert_eq!(
        post_fix(&db.device_router(), "phone-1", Some(FP_A), VALID).await,
        StatusCode::CREATED
    );
}

/// TC-010
#[tokio::test]
async fn tc_010_unknown_ids_are_rejected_naming_the_id() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);

    let wrong: Vec<_> = [
        vec!["device", "set-fingerprint", "phone-9", FP_B],
        vec!["device", "revoke", "phone-9"],
    ]
    .into_iter()
    .map(|args| {
        let outcome = db.cli().run(&args);
        (args, outcome)
    })
    .filter(|(_, outcome)| !(outcome.failed() && outcome.stderr.contains("phone-9")))
    .collect();

    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// TC-010
#[tokio::test]
async fn tc_010_invalid_ids_are_rejected_by_set_fingerprint_and_revoke() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);

    let wrong: Vec<_> = [
        vec!["device", "revoke", "Phone-1"],
        vec!["device", "set-fingerprint", "-x", FP_B],
    ]
    .into_iter()
    .map(|args| {
        let outcome = db.cli().run(&args);
        (args, outcome)
    })
    .filter(|(_, outcome)| !(outcome.failed() && outcome.stderr.contains("invalid device id")))
    .collect();

    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// TC-010
#[tokio::test]
async fn tc_010_rejected_calls_on_unknown_or_invalid_ids_change_nothing() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);

    db.cli()
        .run(&["device", "set-fingerprint", "phone-9", FP_B]);
    db.cli().run(&["device", "revoke", "phone-9"]);
    db.cli().run(&["device", "revoke", "Phone-1"]);
    db.cli().run(&["device", "set-fingerprint", "-x", FP_B]);

    assert_eq!(
        post_fix(&db.device_router(), "phone-1", Some(FP_A), VALID).await,
        StatusCode::CREATED
    );
    let add = db.cli().run(&["device", "add", "phone-9", "Nueve", FP_B]);
    assert!(add.succeeded(), "{add:?}");
}

/// TC-016
#[tokio::test]
async fn tc_016_concurrent_adds_of_one_fingerprint_enrol_exactly_one_device() {
    let db = Db::migrated().await;

    let first = db.cli().spawn(&["device", "add", "phone-1", "Uno", FP_A]);
    let second = db.cli().spawn(&["device", "add", "phone-2", "Dos", FP_A]);
    let outcomes = [first.finish(), second.finish()];

    let successes = outcomes.iter().filter(|o| o.succeeded()).count();
    let failures = outcomes.iter().filter(|o| o.failed()).count();
    assert_eq!((successes, failures), (1, 1), "{outcomes:#?}");
    let holders = read_devices(&db.path)
        .await
        .into_iter()
        .filter(|device| device.fingerprint.as_deref() == Some(FP_A))
        .count();
    assert_eq!(holders, 1);
}

/// TC-021
#[tokio::test]
async fn tc_021_revoking_a_revoked_device_succeeds() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    db.cli().revoke("phone-1");

    let outcome = db.cli().run(&["device", "revoke", "phone-1"]);

    assert!(outcome.succeeded(), "{outcome:?}");
    assert!(outcome.confirms("phone-1"), "{outcome:?}");
    assert_eq!(db.device("phone-1").await.unwrap().fingerprint, None);
}

fn malformed_calls() -> Vec<Vec<&'static str>> {
    vec![
        vec!["devices"],
        vec!["device"],
        vec!["device", "list"],
        vec!["device", "add", "phone-1", "Uno"],
        vec!["device", "add", "phone-1", "Uno", FP_A, "extra"],
        vec!["device", "set-fingerprint", "phone-1"],
        vec!["device", "set-fingerprint", "phone-1", FP_A, "extra"],
        vec!["device", "revoke"],
        vec!["device", "revoke", "phone-1", "extra"],
    ]
}

/// TC-023
#[tokio::test]
async fn tc_023_malformed_calls_print_the_usage_and_do_not_start_the_server() {
    let db = Db::migrated().await;
    let ports = HeldPorts::new();
    let cli = ports.apply(db.cli());

    let wrong: Vec<_> = malformed_calls()
        .into_iter()
        .map(|args| {
            let outcome = cli.run(&args);
            (args, outcome)
        })
        .filter(|(_, outcome)| {
            !(outcome.failed()
                && outcome.stderr.contains("device add")
                && outcome.stderr.contains("device set-fingerprint")
                && outcome.stderr.contains("device revoke"))
        })
        .collect();

    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// TC-023
#[tokio::test]
async fn tc_023_malformed_calls_enrol_nothing() {
    let db = Db::migrated().await;
    let ports = HeldPorts::new();
    let cli = ports.apply(db.cli());

    for args in malformed_calls() {
        cli.run(&args);
    }

    let after = cli.run(&["device", "add", "phone-1", "Uno", FP_A]);
    assert!(after.succeeded(), "{after:?}");
}
