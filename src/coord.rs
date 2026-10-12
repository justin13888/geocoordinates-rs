//! The canonical coordinate model and reference-system tag.
//!
//! Design (locked): a **lean** [`Coordinate`] (position + optional height +
//! reference system) is what the geodetic math and the central
//! `convert` dispatch operate on. Rich observation metadata
//! (accuracy, timestamp, raw source, parse confidence) lives separately in
//! [`crate::fix::Fix`], populated by the ingestion layer.
//!
//! A [`Coordinate`] is valid by construction: its fields are private, and every
//! entry point — the constructors, [`Coordinate::with_height`], and serde
//! deserialization — rejects a non-finite or out-of-range value, so code that
//! holds one never re-checks it.

use core::fmt;

use crate::error::{Error, Result};

/// A coordinate reference system / datum tag used for runtime dispatch.
///
/// GCJ-02 and BD-09 are obfuscation transforms rather than true geodetic
/// datums, but are modeled here as reference systems so the central
/// `convert` dispatch can dispatch over them uniformly.
///
/// Exhaustive (no `#[non_exhaustive]`): the FFI mirror enumerates every variant,
/// so adding a datum here is a deliberate, compile-forcing change on both sides
/// (a wildcard fallback would silently mislabel an unknown datum as WGS-84).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    // to the deferred `proj` feature (see STABILIZATION.md).
}

/// A height value, tagged by the surface it is measured from.
///
/// GNSS reports **ellipsoidal** height natively; humans expect **orthometric**
/// height (above the geoid / "sea level"). Converting between them requires a
/// geoid model — deferred behind the `geoid` feature (see
/// [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md)).
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

impl Height {
    /// The height in meters, whichever surface it is measured from.
    pub(crate) fn meters(self) -> f64 {
        match self {
            Height::Ellipsoidal(value) | Height::Orthometric(value) => value,
        }
    }
}

/// The lean canonical coordinate: position, optional height, and its CRS.
///
/// Valid by construction: latitude is in `[-90, 90]`, longitude in
/// `[-180, 180]` (a closed range — `180` is accepted and not normalized), both
/// are finite, and any height is finite. The fields are private; build one with
/// [`new`](Self::new) (latitude first), [`from_lon_lat`](Self::from_lon_lat)
/// (longitude first), or a per-datum shorthand such as
/// [`wgs84`](Self::wgs84), and read it back with the getters.
///
/// `Coordinate` deliberately implements neither `Display` nor `FromStr`: a bare
/// decimal-degree string carries no datum or height, so a round trip through
/// one would silently relabel a GCJ-02 or BD-09 position as WGS-84. Render with
/// [`format::format`](crate::format::format) and parse with
/// [`parse::parse_coordinate`](crate::parse::parse_coordinate) instead:
///
/// ```compile_fail
/// let c = geocoordinates::Coordinate::wgs84(40.0, -74.0).unwrap();
/// let _ = c.to_string(); // no `Display`
/// ```
///
/// ```compile_fail
/// let _: geocoordinates::Coordinate = "40, -74".parse().unwrap(); // no `FromStr`
/// ```
///
/// Under the `serde` feature the JSON shape is
/// `{"lat":…,"lon":…,"height":…,"crs":…}`, and deserialization applies the same
/// checks as [`new`](Self::new) and [`with_height`](Self::with_height).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "CoordinateRepr"))]
pub struct Coordinate {
    lat: f64,
    lon: f64,
    height: Option<Height>,
    crs: Crs,
}

/// The unchecked wire shape of a [`Coordinate`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct CoordinateRepr {
    lat: f64,
    lon: f64,
    height: Option<Height>,
    crs: Crs,
}

#[cfg(feature = "serde")]
impl TryFrom<CoordinateRepr> for Coordinate {
    type Error = Error;

    fn try_from(repr: CoordinateRepr) -> Result<Self> {
        let coord = Self::new(repr.lat, repr.lon, repr.crs)?;
        match repr.height {
            Some(height) => coord.with_height(height),
            None => Ok(coord),
        }
    }
}

impl Coordinate {
    /// Construct a coordinate from latitude and longitude in decimal degrees
    /// (latitude first) in an explicit reference system, with no height.
    ///
    /// The range check is **closed** (longitude `180` is accepted) and does
    /// *not* normalize — wrap or clamp first with
    /// [`angle::wrap_longitude`](crate::angle::wrap_longitude) /
    /// [`clamp_latitude`](crate::angle::clamp_latitude) if the input may be
    /// out of range. Suspicious-but-valid values such as "Null Island" are
    /// accepted; see [`is_null_island`](Self::is_null_island).
    ///
    /// # Errors
    /// Returns [`Error::OutOfRange`] when latitude is outside `[-90, 90]`,
    /// longitude is outside `[-180, 180]`, or either is not finite.
    pub fn new(lat: f64, lon: f64, crs: Crs) -> Result<Self> {
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            return Err(Error::OutOfRange { lat, lon });
        }
        Ok(Self {
            lat,
            lon,
            height: None,
            crs,
        })
    }

    /// Construct a coordinate from longitude and latitude in decimal degrees
    /// (**longitude first**, the X,Y order of GeoJSON and WKT) in an explicit
    /// reference system.
    ///
    /// Identical to [`new`](Self::new) with the arguments swapped; it exists so
    /// lon-lat input is never passed positionally into a lat-lon signature.
    ///
    /// # Errors
    /// As [`new`](Self::new).
    pub fn from_lon_lat(lon: f64, lat: f64, crs: Crs) -> Result<Self> {
        Self::new(lat, lon, crs)
    }

    /// Construct a WGS-84 coordinate from latitude/longitude in degrees.
    ///
    /// # Errors
    /// As [`new`](Self::new).
    pub fn wgs84(lat: f64, lon: f64) -> Result<Self> {
        Self::new(lat, lon, Crs::Wgs84)
    }

    /// Construct a GCJ-02 ("Mars") coordinate from latitude/longitude in degrees.
    ///
    /// # Errors
    /// As [`new`](Self::new).
    pub fn gcj02(lat: f64, lon: f64) -> Result<Self> {
        Self::new(lat, lon, Crs::Gcj02)
    }

    /// Construct a BD-09 (Baidu) coordinate from latitude/longitude in degrees.
    ///
    /// # Errors
    /// As [`new`](Self::new).
    pub fn bd09(lat: f64, lon: f64) -> Result<Self> {
        Self::new(lat, lon, Crs::Bd09)
    }

    /// Build a coordinate from components the caller has already proven valid.
    ///
    /// For internal producers whose math guarantees the invariant (a validated
    /// input, a wrapped longitude). Debug builds assert it.
    pub(crate) fn from_valid(lat: f64, lon: f64, height: Option<Height>, crs: Crs) -> Self {
        debug_assert!(
            (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon),
            "Coordinate::from_valid out of range: lat={lat}, lon={lon}"
        );
        debug_assert!(
            height.is_none_or(|h| h.meters().is_finite()),
            "Coordinate::from_valid non-finite height: {height:?}"
        );
        Self {
            lat,
            lon,
            height,
            crs,
        }
    }

    /// Attach a height, returning the updated coordinate.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] (field `height`) when the height is not
    /// finite.
    pub fn with_height(self, height: Height) -> Result<Self> {
        if !height.meters().is_finite() {
            return Err(Error::InvalidValue {
                field: "height",
                detail: "must be finite".to_string(),
            });
        }
        Ok(Self {
            height: Some(height),
            ..self
        })
    }

    /// Latitude in decimal degrees, in `[-90, 90]`.
    #[must_use]
    pub fn lat(&self) -> f64 {
        self.lat
    }

    /// Longitude in decimal degrees, in `[-180, 180]`.
    #[must_use]
    pub fn lon(&self) -> f64 {
        self.lon
    }

    /// The reference system the position is expressed in.
    #[must_use]
    pub fn crs(&self) -> Crs {
        self.crs
    }

    /// The height (ellipsoidal or orthometric), if one is attached.
    #[must_use]
    pub fn height(&self) -> Option<Height> {
        self.height
    }

    /// Whether this is "Null Island" — latitude and longitude both ~0, the
    /// telltale of a missing or defaulted fix rather than a real position in
    /// the Gulf of Guinea.
    ///
    /// True when both components are within `1e-6`° (~0.11 m) of zero.
    #[must_use]
    pub fn is_null_island(&self) -> bool {
        self.lat.abs() <= NULL_ISLAND_EPS_DEG && self.lon.abs() <= NULL_ISLAND_EPS_DEG
    }
}

/// Half-width of the "Null Island" detection window, in degrees (~0.11 m).
/// Tight enough to exclude any genuine position, loose enough to absorb float
/// noise in a zeroed fix.
const NULL_ISLAND_EPS_DEG: f64 = 1e-6;

/// The private supertrait that seals [`LatLon`].
pub(crate) mod sealed {
    /// Implemented only inside this crate, so no downstream type can claim
    /// [`LatLon`](super::LatLon) and bypass the validated constructors.
    pub trait Sealed {}
}

/// Shared read access to a latitude/longitude pair.
///
/// Implemented by [`Coordinate`] and by the per-datum newtypes so generic code
/// (formatters, geodesics) can operate over any positioned type. The trait is
/// **sealed**: only this crate's types implement it, so a function taking
/// `&impl LatLon` only ever sees one of them.
pub trait LatLon: sealed::Sealed {
    /// Latitude in decimal degrees.
    #[must_use]
    fn lat(&self) -> f64;
    /// Longitude in decimal degrees.
    #[must_use]
    fn lon(&self) -> f64;
    /// Reference system of the position.
    #[must_use]
    fn crs(&self) -> Crs;
}

impl sealed::Sealed for Coordinate {}

/// Sugar over the inherent getters of the same name.
impl LatLon for Coordinate {
    fn lat(&self) -> f64 {
        self.lat
    }
    fn lon(&self) -> f64 {
        self.lon
    }
    fn crs(&self) -> Crs {
        self.crs
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

#[cfg(test)]
mod tests {
    use super::*;

    fn wgs(lat: f64, lon: f64) -> Coordinate {
        Coordinate::wgs84(lat, lon).unwrap()
    }

    #[test]
    fn new_accepts_in_range_incl_poles_and_antimeridian() {
        assert!(Coordinate::wgs84(40.0, -74.0).is_ok());
        assert!(Coordinate::wgs84(90.0, 0.0).is_ok());
        assert!(Coordinate::wgs84(-90.0, 0.0).is_ok());
        // Longitude ±180 is accepted (closed range; construction does not normalize).
        assert_eq!(wgs(0.0, 180.0).lon(), 180.0);
        assert_eq!(wgs(0.0, -180.0).lon(), -180.0);
    }

    #[test]
    fn new_rejects_out_of_range_and_nan() {
        assert!(matches!(
            Coordinate::wgs84(91.0, 0.0),
            Err(Error::OutOfRange { lat, lon }) if lat == 91.0 && lon == 0.0
        ));
        assert!(Coordinate::wgs84(-90.5, 0.0).is_err());
        assert!(Coordinate::wgs84(0.0, 181.0).is_err());
        assert!(Coordinate::wgs84(0.0, -180.5).is_err());
        assert!(Coordinate::wgs84(f64::NAN, 0.0).is_err());
        assert!(Coordinate::wgs84(0.0, f64::NAN).is_err());
        assert!(Coordinate::wgs84(f64::INFINITY, 0.0).is_err());
        assert!(Coordinate::new(0.0, f64::NEG_INFINITY, Crs::Nad27).is_err());
    }

    #[test]
    fn with_height_rejects_non_finite_and_keeps_finite() {
        for bad in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            assert!(matches!(
                wgs(0.0, 0.0).with_height(Height::Ellipsoidal(bad)),
                Err(Error::InvalidValue {
                    field: "height",
                    ..
                })
            ));
            assert!(wgs(0.0, 0.0).with_height(Height::Orthometric(bad)).is_err());
        }
        let c = wgs(1.0, 2.0)
            .with_height(Height::Orthometric(-3.5))
            .unwrap();
        assert_eq!(c.height(), Some(Height::Orthometric(-3.5)));
        assert_eq!((c.lat(), c.lon(), c.crs()), (1.0, 2.0, Crs::Wgs84));
    }

    #[test]
    fn constructors_tag_their_datum_and_order_their_axes() {
        assert_eq!(Coordinate::gcj02(1.0, 2.0).unwrap().crs(), Crs::Gcj02);
        assert_eq!(Coordinate::bd09(1.0, 2.0).unwrap().crs(), Crs::Bd09);
        assert_eq!(wgs(1.0, 2.0).height(), None);
        let lon_first = Coordinate::from_lon_lat(120.0, 30.0, Crs::Tokyo).unwrap();
        assert_eq!(lon_first, Coordinate::new(30.0, 120.0, Crs::Tokyo).unwrap());
        // A lon-lat pair whose "latitude" is out of range is rejected, not swapped.
        assert!(Coordinate::from_lon_lat(10.0, 120.0, Crs::Wgs84).is_err());
    }

    /// C4: `Coordinate` has no `Display`/`FromStr`, so the only text round trip
    /// is the explicit `format` → `parse_coordinate` pair, which states its
    /// datum: the result is tagged WGS-84 rather than claiming to preserve one.
    #[test]
    fn text_round_trip_goes_through_explicit_routes() {
        let c = wgs(40.7128, -74.006);
        let rendered = crate::format::format(&c, &crate::format::FormatOptions::default());
        assert_eq!(rendered.as_deref(), Ok("40.712800, -74.006000"));
        let back = crate::parse::parse_coordinate(&rendered.unwrap()).unwrap();
        assert_eq!(back.coord, c);
    }

    /// C9: deserialization must apply the same range check as construction.
    #[cfg(feature = "serde")]
    #[test]
    fn deserialize_rejects_out_of_range_latitude() {
        let json = r#"{"lat":91.0,"lon":0.0,"height":null,"crs":"Wgs84"}"#;
        let parsed: serde_json::Result<Coordinate> = serde_json::from_str(json);
        assert!(parsed.is_err(), "lat = 91 deserialized: {parsed:?}");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_shape_is_unchanged_and_validates_height() {
        let c = wgs(1.5, -2.5)
            .with_height(Height::Ellipsoidal(7.0))
            .unwrap();
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(
            json,
            r#"{"lat":1.5,"lon":-2.5,"height":{"Ellipsoidal":7.0},"crs":"Wgs84"}"#
        );
        assert_eq!(serde_json::from_str::<Coordinate>(&json).unwrap(), c);
        let no_height = r#"{"lat":1.5,"lon":-2.5,"crs":"Gcj02"}"#;
        assert_eq!(
            serde_json::from_str::<Coordinate>(no_height).unwrap(),
            Coordinate::gcj02(1.5, -2.5).unwrap()
        );
        let bad_lon = r#"{"lat":0.0,"lon":180.5,"height":null,"crs":"Wgs84"}"#;
        assert!(serde_json::from_str::<Coordinate>(bad_lon).is_err());
    }

    /// JSON cannot carry a non-finite number, so the height check is reached
    /// through the conversion serde calls.
    #[cfg(feature = "serde")]
    #[test]
    fn deserialize_conversion_rejects_non_finite_height() {
        let repr = CoordinateRepr {
            lat: 0.0,
            lon: 0.0,
            height: Some(Height::Orthometric(f64::NAN)),
            crs: Crs::Wgs84,
        };
        assert!(matches!(
            Coordinate::try_from(repr),
            Err(Error::InvalidValue {
                field: "height",
                ..
            })
        ));
    }

    #[test]
    fn null_island_detection() {
        assert!(wgs(0.0, 0.0).is_null_island());
        assert!(wgs(0.0, -0.0).is_null_island());
        assert!(wgs(1e-7, 1e-7).is_null_island());
        // ~111 m away is a real position, not Null Island.
        assert!(!wgs(0.001, 0.0).is_null_island());
        assert!(!wgs(40.0, -74.0).is_null_island());
    }
}
