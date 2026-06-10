//! PDF download and text extraction.

use super::util;
use super::PipelineError;
use super::PipelineResultExt;
use std::fs;
use std::path::{Path, PathBuf};

/// Result of downloading a file with its SHA-256 hash.
pub(crate) struct DownloadResult {
    pub(crate) data: Vec<u8>,
    pub(crate) sha256: String,
}

/// Download a file with retries and compute its SHA-256 hash.
pub(crate) fn download_with_sha256(url: &str) -> Result<DownloadResult, PipelineError> {
    let data = util::fetch_url_with_retry(url, 5, 300, "PDF download")?;
    let sha256 = util::hash_sha256(&data);
    Ok(DownloadResult { data, sha256 })
}

/// Ensure the Kemendagri PDF is downloaded to the cache directory.
pub(crate) fn ensure_pdf(pdf_url: &str, cache_dir: &Path) -> Result<PathBuf, PipelineError> {
    fs::create_dir_all(cache_dir).ctx("failed to create cache directory")?;
    let pdf_path = cache_dir.join("kemendagri.pdf");

    if !pdf_path.exists() {
        eprintln!("Downloading Kemendagri PDF (57 MB)...");
        let bytes = download_with_sha256(pdf_url)?;
        fs::write(&pdf_path, bytes.data).ctx("failed to write PDF")?;
        eprintln!("PDF SHA-256: {}", bytes.sha256);
    }

    Ok(pdf_path)
}

/// Extract text from a PDF file using the `pdftotext` command-line tool.
pub(crate) fn extract_text(pdf_path: &Path) -> Result<String, PipelineError> {
    eprintln!("Extracting text from PDF...");
    let output = std::process::Command::new("pdftotext")
        .arg("-layout")
        .arg(pdf_path)
        .arg("-")
        .output()
        .ctx("pdftotext failed")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PipelineError::new(format!(
            "pdftotext exited with status {}: {}",
            output.status, stderr
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
