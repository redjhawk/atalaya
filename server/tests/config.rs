use std::path::PathBuf;

use atalaya_server::{Config, ConfigError};

fn lookup<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.to_string())
    }
}

#[test]
fn defaults_apply_when_nothing_is_set() {
    let config = Config::from_lookup(lookup(&[])).unwrap();

    assert_eq!(config.device_port, 8080);
    assert_eq!(config.panel_port, 8081);
    assert_eq!(config.database_path, PathBuf::from("atalaya.db"));
    assert_eq!(config.fingerprint_header, "x-client-cert-sha256");
}

#[test]
fn every_setting_can_be_overridden() {
    let config = Config::from_lookup(lookup(&[
        ("ATALAYA_DEVICE_PORT", "9000"),
        ("ATALAYA_PANEL_PORT", "9001"),
        ("ATALAYA_DATABASE_PATH", "/var/lib/atalaya/data.db"),
        ("ATALAYA_FINGERPRINT_HEADER", "Cf-Cert-Fingerprint-Sha256"),
    ]))
    .unwrap();

    assert_eq!(config.device_port, 9000);
    assert_eq!(config.panel_port, 9001);
    assert_eq!(
        config.database_path,
        PathBuf::from("/var/lib/atalaya/data.db")
    );
    assert_eq!(config.fingerprint_header, "cf-cert-fingerprint-sha256");
}

#[test]
fn a_port_that_is_not_a_number_is_rejected() {
    let error = Config::from_lookup(lookup(&[("ATALAYA_PANEL_PORT", "http")])).unwrap_err();

    assert_eq!(
        error,
        ConfigError::InvalidPort {
            variable: "ATALAYA_PANEL_PORT",
            value: "http".into()
        }
    );
}

#[test]
fn an_invalid_header_name_is_rejected() {
    let error =
        Config::from_lookup(lookup(&[("ATALAYA_FINGERPRINT_HEADER", "bad header")])).unwrap_err();

    assert_eq!(
        error,
        ConfigError::InvalidHeaderName {
            value: "bad header".into()
        }
    );
}

#[test]
fn the_device_api_and_the_panel_cannot_share_a_port() {
    let error = Config::from_lookup(lookup(&[
        ("ATALAYA_DEVICE_PORT", "9000"),
        ("ATALAYA_PANEL_PORT", "9000"),
    ]))
    .unwrap_err();

    assert_eq!(error, ConfigError::SharedPort { port: 9000 });
}
