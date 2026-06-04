use std::sync::Arc;
use wilayah::{version, AdminLevel, Database};

#[test]
fn test_version() {
    assert_eq!(version(), "0.5.0");
}

#[test]
fn test_admin_level_display() {
    let level = AdminLevel {
        code: "31.71".into(),
        name: "Jakarta".into(),
    };
    assert_eq!(format!("{level}"), "31.71 Jakarta");
}

#[test]
fn test_error_display() {
    let db = Database::open().expect("open DB");
    let result = db.find_by_code("31.71.03.1001");
    assert!(result.is_ok());

    let err = match Database::open_with_polygons("/nonexistent/path/poly.db") {
        Ok(_) => panic!("expected error for invalid path"),
        Err(e) => e,
    };
    let msg = format!("{err}");
    assert!(!msg.is_empty(), "Error display should not be empty");
}

#[test]
fn test_error_source_chain() {
    let err = match Database::open_with_polygons("/nonexistent/path/poly.db") {
        Ok(_) => panic!("expected error for invalid path"),
        Err(e) => e,
    };
    assert!(
        std::error::Error::source(&err).is_some(),
        "db::Error should have a source"
    );
}

#[test]
fn test_error_serialize() {
    let err = match Database::open_with_polygons("/nonexistent/path/poly.db") {
        Ok(_) => panic!("expected error for invalid path"),
        Err(e) => e,
    };
    let json = serde_json::to_string(&err).expect("serialize error");
    assert!(
        json.starts_with('"'),
        "serialized error should be a JSON string"
    );
}

#[test]
fn test_database_is_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<Database>();
    assert_sync::<Database>();
}

#[test]
fn test_database_concurrent_access() {
    let db = Arc::new(Database::open().expect("open DB"));
    let mut handles = Vec::new();

    for i in 0..4 {
        let db = Arc::clone(&db);
        handles.push(std::thread::spawn(move || {
            let lat = -6.0 + (i as f64) * 0.5;
            let lon = 106.0 + (i as f64) * 0.5;
            let results = db.find_nearest(lat, lon, 3).expect("query");
            assert!(!results.is_empty(), "thread {i} should get results");
        }));
    }

    for handle in handles {
        handle.join().expect("thread should not panic");
    }
}

#[cfg(feature = "build-db")]
mod pipeline_error {
    use wilayah::builder::PipelineError;

    #[test]
    fn test_pipeline_error_new() {
        let err = PipelineError::new("something failed");
        assert_eq!(format!("{err}"), "something failed");
    }

    #[test]
    fn test_pipeline_error_context() {
        let err = PipelineError::new("inner").context("outer");
        assert_eq!(format!("{err}"), "outer");
        assert!(
            std::error::Error::source(&err).is_some(),
            "context should preserve source"
        );
    }

    #[test]
    fn test_pipeline_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = PipelineError::from(io_err);
        let msg = format!("{err}");
        assert!(msg.contains("file missing"), "msg: {msg}");
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn test_pipeline_error_from_rusqlite() {
        let sqlite_err = rusqlite::Error::InvalidColumnIndex(99);
        let err = PipelineError::from(sqlite_err);
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn test_pipeline_error_from_json() {
        let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let err = PipelineError::from(json_err);
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn test_pipeline_error_source_chain() {
        let err = PipelineError::new("root").context("middle").context("top");
        assert_eq!(format!("{err}"), "top");
        let src = std::error::Error::source(&err).unwrap();
        assert_eq!(format!("{src}"), "middle");
        let src2 = std::error::Error::source(src).unwrap();
        assert_eq!(format!("{src2}"), "root");
        assert!(std::error::Error::source(src2).is_none());
    }
}
