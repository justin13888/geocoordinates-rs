//! The canonical coordinate model and reference-system tag.
//!
//! Design (locked): a **lean** [`Coordinate`] (position + optional height +
//! reference system) is what the geodetic math and the central
//! [`crate::convert`] dispatch operate on. Rich observation metadata
//! (accuracy, timestamp, raw source, parse confidence) lives separately in
//! [`crate::fix::Fix`], populated by the ingestion layer.

use crate::error::Result;

/// A coordinate reference system / datum tag used for runtime dispatch.
///
/// GCJ-02 and BD-09 are obfuscation transforms rather than true geodetic
/// datums, but are modeled here as reference systems so the central
/// [`crate::convert::convert`] can dispatch over them uniformly.
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
    // TODO(impl): NAD83, ETRS89, ITRF realizations, national datums, EPSG codes
    // (the long tail is delegated to the optional `proj` feature).
}

/// A height value, tagged by the surface it is measured from.
///
/// GNSS reports **ellipsoidal** height natively; humans expect **orthometric**
/// height (above the geoid / "sea level"). Converting between them requires a
/// geoid model — see the optional `geoid` feature.
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

    /// Validate that latitude ∈ [-90, 90] and longitude ∈ [-180, 180].
    ///
    /// # Errors
    /// Returns [`crate::Error::OutOfRange`] when either component is invalid.
    pub fn validate(&self) -> Result<()> {
        todo!("range-check lat/lon; see units::wrap_longitude / clamp_latitude")
    }
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
