//! The canonical coordinate model and reference-system tag.
//!
//! Design (locked): a **lean** [`Coordinate`] (position + optional height +
//! reference system) is what the geodetic math and the central
//! `convert` dispatch (a later release) operate on. Rich observation metadata
//! (accuracy, timestamp, raw source, parse confidence) lives separately in
//! [`crate::fix::Fix`], populated by the ingestion layer.

use core::fmt;
// Re-enabled with the items that use them (see ROADMAP.md):
// use core::str::FromStr;          // Coordinate: FromStr (text-parse milestone)
// use crate::error::{Error, Result}; // Coordinate::validate (angles-and-units milestone)

/// A coordinate reference system / datum tag used for runtime dispatch.
///
/// GCJ-02 and BD-09 are obfuscation transforms rather than true geodetic
/// datums, but are modeled here as reference systems so the central
/// `convert` dispatch (a later release) can dispatch over them uniformly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Crs {
    /// WGS-84 — the global GNSS reference and library default.
    Wgs84,
    /// GCJ-02 — "Mars" coordinates used by Chinese map providers.
    Gcj02,
    /// BD-09 — Baidu's additional obfuscation atop GCJ-02.
    Bd09,
    /// NAD27 — North American Datum 1927 (Clarke-1866 ellipsoid).
    Nad27,
    /// Tokyo datum (Bessel-1841 ellipsoid; legacy Japan / Korea).
    Tokyo,
    /// Pulkovo-1942 / SK-42 (Krasovsky-1940 ellipsoid).
    Pulkovo42,
    // These classic datums are reached natively via a 7-parameter Helmert
    // transform — see [`crate::geodesy::datum`]. NAD83, ETRS89, ITRF
    // realizations, national grids, and the full EPSG long tail are delegated
    // to the optional `proj` feature.
}

/// A height value, tagged by the surface it is measured from.
///
/// GNSS reports **ellipsoidal** height natively; humans expect **orthometric**
/// height (above the geoid / "sea level"). Converting between them requires a
/// geoid model — see the optional `geoid` feature.
///
/// Only these two surfaces are modeled; tidal datums (MSL, MLLW, …) are out of
/// scope.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Height {
    /// Meters above the reference ellipsoid.
    Ellipsoidal(f64),
    /// Meters above the geoid (mean sea level).
    Orthometric(f64),
}

/// The lean canonical coordinate: position, optional height, and its CRS.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Coordinate {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
    /// Optional height (ellipsoidal or orthometric).
    pub height: Option<Height>,
    /// The reference system the position is expressed in.
    pub crs: Crs,
}

impl Coordinate {
    /// Construct a WGS-84 coordinate from latitude/longitude in degrees.
    #[must_use]
    pub fn wgs84(lat: f64, lon: f64) -> Self {
        Self {
            lat,
            lon,
            height: None,
            crs: Crs::Wgs84,
        }
    }

    /// Construct a GCJ-02 ("Mars") coordinate from latitude/longitude in degrees.
    #[must_use]
    pub fn gcj02(lat: f64, lon: f64) -> Self {
        Self {
            lat,
            lon,
            height: None,
            crs: Crs::Gcj02,
        }
    }

    /// Construct a BD-09 (Baidu) coordinate from latitude/longitude in degrees.
    #[must_use]
    pub fn bd09(lat: f64, lon: f64) -> Self {
        Self {
            lat,
            lon,
            height: None,
            crs: Crs::Bd09,
        }
    }

    /// Construct a coordinate in an explicit reference system.
    #[must_use]
    pub fn new(lat: f64, lon: f64, crs: Crs) -> Self {
        Self {
            lat,
            lon,
            height: None,
            crs,
        }
    }

    /// Attach a height, returning the updated coordinate.
    #[must_use]
    pub fn with_height(self, height: Height) -> Self {
        Self {
            height: Some(height),
            ..self
        }
    }

    // --- Released with the angles-and-units milestone (see ROADMAP.md) ---
    /*
    /// Validate that latitude ∈ [-90, 90] and longitude ∈ [-180, 180].
    ///
    /// This is a pure range check; it does *not* flag suspicious-but-valid
    /// values such as "Null Island" — use [`is_null_island`](Self::is_null_island)
    /// for that.
    ///
    /// # Errors
    /// Returns [`crate::Error::OutOfRange`] when either component is invalid.
    pub fn validate(&self) -> Result<()> {
        todo!("range-check lat/lon; see angle::wrap_longitude / clamp_latitude")
    }

    /// Whether this is "Null Island" — latitude and longitude both ~0, the
    /// telltale of a missing or defaulted fix rather than a real position in
    /// the Gulf of Guinea.
    #[must_use]
    pub fn is_null_island(&self) -> bool {
        todo!("true when |lat| and |lon| are within a small epsilon of 0")
    }
    */
}

/// Shared read access to a latitude/longitude pair.
///
/// Implemented by [`Coordinate`] and by the per-datum newtypes so generic code
/// (formatters, geodesics) can operate over any positioned type.
pub trait LatLon {
    /// Latitude in decimal degrees.
    fn lat(&self) -> f64;
    /// Longitude in decimal degrees.
    fn lon(&self) -> f64;
}

impl LatLon for Coordinate {
    fn lat(&self) -> f64 {
        self.lat
    }
    fn lon(&self) -> f64 {
        self.lon
    }
}

impl fmt::Display for Crs {
    /// The short canonical name (e.g. `WGS84`, `GCJ-02`), as used in errors.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Crs::Wgs84 => "WGS84",
            Crs::Gcj02 => "GCJ-02",
            Crs::Bd09 => "BD-09",
            Crs::Nad27 => "NAD27",
            Crs::Tokyo => "Tokyo",
            Crs::Pulkovo42 => "Pulkovo-1942",
        };
        f.write_str(name)
    }
}

// --- FromStr: released with the text-parse milestone (see ROADMAP.md) ---
/*
impl FromStr for Coordinate {
    type Err = Error;

    /// Parse via [`parse_coordinate`](crate::parse::parse_coordinate) with
    /// default options, discarding the surrounding [`Fix`](crate::Fix)
    /// metadata. Use `parse::parse_coordinate` directly to keep provenance.
    ///
    /// # Errors
    /// Returns [`Error::Parse`] when the input cannot be interpreted.
    fn from_str(s: &str) -> Result<Self> {
        todo!("delegate to parse::parse_coordinate(s)?.coord")
    }
}
*/

// --- Display: released with the format milestone (see ROADMAP.md) ---
/*
impl fmt::Display for Coordinate {
    /// Render in decimal degrees with default precision. For other
    /// representations, symbols, or locale use [`format`](crate::format::format).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("default decimal-degrees rendering (infallible for DD)")
    }
}
*/
