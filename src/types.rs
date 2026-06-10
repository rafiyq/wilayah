use core::fmt;
use serde::Serialize;

/// Maximum number of results returned by [`Database::find_nearest`](crate::Database::find_nearest).
pub const NEAREST_MAX_LIMIT: usize = 20;

/// Maximum number of results returned by [`Database::find_by_name`](crate::Database::find_by_name).
pub const SEARCH_MAX_LIMIT: usize = 100;

/// Maximum number of results per page returned by [`Database::find_by_code_prefix`](crate::Database::find_by_code_prefix).
pub const CODE_PREFIX_MAX_LIMIT: usize = 1000;

/// Metadata about the embedded location database.
///
/// Returned by [`data_info()`](crate::data_info) and
/// [`Database::data_info()`](crate::Database::data_info). Contains information
/// about the data source, the government decree it's based on, the number of
/// villages, and when the database was built.
///
/// Metadata is read from the `db_meta` table embedded in the database itself,
/// so it is always correct regardless of how the binary was built (pipeline mode
/// or download mode).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DataInfo {
    /// The upstream data source (e.g., `"official"` or `"release"`).
    pub source: String,
    /// The government decree this data is based on
    /// (e.g., `"Kepmendagri No 300.2.2-2138 Tahun 2025"`).
    pub decree: String,
    /// The number of villages in the database.
    pub village_count: u32,
    /// Unix timestamp (seconds since epoch) of when this database was built.
    pub build_date: u64,
}

/// A village record with administrative hierarchy and coordinates.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Village {
    /// BMKG-compatible administrative code (e.g., `31.71.03.1001`)
    pub code: String,
    /// Village (desa/kelurahan) name
    pub name: String,
    /// District (kecamatan) name
    pub district: String,
    /// City/regency (kabupaten/kota) name
    pub city: String,
    /// Province name
    pub province: String,
    /// Latitude coordinate
    pub lat: f64,
    /// Longitude coordinate
    pub lon: f64,
    /// Distance from query point in kilometers.
    /// Only set by `find_nearest()`, always `None` from `find_by_name()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist_km: Option<f64>,
}

impl fmt::Display for Village {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} — {}, {}, {} ({})",
            self.name, self.district, self.city, self.province, self.code
        )
    }
}

/// Method used to determine the administrative location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LocateMethod {
    /// Matched by nearest village centroid (Haversine distance).
    Nearest,
    /// Matched by polygon containment — the query point falls within the
    /// village's administrative boundary polygon.
    Contained,
}

impl fmt::Display for LocateMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocateMethod::Nearest => write!(f, "nearest"),
            LocateMethod::Contained => write!(f, "contained"),
        }
    }
}

/// A single level of the administrative hierarchy with code and name.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AdminLevel {
    /// Administrative code for this level (e.g., `"31"`, `"31.71"`, `"31.71.03"`).
    pub code: String,
    /// Name of this administrative unit.
    pub name: String,
}

impl fmt::Display for AdminLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.code, self.name)
    }
}

/// Result of a reverse-geocode lookup showing the full administrative hierarchy.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Location {
    /// Province level (code + name).
    pub province: AdminLevel,
    /// City/regency (kabupaten/kota) level.
    pub city: AdminLevel,
    /// District (kecamatan) level.
    pub district: AdminLevel,
    /// Village (desa/kelurahan) name.
    pub village: String,
    /// Village administrative code (e.g., `"31.71.03.1001"`).
    pub village_code: String,
    /// Latitude of the matched village centroid.
    pub lat: f64,
    /// Longitude of the matched village centroid.
    pub lon: f64,
    /// Distance in km from the query point to the matched village centroid.
    pub dist_km: f64,
    /// Method used to determine this location.
    pub method: LocateMethod,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.province)?;
        writeln!(f, "  {}", self.city)?;
        writeln!(f, "  {}", self.district)?;
        writeln!(
            f,
            "  {} {} ({:.1} km, {})",
            self.village_code, self.village, self.dist_km, self.method
        )
    }
}

/// Build a [`Location`] from a village record by parsing its administrative code.
///
/// Splits the `code` field (e.g., `"31.71.03.1001"`) into province, city, and
/// district codes, and combines them with the village's administrative names.
/// Returns `None` if the code doesn't have exactly 4 dot-separated parts.
pub fn location_from_village(v: &Village, dist_km: f64, method: LocateMethod) -> Option<Location> {
    let parts: Vec<&str> = v.code.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    Some(Location {
        province: AdminLevel {
            code: parts[0].to_string(),
            name: v.province.clone(),
        },
        city: AdminLevel {
            code: format!("{}.{}", parts[0], parts[1]),
            name: v.city.clone(),
        },
        district: AdminLevel {
            code: format!("{}.{}.{}", parts[0], parts[1], parts[2]),
            name: v.district.clone(),
        },
        village: v.name.clone(),
        village_code: v.code.clone(),
        lat: v.lat,
        lon: v.lon,
        dist_km,
        method,
    })
}

/// Result of an unambiguous name lookup.
///
/// Implements [`fmt::Display`] for friendly CLI output:
///
/// ```ignore
/// match result {
///     LookupResult::Found(v) => println!("{v}"),
///     LookupResult::Ambiguous(list) => println!("{result}"),
///     LookupResult::NotFound => eprintln!("{result}"),
/// }
/// ```
#[derive(Debug, Clone, Serialize)]
pub enum LookupResult {
    /// Exactly one match
    Found(Village),
    /// Multiple matches found
    Ambiguous(Vec<Village>),
    /// No matches
    NotFound,
}

impl fmt::Display for LookupResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LookupResult::Found(v) => write!(f, "{}", v),
            LookupResult::Ambiguous(list) => {
                writeln!(f, "Found {} matching villages:", list.len())?;
                for (i, v) in list.iter().enumerate() {
                    writeln!(f, "  {}. {}", i + 1, v)?;
                }
                write!(
                    f,
                    "Use a more specific query (e.g., include city or province)"
                )
            }
            LookupResult::NotFound => write!(f, "No matching village found"),
        }
    }
}

/// Paginated result from a code prefix lookup.
#[derive(Debug, Clone, Serialize)]
pub struct PrefixResult {
    /// The villages in this page of results.
    pub villages: Vec<Village>,
    /// Total number of villages matching the prefix.
    pub total: usize,
    /// Whether more results exist beyond this page.
    pub has_more: bool,
}

impl fmt::Display for PrefixResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} result(s), total: {}, has_more: {}",
            self.villages.len(),
            self.total,
            self.has_more,
        )
    }
}
