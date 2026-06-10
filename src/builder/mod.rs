//! Build pipeline for constructing the `wilayah` location database.
//!
//! This module provides an end-to-end pipeline that:
//! 1. Downloads the official Kemendagri PDF listing all Indonesian villages
//! 2. Extracts and parses village records from the PDF text
//! 3. Fetches village polygon boundaries from the BIG ArcGIS API and computes centroids
//! 4. Merges the data, using kecamatan centroids as fallback for new villages
//! 5. Builds a SQLite database with RTree spatial index and FTS5 full-text search
//!
//! # Example
//!
//! ```no_run
//! use wilayah::builder::Pipeline;
//!
//! let output = Pipeline::new()
//!     .output(std::path::Path::new("data/locations.db"))
//!     .run()
//!     .expect("pipeline failed");
//!
//! println!("Built database with {} villages", output.village_count);
//! ```
//!
//! The pipeline is designed to be reproducible and transparent, sourcing data from
//! official government publications and APIs. The resulting database is embedded
//! into the `wilayah` crate at compile time via the build script.

mod big_api;
mod db_create;
mod parse;
mod pdf;
mod spatial;
mod util;

pub use parse::ParseOutputDetail;

use std::path::{Path, PathBuf};

/// The government decree number and year that the Kemendagri PDF data is based on.
pub const DATA_DECREE: &str = "Kepmendagri No 300.2.2-2138 Tahun 2025";

const PDF_URL: &str =
    "https://drive.google.com/uc?export=download&id=1o_m621D00TtwCwQMLn8XUnV3nolamPDm";
const BIG_API_URL: &str =
    "https://geoservices.big.go.id/gis/rest/services/BAPANAS/Batas_Administrasi/MapServer/2/query";

/// How to classify multi-ring polygon features.
///
/// BIG ArcGIS data can return features with multiple rings. Some rings are
/// separate outer boundaries (e.g., an island village spanning multiple
/// islands), while rare rings are holes (enclaves).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingClassification {
    /// Treat all rings as separate outer polygons (MultiPolygon).
    ///
    /// A point in an enclave would match both the surrounding village and
    /// the enclave village. This is correct for 99%+ of Indonesian village
    /// boundaries where holes are essentially nonexistent.
    SeparateRings,
    /// Use spatial containment to detect holes.
    ///
    /// Rings contained within an outer ring become interior rings (holes).
    /// A point inside a hole will NOT match the outer village. This is fully
    /// correct but adds ~50 lines of classification logic at build time.
    ClassifyHoles,
}

/// Error type returned when a pipeline step fails.
///
/// Contains a descriptive error message indicating what went wrong during
/// the pipeline execution (e.g., download failure, parsing error, database
/// creation failure).
pub struct PipelineError {
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl PipelineError {
    /// Creates a new `PipelineError` with the given message and no source.
    pub fn new(msg: impl Into<String>) -> Self {
        PipelineError {
            message: msg.into(),
            source: None,
        }
    }

    /// Wraps this error with additional context message, preserving the original as `source`.
    pub fn context(self, msg: impl Into<String>) -> Self {
        PipelineError {
            message: msg.into(),
            source: Some(Box::new(self)),
        }
    }
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::fmt::Debug for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(src) = &self.source {
            write!(f, "PipelineError({}, source: {})", self.message, src)
        } else {
            write!(f, "PipelineError({})", self.message)
        }
    }
}

impl std::error::Error for PipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}

trait PipelineResultExt<T> {
    fn ctx(self, msg: impl Into<String>) -> Result<T, PipelineError>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> PipelineResultExt<T> for Result<T, E> {
    fn ctx(self, msg: impl Into<String>) -> Result<T, PipelineError> {
        self.map_err(|e| PipelineError {
            message: msg.into(),
            source: Some(Box::new(e)),
        })
    }
}

impl From<std::io::Error> for PipelineError {
    fn from(e: std::io::Error) -> Self {
        PipelineError {
            message: e.to_string(),
            source: Some(Box::new(e)),
        }
    }
}

impl From<rusqlite::Error> for PipelineError {
    fn from(e: rusqlite::Error) -> Self {
        PipelineError {
            message: e.to_string(),
            source: Some(Box::new(e)),
        }
    }
}

impl From<serde_json::Error> for PipelineError {
    fn from(e: serde_json::Error) -> Self {
        PipelineError {
            message: e.to_string(),
            source: Some(Box::new(e)),
        }
    }
}

/// Output of a successful pipeline run.
pub struct PipelineOutput {
    /// Path to the built SQLite database file.
    pub db_path: PathBuf,
    /// Path to the built polygon database file, if `include_polygons(true)` was set.
    pub poly_db_path: Option<PathBuf>,
    /// Path to the saved parsed villages JSON, if `save_parsed_villages` was set.
    pub parsed_villages_path: Option<PathBuf>,
    /// Number of villages in the database.
    pub village_count: usize,
    /// SHA-256 hash of the database file, in hexadecimal.
    pub sha256: String,
}

/// Builder for configuring and running the database build pipeline.
///
/// The pipeline fetches data from official sources (Kemendagri PDF and BIG ArcGIS API),
/// merges and validates it, then constructs a SQLite database with RTree and FTS5
/// indexes. The resulting database is used by the `wilayah` crate at compile time.
pub struct Pipeline {
    pdf_url: String,
    big_api_url: String,
    cache_dir: PathBuf,
    output: PathBuf,
    decree: String,
    force_refresh_big: bool,
    ring_classification: RingClassification,
    include_polygons: bool,
    save_parsed_villages: Option<parse::ParseOutputDetail>,
}

impl Pipeline {
    /// Creates a new `Pipeline` with default configuration.
    ///
    /// Defaults:
    /// - PDF URL: Kemendagri official PDF from Google Drive
    /// - BIG API URL: `https://geoservices.big.go.id/...`
    /// - Cache directory: `data/cache` (relative to current working directory)
    /// - Output database: `data/locations.db`
    /// - Decree: `DATA_DECREE` constant
    /// - `force_refresh_big`: `false`
    ///
    /// To change any of these, use the builder methods (`pdf_url()`, `cache_dir()`,
    /// etc.) before calling `run()`.
    pub fn new() -> Self {
        Self {
            pdf_url: PDF_URL.to_string(),
            big_api_url: BIG_API_URL.to_string(),
            cache_dir: PathBuf::from("data/cache"),
            output: PathBuf::from("data/locations.db"),
            decree: DATA_DECREE.to_string(),
            force_refresh_big: false,
            ring_classification: RingClassification::SeparateRings,
            include_polygons: false,
            save_parsed_villages: None,
        }
    }

    /// Overrides the default Kemendagri PDF download URL.
    ///
    /// The URL should point to a PDF file containing the official village listing.
    /// The default is the Google Drive link used by the Ministry of Home Affairs.
    pub fn pdf_url(mut self, url: &str) -> Self {
        self.pdf_url = url.to_string();
        self
    }

    /// Overrides the default BIG (Badan Informasi Geospasial) ArcGIS API endpoint.
    ///
    /// The pipeline queries this service for village polygon boundaries and computes
    /// centroids. The default is the public BAPANAS service.
    pub fn big_api_url(mut self, url: &str) -> Self {
        self.big_api_url = url.to_string();
        self
    }

    /// Sets the directory where intermediate files are cached.
    ///
    /// This includes the downloaded PDF (`kemendagri.pdf`) and cached BIG data
    /// (`big_villages.json`). The directory is created if it does not exist.
    pub fn cache_dir(mut self, dir: &Path) -> Self {
        self.cache_dir = dir.to_path_buf();
        self
    }

    /// Sets the output path for the final SQLite database.
    ///
    /// This file will be overwritten if it already exists. The parent directory
    /// must be writable.
    pub fn output(mut self, path: &Path) -> Self {
        self.output = path.to_path_buf();
        self
    }

    /// Overrides the government decree string stored in the database metadata.
    ///
    /// This value is for informational purposes only and appears in `DataInfo`.
    /// The default is `DATA_DECREE`.
    pub fn decree(mut self, decree: &str) -> Self {
        self.decree = decree.to_string();
        self
    }

    /// Forces re-downloading BIG API data even if a cached copy exists.
    ///
    /// By default, the pipeline uses the cached `big_villages.json` if present.
    /// Set this to `true` to fetch fresh data from the API.
    pub fn force_refresh_big(mut self, yes: bool) -> Self {
        self.force_refresh_big = yes;
        self
    }

    /// Sets how multi-ring polygon features are classified.
    ///
    /// Defaults to [`RingClassification::SeparateRings`] — all rings are treated
    /// as separate outer polygons. Use [`RingClassification::ClassifyHoles`] to
    /// detect holes (enclaves) via spatial containment tests.
    pub fn ring_classification(mut self, mode: RingClassification) -> Self {
        self.ring_classification = mode;
        self
    }

    /// Enables building a separate polygon database alongside the main database.
    ///
    /// When `true`, the pipeline preserves raw polygon geometry from the BIG API
    /// and writes it to a `locations-poly.db` file (same directory as the output).
    /// This enables [`LocateMethod::Contained`](crate::types::LocateMethod::Contained)
    /// lookups via `Database::open_with_polygons()`.
    pub fn include_polygons(mut self, yes: bool) -> Self {
        self.include_polygons = yes;
        self
    }

    /// Enable saving parsed village records to a JSON file in the cache directory.
    ///
    /// The output detail level controls how much information is included:
    /// - `Minimal`: code + cleaned name + district + city + province
    /// - `WithRawName`: adds `raw_name` (original text before note stripping)
    /// - `Full`: adds `note_keyword` and `note_boundary` for parser auditing
    ///
    /// When set, the pipeline writes `parsed_villages.json` to the cache directory
    /// and includes its path in [`PipelineOutput::parsed_villages_path`].
    pub fn save_parsed_villages(mut self, detail: parse::ParseOutputDetail) -> Self {
        self.save_parsed_villages = Some(detail);
        self
    }

    /// Executes the full pipeline.
    ///
    /// Steps:
    /// 1. Ensure Kemendagri PDF is downloaded (cached if already present)
    /// 2. Extract text from PDF using `pdftotext`
    /// 3. Parse village records from the extracted text
    /// 4. Fetch BIG polygon data (cached or fresh with retries)
    /// 5. Merge villages with coordinates, using kecamatan centroids as fallback
    /// 6. Build the SQLite database with indexes and optimize
    /// 7. Compute SHA-256 of the final database
    ///
    /// Returns `PipelineOutput` on success, or `PipelineError` if any step fails.
    pub fn run(self) -> Result<PipelineOutput, PipelineError> {
        eprintln!("Starting pipeline...");

        let pdf_path = pdf::ensure_pdf(&self.pdf_url, &self.cache_dir)?;
        let text = pdf::extract_text(&pdf_path)?;
        let villages = parse::parse_villages(&text);

        let parsed_villages_path = if let Some(detail) = self.save_parsed_villages {
            let path = self.cache_dir.join("parsed_villages.json");
            parse::save_parsed_villages(&villages, detail, &path)?;
            Some(path)
        } else {
            None
        };

        let big_data = big_api::fetch_big_data(
            &self.big_api_url,
            &self.cache_dir,
            self.force_refresh_big,
            self.include_polygons,
        )?;
        let merged = db_create::merge_villages(&villages, &big_data);

        let build_date = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        db_create::build_db(&merged, &self.output, &self.decree, "official", build_date)?;

        let poly_db_path = if self.include_polygons {
            let poly_path = self.output.with_extension("poly.db");
            db_create::build_poly_db(&big_data, &poly_path, self.ring_classification)?;
            Some(poly_path)
        } else {
            None
        };

        let sha256 = db_create::compute_sha256(&self.output)?;

        let village_count = merged.len();

        eprintln!("Pipeline completed successfully.");
        Ok(PipelineOutput {
            db_path: self.output,
            poly_db_path,
            parsed_villages_path,
            village_count,
            sha256,
        })
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}
