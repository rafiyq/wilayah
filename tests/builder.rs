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
