//! Core mirrors: the reference-system tag, heights, the canonical coordinate
//! and its approximate form, and length units, with their constructors,
//! validators, and the runtime conversion dispatch. The `Fix` observation
//! family is in the sibling `fix` module.

use geocoordinates as gc;

use crate::GeoError;

// ===========================================================================
// Mirror types
// ===========================================================================

/// Coordinate reference system / datum tag — mirror of [`gc::Crs`].
///
/// `gc::Crs` is exhaustive, so the `From` impls below are exhaustive matches:
/// adding a datum upstream fails this crate's build until mirrored here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Crs {
    /// WGS-84 — the global GNSS reference and library default.
    Wgs84,
    /// GCJ-02 — "Mars" coordinates used by Chinese map providers.
    Gcj02,
    /// BD-09 — Baidu's additional obfuscation atop GCJ-02.
    Bd09,
    /// NAD27 — North American Datum 1927.
    Nad27,
    /// Tokyo datum (legacy Japan / Korea).
    Tokyo,
    /// Pulkovo-1942 / SK-42.
    Pulkovo42,
}

/// A height value tagged by its reference surface — mirror of [`gc::Height`].
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum Height {
    /// Meters above the reference ellipsoid.
    Ellipsoidal {
        /// Height in meters.
        meters: f64,
    },
    /// Meters above the geoid (mean sea level).
    Orthometric {
        /// Height in meters.
        meters: f64,
    },
}

/// The canonical coordinate — mirror of [`gc::Coordinate`].
///
/// A record can be built field by field in foreign code, so it is checked at
/// the boundary instead: every function that takes one rejects a latitude
/// outside `[-90, 90]`, a longitude outside `[-180, 180]`, a non-finite
/// component, or a non-finite height with [`GeoError::OutOfRange`] /
/// [`GeoError::InvalidValue`] before doing anything else. Build one with
/// [`coordinate_new`] to fail at construction, or call [`coordinate_validate`]
/// on a hand-built record. Every `Coordinate` this library returns is valid.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
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

/// A length unit — mirror of [`gc::LengthUnit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LengthUnit {
    /// Meter (SI).
    Meter,
    /// Kilometer.
    Kilometer,
    /// International foot (exactly 0.3048 m).
    Foot,
    /// US survey foot (1200/3937 m).
    UsSurveyFoot,
    /// Nautical mile (1852 m).
    NauticalMile,
    /// International statute mile (1609.344 m).
    Mile,
    /// International yard (0.9144 m).
    Yard,
}

// ===========================================================================
// Translation layer (mirror <-> core)
// ===========================================================================

impl From<gc::Crs> for Crs {
    fn from(c: gc::Crs) -> Self {
        match c {
            gc::Crs::Wgs84 => Crs::Wgs84,
            gc::Crs::Gcj02 => Crs::Gcj02,
            gc::Crs::Bd09 => Crs::Bd09,
            gc::Crs::Nad27 => Crs::Nad27,
            gc::Crs::Tokyo => Crs::Tokyo,
            gc::Crs::Pulkovo42 => Crs::Pulkovo42,
        }
    }
}

impl From<Crs> for gc::Crs {
    fn from(c: Crs) -> Self {
        match c {
            Crs::Wgs84 => gc::Crs::Wgs84,
            Crs::Gcj02 => gc::Crs::Gcj02,
            Crs::Bd09 => gc::Crs::Bd09,
            Crs::Nad27 => gc::Crs::Nad27,
            Crs::Tokyo => gc::Crs::Tokyo,
            Crs::Pulkovo42 => gc::Crs::Pulkovo42,
        }
    }
}

impl From<gc::Height> for Height {
    fn from(h: gc::Height) -> Self {
        match h {
            gc::Height::Ellipsoidal(meters) => Height::Ellipsoidal { meters },
            gc::Height::Orthometric(meters) => Height::Orthometric { meters },
        }
    }
}

impl From<Height> for gc::Height {
    fn from(h: Height) -> Self {
        match h {
            Height::Ellipsoidal { meters } => gc::Height::Ellipsoidal(meters),
            Height::Orthometric { meters } => gc::Height::Orthometric(meters),
        }
    }
}

impl From<gc::Coordinate> for Coordinate {
    fn from(c: gc::Coordinate) -> Self {
        Coordinate {
            lat: c.lat(),
            lon: c.lon(),
            height: c.height().map(Into::into),
            crs: c.crs().into(),
        }
    }
}

/// Every mirror record entering the core crate is validated here, so a
/// hand-built out-of-range record is rejected rather than computed on.
impl TryFrom<Coordinate> for gc::Coordinate {
    type Error = gc::Error;

    fn try_from(c: Coordinate) -> Result<Self, gc::Error> {
        let base = gc::Coordinate::new(c.lat, c.lon, c.crs.into())?;
        match c.height {
            Some(h) => base.with_height(h.into()),
            None => Ok(base),
        }
    }
}

impl From<gc::LengthUnit> for LengthUnit {
    fn from(u: gc::LengthUnit) -> Self {
        match u {
            gc::LengthUnit::Meter => LengthUnit::Meter,
            gc::LengthUnit::Kilometer => LengthUnit::Kilometer,
            gc::LengthUnit::Foot => LengthUnit::Foot,
            gc::LengthUnit::UsSurveyFoot => LengthUnit::UsSurveyFoot,
            gc::LengthUnit::NauticalMile => LengthUnit::NauticalMile,
            gc::LengthUnit::Mile => LengthUnit::Mile,
            gc::LengthUnit::Yard => LengthUnit::Yard,
        }
    }
}

impl From<LengthUnit> for gc::LengthUnit {
    fn from(u: LengthUnit) -> Self {
        match u {
            LengthUnit::Meter => gc::LengthUnit::Meter,
            LengthUnit::Kilometer => gc::LengthUnit::Kilometer,
            LengthUnit::Foot => gc::LengthUnit::Foot,
            LengthUnit::UsSurveyFoot => gc::LengthUnit::UsSurveyFoot,
            LengthUnit::NauticalMile => gc::LengthUnit::NauticalMile,
            LengthUnit::Mile => gc::LengthUnit::Mile,
            LengthUnit::Yard => gc::LengthUnit::Yard,
        }
    }
}

// ===========================================================================
// Exported API
// ===========================================================================

// --- Coordinate constructor ---

/// Construct a [`Coordinate`] from latitude and longitude in decimal degrees
/// (latitude first) in `crs`, with no height. Attach a height by setting the
/// record's `height` field; it is validated wherever the record is used.
///
/// # Errors
/// [`GeoError::OutOfRange`] when latitude is outside `[-90, 90]`, longitude is
/// outside `[-180, 180]`, or either is not finite.
#[uniffi::export]
pub fn coordinate_new(lat: f64, lon: f64, crs: Crs) -> Result<Coordinate, GeoError> {
    Ok(gc::Coordinate::new(lat, lon, crs.into())?.into())
}

/// Construct a [`Coordinate`] from longitude and latitude in decimal degrees
/// (**longitude first**, the GeoJSON / WKT X,Y order) in `crs`, with no
/// height — the FFI form of `Coordinate::from_lon_lat`. Use it where the
/// source is X,Y, so the swap is spelled by the function name rather than by
/// argument order at the call site.
///
/// # Errors
/// As [`coordinate_new`].
#[uniffi::export]
pub fn coordinate_from_lon_lat(lon: f64, lat: f64, crs: Crs) -> Result<Coordinate, GeoError> {
    Ok(gc::Coordinate::from_lon_lat(lon, lat, crs.into())?.into())
}

// --- Length unit conversions (`Length` is flattened to `f64` meters) ---

/// Convert `value` expressed in `unit` to meters.
#[uniffi::export]
pub fn length_from_unit(value: f64, unit: LengthUnit) -> f64 {
    gc::Length::from_unit(value, unit.into()).meters()
}

/// Convert `meters` to `unit`.
#[uniffi::export]
pub fn length_to_unit(meters: f64, unit: LengthUnit) -> f64 {
    gc::Length::from_meters(meters).to_unit(unit.into())
}

/// Meters in one of `unit` — mirror of [`gc::LengthUnit::meters_per_unit`].
#[uniffi::export]
pub fn length_unit_meters_per_unit(unit: LengthUnit) -> f64 {
    gc::LengthUnit::from(unit).meters_per_unit()
}

/// The height in meters, whichever surface it is measured from — mirror of
/// [`gc::Height::length`] (`Length` is flattened to `f64` meters).
#[uniffi::export]
pub fn height_meters(height: Height) -> f64 {
    gc::Height::from(height).length().meters()
}

// --- Coordinate validation ---

/// Validate a (possibly hand-built) coordinate record: latitude in
/// `[-90, 90]`, longitude in `[-180, 180]`, both finite, and any height finite.
///
/// # Errors
/// [`GeoError::OutOfRange`] when either horizontal component is invalid, or
/// [`GeoError::InvalidValue`] when the height is not finite.
#[uniffi::export]
pub fn coordinate_validate(coord: Coordinate) -> Result<(), GeoError> {
    gc::Coordinate::try_from(coord)?;
    Ok(())
}

/// Whether the coordinate is "Null Island" — both components within ~0.11 m of
/// zero, the telltale of a missing or defaulted fix.
///
/// # Errors
/// As [`coordinate_validate`], for an invalid record.
#[uniffi::export]
pub fn coordinate_is_null_island(coord: Coordinate) -> Result<bool, GeoError> {
    Ok(gc::Coordinate::try_from(coord)?.is_null_island())
}

// ===========================================================================
// Runtime conversion dispatch
// ===========================================================================

/// A coordinate with its error bound — the single FFI form of
/// `Approx<Coordinate>`, returned by [`convert`], by `centroid` and by every grid /
/// cell decoder (Plus Code, geohash, Maidenhead, MGRS, H3, S2).
///
/// What `max_error_m` bounds depends on the producer:
///
/// - **`centroid`** bounds the geodesic distance from the result to the farthest
///   input.
/// - **Grid / cell decoders** return the WGS-84 cell center; the bound is the
///   distance from that center to the farthest point of the cell, as each
///   decoder documents it: the maximum geodesic corner distance (Plus Code,
///   geohash, Maidenhead, S2), the half diagonal in ground meters (MGRS), or
///   the cell radius (H3).
/// - **[`convert`]** sums its legs' estimates: the iterative GCJ-02 / BD-09
///   inverses report their own bound, the forward China offsets `0.0`, and
///   each classic-datum (Helmert) leg its parameters' published accuracy
///   (`DatumTransform.accuracy_m`: about 9.27 m for NAD27 and 28.72 m for
///   Tokyo, and infinity for Pulkovo-1942, which has no published estimate).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ApproxCoordinate {
    /// The coordinate (carries its [`Crs`]: the conversion target, or WGS-84
    /// for a decoded cell center).
    pub coord: Coordinate,
    /// Estimated upper bound on the positional error, in meters (see the
    /// record docs for what each producer bounds).
    pub max_error_m: f64,
}

/// Flatten an `Approx<Coordinate>` into its [`ApproxCoordinate`] record.
pub(crate) fn approx_coordinate(approx: gc::Approx<gc::Coordinate>) -> ApproxCoordinate {
    let max_error_m = approx.max_error().meters();
    ApproxCoordinate {
        coord: approx.into_inner().into(),
        max_error_m,
    }
}

/// Convert `coord` from its own reference system to `to`, routing through the
/// WGS-84 hub (China typed conversions + classic-datum Helmert transforms).
///
/// Every pair of reference systems has a route.
///
/// # Errors
/// `GeoError::OutOfRange` / `GeoError::InvalidValue` for an invalid record;
/// `GeoError::OutsideDomain` (operation `"BD-09"` or `"BD-09 inverse"`) when
/// a BD-09 leg would carry a latitude past a pole; `GeoError::InvalidValue`
/// (field `height`) when the coordinate carries an orthometric height and a
/// leg is a classic-datum Helmert shift.
#[uniffi::export]
pub fn convert(coord: Coordinate, to: Crs) -> Result<ApproxCoordinate, GeoError> {
    let coord: gc::Coordinate = coord.try_into()?;
    Ok(approx_coordinate(coord.try_to_crs(to.into())?))
}
