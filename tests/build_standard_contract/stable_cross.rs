//! The build standard retains Thysalion's stable Cross backend exception.

use super::helpers::cargo_config;

/// Stable Cargo 1.98.1 rejects either key even when the build selects release.
fn is_stable_cross_compatible(config: &toml::Value) -> bool {
    config
        .get("unstable")
        .and_then(|table| table.get("codegen-backend"))
        .is_none()
        && config
            .get("profile")
            .and_then(|profile| profile.get("dev"))
            .and_then(|dev| dev.get("codegen-backend"))
            .is_none()
}

/// The merged #43 exception keeps LLVM as the development backend while
/// `cross +stable` reads the same Cargo configuration for release builds.
#[test]
fn stable_cross_exception_rejects_both_codegen_backend_keys() {
    let config = cargo_config().expect("read Cargo development configuration");
    assert!(
        is_stable_cross_compatible(&config),
        "stable Cross rejects [unstable] or [profile.dev] codegen-backend; see #43"
    );

    let mut unstable = config.clone();
    unstable
        .as_table_mut()
        .expect("Cargo configuration is a table")
        .insert(
            "unstable".to_owned(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "codegen-backend".to_owned(),
                toml::Value::Boolean(true),
            )])),
        );
    assert!(
        !is_stable_cross_compatible(&unstable),
        "restoring [unstable] codegen-backend must fail the #43 contract"
    );

    let mut profile = config;
    profile
        .as_table_mut()
        .expect("Cargo configuration is a table")
        .insert(
            "profile".to_owned(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "dev".to_owned(),
                toml::Value::Table(toml::map::Map::from_iter([(
                    "codegen-backend".to_owned(),
                    toml::Value::String("cranelift".to_owned()),
                )])),
            )])),
        );
    assert!(
        !is_stable_cross_compatible(&profile),
        "restoring [profile.dev] codegen-backend must fail the #43 contract"
    );
}
