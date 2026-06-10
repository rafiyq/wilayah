//! Shared utilities: HTTP fetching with retries and SHA-256 hashing.

use super::PipelineError;
use super::PipelineResultExt;

/// Fetch a URL with exponential-backoff retries, returning raw bytes.
pub(crate) fn fetch_url_with_retry(
    url: &str,
    max_retries: usize,
    timeout_secs: u64,
    label: &str,
) -> Result<Vec<u8>, PipelineError> {
    let mut last_err = String::new();
    for attempt in 0..=max_retries {
        match ureq::get(url)
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .call()
        {
            Ok(resp) => {
                let mut data = Vec::new();
                resp.into_reader()
                    .read_to_end(&mut data)
                    .ctx("failed to read response")?;
                return Ok(data);
            }
            Err(e) => {
                last_err = format!("{}", e);
                if attempt < max_retries {
                    let wait_secs = 2_u64.pow(attempt as u32);
                    eprintln!(
                        "{} attempt {} failed, retrying in {}s: {}",
                        label,
                        attempt + 1,
                        wait_secs,
                        last_err
                    );
                    std::thread::sleep(std::time::Duration::from_secs(wait_secs));
                }
            }
        }
    }
    Err(PipelineError::new(format!(
        "{} failed after {} retries: {}",
        label, max_retries, last_err
    )))
}

/// Fetch a URL with retries, returning a UTF-8 string.
pub(crate) fn fetch_with_retry(url: &str, max_retries: usize) -> Result<String, PipelineError> {
    let data = fetch_url_with_retry(url, max_retries, 60, "BIG API")?;
    String::from_utf8(data).ctx("BIG API response is not valid UTF-8")
}

/// Compute the SHA-256 hash of raw bytes, returned as lowercase hex.
pub(crate) fn hash_sha256(data: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Compute the SHA-256 hash of a file, returned as lowercase hex.
pub(crate) fn hash_file(path: &std::path::Path) -> Result<String, PipelineError> {
    let data = std::fs::read(path).ctx("failed to read file for SHA-256")?;
    Ok(hash_sha256(&data))
}
