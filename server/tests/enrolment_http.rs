//! The effect of the enrolment subcommands on the device API: devices are
//! enrolled with the CLI binary on the database file the routers use.

mod common;

use axum::http::StatusCode;
use common::*;

/// TC-011
#[tokio::test]
async fn tc_011_an_uppercase_fingerprint_is_stored_lowercase_and_matches_the_edge() {
    let db = Db::migrated().await;

    let outcome = db
        .cli()
        .run(&["device", "add", "phone-1", NAME, FP_A_UPPER]);

    assert!(outcome.succeeded(), "{outcome:?}");
    assert_eq!(
        db.device("phone-1").await.unwrap().fingerprint.as_deref(),
        Some(FP_A)
    );
    assert_eq!(
        post_fix(&db.device_router(), "phone-1", Some(FP_A), VALID).await,
        StatusCode::CREATED
    );
}

/// TC-011
#[tokio::test]
async fn tc_011_fingerprint_uniqueness_ignores_case() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A_UPPER);

    let outcome = db.cli().run(&["device", "add", "phone-2", "Otro", FP_A]);

    assert!(outcome.failed(), "{outcome:?}");
}

/// TC-011
#[tokio::test]
async fn tc_011_a_mixed_case_fingerprint_is_accepted() {
    let db = Db::migrated().await;

    let outcome = db
        .cli()
        .run(&["device", "add", "phone-1", NAME, &"Aa".repeat(32)]);

    assert!(outcome.succeeded(), "{outcome:?}");
    assert_eq!(
        post_fix(&db.device_router(), "phone-1", Some(FP_A), VALID).await,
        StatusCode::CREATED
    );
}

/// TC-012
#[tokio::test]
async fn tc_012_adding_an_existing_id_fails_and_keeps_the_device() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);

    let outcome = db
        .cli()
        .run(&["device", "add", "phone-1", "Otro nombre", FP_B]);

    assert!(outcome.failed(), "{outcome:?}");
    assert!(outcome.stderr.contains("phone-1"), "{outcome:?}");
    assert_eq!(db.device("phone-1").await.unwrap().name, NAME);
    let router = db.device_router();
    assert_eq!(
        post_fix(&router, "phone-1", Some(FP_B), VALID).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post_fix(&router, "phone-1", Some(FP_A), VALID).await,
        StatusCode::CREATED
    );
}

/// TC-013
#[tokio::test]
async fn tc_013_adding_with_a_fingerprint_of_another_device_fails() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);

    let outcome = db.cli().run(&["device", "add", "phone-2", "Otro", FP_A]);

    assert!(outcome.failed(), "{outcome:?}");
    assert!(outcome.stderr.contains("fingerprint"), "{outcome:?}");
    let retry = db.cli().run(&["device", "add", "phone-2", "Otro", FP_B]);
    assert!(retry.succeeded(), "{retry:?}");
}

/// TC-014
#[tokio::test]
async fn tc_014_replacing_with_a_fingerprint_of_another_device_fails() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    db.cli().enrol("phone-2", "Otro", FP_B);

    let outcome = db
        .cli()
        .run(&["device", "set-fingerprint", "phone-2", FP_A]);

    assert!(outcome.failed(), "{outcome:?}");
    assert!(outcome.stderr.contains("fingerprint"), "{outcome:?}");
    let router = db.device_router();
    assert_eq!(
        post_fix(&router, "phone-2", Some(FP_B), VALID).await,
        StatusCode::CREATED
    );
    assert_eq!(
        post_fix(&router, "phone-1", Some(FP_A), VALID).await,
        StatusCode::CREATED
    );
}

/// TC-015
#[tokio::test]
async fn tc_015_a_revoked_fingerprint_is_free_for_add_and_replace() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    db.cli().revoke("phone-1");
    db.cli().enrol("phone-3", "Tres", FP_C);

    let outcomes = [
        db.cli().run(&["device", "add", "phone-2", "Otro", FP_A]),
        db.cli().run(&["device", "revoke", "phone-2"]),
        db.cli()
            .run(&["device", "set-fingerprint", "phone-3", FP_A]),
    ];

    assert!(outcomes.iter().all(Outcome::succeeded), "{outcomes:#?}");
    let router = db.device_router();
    let statuses = [
        post_fix(&router, "phone-3", Some(FP_A), VALID).await,
        post_fix(&router, "phone-1", Some(FP_A), VALID).await,
        post_fix(&router, "phone-2", Some(FP_A), VALID).await,
    ];
    assert_eq!(
        statuses,
        [
            StatusCode::CREATED,
            StatusCode::UNAUTHORIZED,
            StatusCode::UNAUTHORIZED
        ]
    );
}

/// A device `phone-1` named `NAME` with `FP_A` and one accepted fix; returns
/// its last contact.
async fn enrolled_with_one_fix(db: &Db) -> Option<i64> {
    db.cli().enrol("phone-1", NAME, FP_A);
    let status = post_fix(&db.device_router(), "phone-1", Some(FP_A), VALID).await;
    assert_eq!(status, StatusCode::CREATED, "the first fix is accepted");
    let last_contact = db.last_contact("phone-1").await;
    assert!(
        last_contact.is_some(),
        "the first fix sets the last contact"
    );
    last_contact
}

/// TC-017
#[tokio::test]
async fn tc_017_replacing_the_fingerprint_keeps_name_fixes_and_last_contact() {
    let db = Db::migrated().await;
    let last_contact = enrolled_with_one_fix(&db).await;

    let outcome = db
        .cli()
        .run(&["device", "set-fingerprint", "phone-1", FP_B]);

    assert!(outcome.succeeded(), "{outcome:?}");
    assert!(outcome.confirms("phone-1"), "{outcome:?}");
    assert_eq!(
        db.device("phone-1").await,
        Some(DeviceRow {
            id: "phone-1".into(),
            name: NAME.into(),
            fingerprint: Some(FP_B.into()),
            last_contact,
        })
    );
    let lats: Vec<f64> = db.fixes("phone-1").await.iter().map(|f| f.lat).collect();
    assert_eq!(lats, vec![40.0]);
}

/// TC-018
#[tokio::test]
async fn tc_018_a_revoked_device_gets_a_new_fingerprint() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    db.cli().revoke("phone-1");

    let outcome = db
        .cli()
        .run(&["device", "set-fingerprint", "phone-1", FP_B]);

    assert!(outcome.succeeded(), "{outcome:?}");
    assert_eq!(
        post_fix(&db.device_router(), "phone-1", Some(FP_B), VALID).await,
        StatusCode::CREATED
    );
}

/// TC-019
#[tokio::test]
async fn tc_019_setting_the_same_fingerprint_in_any_case_changes_nothing() {
    let db = Db::migrated().await;
    let last_contact = enrolled_with_one_fix(&db).await;

    let outcomes = [
        db.cli()
            .run(&["device", "set-fingerprint", "phone-1", FP_A]),
        db.cli()
            .run(&["device", "set-fingerprint", "phone-1", FP_A_UPPER]),
    ];

    assert!(outcomes.iter().all(Outcome::succeeded), "{outcomes:#?}");
    assert_eq!(
        db.device("phone-1").await,
        Some(DeviceRow {
            id: "phone-1".into(),
            name: NAME.into(),
            fingerprint: Some(FP_A.into()),
            last_contact,
        })
    );
    assert_eq!(db.fixes("phone-1").await.len(), 1);
    assert_eq!(
        post_fix(&db.device_router(), "phone-1", Some(FP_A), VALID).await,
        StatusCode::CREATED
    );
}

/// TC-020
#[tokio::test]
async fn tc_020_revoking_keeps_name_fixes_and_last_contact() {
    let db = Db::migrated().await;
    let last_contact = enrolled_with_one_fix(&db).await;

    let outcome = db.cli().run(&["device", "revoke", "phone-1"]);

    assert!(outcome.succeeded(), "{outcome:?}");
    assert!(outcome.confirms("phone-1"), "{outcome:?}");
    assert_eq!(
        db.device("phone-1").await,
        Some(DeviceRow {
            id: "phone-1".into(),
            name: NAME.into(),
            fingerprint: None,
            last_contact,
        })
    );
    let lats: Vec<f64> = db.fixes("phone-1").await.iter().map(|f| f.lat).collect();
    assert_eq!(lats, vec![40.0]);
}

/// TC-022
#[tokio::test]
async fn tc_022_a_revoked_device_is_rejected_with_any_fingerprint() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    db.cli().revoke("phone-1");
    db.cli().enrol("phone-2", "Otro", FP_B);
    let router = db.device_router();

    let mut statuses = Vec::new();
    for fingerprint in [Some(FP_A), Some(FP_B), Some(FP_F), None] {
        statuses.push(post_fix(&router, "phone-1", fingerprint, VALID).await);
    }

    assert_eq!(statuses, vec![StatusCode::UNAUTHORIZED; 4]);
    assert!(db.fixes("phone-1").await.is_empty());
}
