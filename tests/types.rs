use wilayah::{version, AdminLevel};

#[test]
fn test_version() {
    assert_eq!(version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn test_admin_level_display() {
    let level = AdminLevel {
        code: "31.71".into(),
        name: "Jakarta".into(),
    };
    assert_eq!(format!("{level}"), "31.71 Jakarta");
}
