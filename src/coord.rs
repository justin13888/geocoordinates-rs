//! The canonical coordinate model, its reference-system tag, and the
//! surface-tagged [`Height`].
//!
//! Design (locked): a **lean** [`Coordinate`] (position + optional height +
//! reference system) is what the geodetic math and the central
//! [`Coordinate::try_to_crs`] dispatch operate on. Rich observation metadata
//! (accuracy, timestamp, raw source, parse confidence) lives separately in
//! [`crate::fix::Fix`], populated by the ingestion layer.
//!
//! A [`Coordinate`] is valid by construction: its fields are private, and every
//! entry point — the constructors, [`Coordinate::with_height`], and serde
//! deserialization — rejects a non-finite or out-of-range value, so code that
//! holds one never re-checks it.

use core::fmt;

use crate::error::{Error, Result};
use crate::units::Length;

/// A coordinate reference system / datum tag used for runtime dispatch.
///
/// GCJ-02 and BD-09 are obfuscation transforms rather than true geodetic
/// datums, but are modeled here as reference systems so the central
/// [`Coordinate::try_to_crs`] dispatch can dispatch over them uniformly.
///
/// Exhaustive (no `#[non_exhaustive]`): the FFI mirror enumerates every variant,
/// so adding a datum here is a deliberate, compile-forcing change on both sides
/// (a wildcard fallback would silently mislabel an unknown datum as WGS-84).
///
/// `Display` renders the short canonical name used in errors:
///
/// ```
/// use geocoordinates::{Coordinate, Crs};
///
/// let tokyo = Coordinate::new(35.68, 139.77, Crs::Tokyo)?;
/// assert_eq!(tokyo.crs(), Crs::Tokyo);
/// assert_eq!(Crs::Gcj02.to_string(), "GCJ-02");
/// assert_eq!(Crs::Pulkovo42.to_string(), "Pulkovo-1942");
/// # Ok::<(), geocoordinates::Error>(())
/// ```
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

/// The lean canonical coordinate: position, optional height, and its CRS.
///
/// Valid by construction: latitude is in `[-90, 90]`, longitude in
/// `[-180, 180]` (a closed range — `180` is accepted and not normalized), both
/// are finite, and any height is finite. The fields are private; build one with
/// [`new`](Self::new) (latitude first), [`from_lon_lat`](Self::from_lon_lat)
/// (longitude first), or a per-datum shorthand such as
/// [`wgs84`](Self::wgs84), and read it back with the getters.
///
/// ```
/// use geocoordinates::{Coordinate, Crs, Error};
///
/// let nyc = Coordinate::new(40.7128, -74.006, Crs::Wgs84)?;
/// assert_eq!((nyc.lat(), nyc.lon(), nyc.crs()), (40.7128, -74.006, Crs::Wgs84));
/// // An invalid position never becomes a `Coordinate`.
/// assert!(matches!(
///     Coordinate::new(91.0, 0.0, Crs::Wgs84),
///     Err(Error::OutOfRange { .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
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
/// The fields stay private, so a struct literal cannot skip the validation:
///
/// ```compile_fail,E0451
/// use geocoordinates::{Coordinate, Crs};
/// // Expected: E0451, the fields of `Coordinate` are private.
/// let _ = Coordinate { lat: 91.0, lon: 0.0, height: None, crs: Crs::Wgs84 };
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
    /// accepted; see [`is_null_island`](Self::is_null_island). A `-0.0` is
    /// stored as `+0.0`.
    ///
    /// # Errors
    /// Returns [`Error::OutOfRange`] when latitude is outside `[-90, 90]`,
    /// longitude is outside `[-180, 180]`, or either is not finite.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs, Error};
    ///
    /// // The antimeridian is accepted as given, not normalized to -180.
    /// let c = Coordinate::new(-16.5, 180.0, Crs::Wgs84)?;
    /// assert_eq!(c.lon(), 180.0);
    /// assert_eq!(
    ///     Coordinate::new(0.0, 181.0, Crs::Wgs84),
    ///     Err(Error::OutOfRange { lat: 0.0, lon: 181.0 })
    /// );
    /// let nan = Coordinate::new(f64::NAN, 0.0, Crs::Wgs84);
    /// assert!(matches!(nan, Err(Error::OutOfRange { lat, lon: 0.0 }) if lat.is_nan()));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(lat: f64, lon: f64, crs: Crs) -> Result<Self> {
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            return Err(Error::OutOfRange { lat, lon });
        }
        Ok(Self::from_valid(lat, lon, None, crs))
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
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs};
    ///
    /// // A GeoJSON position `[2.3522, 48.8566]` is longitude first.
    /// let paris = Coordinate::from_lon_lat(2.3522, 48.8566, Crs::Wgs84)?;
    /// assert_eq!((paris.lat(), paris.lon()), (48.8566, 2.3522));
    /// // A lat-lon pair passed here puts 121.5 in the latitude slot and is rejected.
    /// assert!(Coordinate::from_lon_lat(31.2, 121.5, Crs::Wgs84).is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn from_lon_lat(lon: f64, lat: f64, crs: Crs) -> Result<Self> {
        Self::new(lat, lon, crs)
    }

    /// Construct a WGS-84 coordinate from latitude/longitude in degrees.
    ///
    /// # Errors
    /// As [`new`](Self::new).
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs};
    ///
    /// let c = Coordinate::wgs84(51.4779, -0.0015)?;
    /// assert_eq!(c.crs(), Crs::Wgs84);
    /// assert!(Coordinate::wgs84(-90.5, 0.0).is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn wgs84(lat: f64, lon: f64) -> Result<Self> {
        Self::new(lat, lon, Crs::Wgs84)
    }

    /// Construct a GCJ-02 ("Mars") coordinate from latitude/longitude in degrees.
    ///
    /// # Errors
    /// As [`new`](Self::new).
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs};
    ///
    /// let c = Coordinate::gcj02(39.9165, 116.4102)?;
    /// assert_eq!(c.crs(), Crs::Gcj02);
    /// assert!(Coordinate::gcj02(39.9, 216.4).is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn gcj02(lat: f64, lon: f64) -> Result<Self> {
        Self::new(lat, lon, Crs::Gcj02)
    }

    /// Construct a BD-09 (Baidu) coordinate from latitude/longitude in degrees.
    ///
    /// # Errors
    /// As [`new`](Self::new).
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs};
    ///
    /// let c = Coordinate::bd09(39.9228, 116.4166)?;
    /// assert_eq!(c.crs(), Crs::Bd09);
    /// assert!(Coordinate::bd09(f64::INFINITY, 116.4).is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn bd09(lat: f64, lon: f64) -> Result<Self> {
        Self::new(lat, lon, Crs::Bd09)
    }

    /// Build a coordinate from components the caller has already proven valid.
    ///
    /// For internal producers whose math guarantees the invariant (a validated
    /// input, a wrapped longitude). Debug builds assert it. Adding `+0.0`
    /// stores a `-0.0` component as `+0.0` and leaves every other value as is.
    pub(crate) fn from_valid(lat: f64, lon: f64, height: Option<Height>, crs: Crs) -> Self {
        debug_assert!(
            (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon),
            "Coordinate::from_valid out of range: lat={lat}, lon={lon}"
        );
        debug_assert!(
            height.is_none_or(|h| h.length().meters().is_finite()),
            "Coordinate::from_valid non-finite height: {height:?}"
        );
        Self {
            lat: lat + 0.0,
            lon: lon + 0.0,
            height,
            crs,
        }
    }

    /// Attach a height, returning the updated coordinate.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] (field `height`) when the height is not
    /// finite.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, Height};
    ///
    /// let base = Coordinate::wgs84(46.5, 7.9)?;
    /// let up = base.with_height(Height::Ellipsoidal(4158.0))?;
    /// assert_eq!(up.height(), Some(Height::Ellipsoidal(4158.0)));
    /// assert!(matches!(
    ///     base.with_height(Height::Ellipsoidal(f64::NAN)),
    ///     Err(Error::InvalidValue { field: "height", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn with_height(self, height: Height) -> Result<Self> {
        if !height.length().meters().is_finite() {
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
    ///
    /// ```
    /// let c = geocoordinates::Coordinate::wgs84(-33.8688, 151.2093)?;
    /// assert_eq!(c.lat(), -33.8688);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn lat(self) -> f64 {
        self.lat
    }

    /// Longitude in decimal degrees, in `[-180, 180]`.
    ///
    /// ```
    /// let c = geocoordinates::Coordinate::wgs84(-33.8688, 151.2093)?;
    /// assert_eq!(c.lon(), 151.2093);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn lon(self) -> f64 {
        self.lon
    }

    /// The reference system the position is expressed in.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Crs};
    ///
    /// let c = Coordinate::new(59.94, 30.31, Crs::Pulkovo42)?;
    /// assert_eq!(c.crs(), Crs::Pulkovo42);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn crs(self) -> Crs {
        self.crs
    }

    /// The height (ellipsoidal or orthometric), if one is attached.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Height};
    ///
    /// let c = Coordinate::wgs84(0.5, 0.5)?;
    /// assert_eq!(c.height(), None);
    /// let c = c.with_height(Height::Ellipsoidal(-12.0))?;
    /// assert_eq!(c.height(), Some(Height::Ellipsoidal(-12.0)));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn height(self) -> Option<Height> {
        self.height
    }

    /// Whether this is "Null Island" — latitude and longitude both ~0, the
    /// telltale of a missing or defaulted fix rather than a real position in
    /// the Gulf of Guinea.
    ///
    /// True when both components are within `1e-6`° (~0.11 m) of zero.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    ///
    /// assert!(Coordinate::wgs84(0.0, 0.0)?.is_null_island());
    /// // ~111 m off the origin is a real position.
    /// assert!(!Coordinate::wgs84(0.001, 0.0)?.is_null_island());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn is_null_island(self) -> bool {
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
/// Implemented by [`Coordinate`] and the per-datum newtypes, so the
/// [`geodesy`](crate::geodesy) measurements (distances, bearings, cross- and
/// along-track) accept any of them by value, as sugar over `Coordinate`.
/// Everything else ([`format`](crate::format::format), the position producers,
/// the grid encoders) takes a `Coordinate`: pass a newtype's `to_coordinate()`.
/// The trait is **sealed** (a function taking `impl LatLon` only ever sees this
/// crate's types) and requires `Copy`.
///
/// ```
/// use geocoordinates::china::Wgs84;
/// use geocoordinates::{Coordinate, LatLon, geodesy::geodesic_distance};
///
/// fn describe(p: impl LatLon) -> String { format!("{} {:.4},{:.4}", p.crs(), p.lat(), p.lon()) }
/// assert_eq!(describe(Coordinate::wgs84(1.0, 2.0)?), "WGS84 1.0000,2.0000");
/// let origin = Wgs84::new(0.0, 0.0)?; // a newtype and a Coordinate, mixed
/// assert!(geodesic_distance(origin, Coordinate::wgs84(0.0, 1.0)?)?.meters() > 0.0);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
///
/// Being sealed, it cannot be implemented outside this crate, even by a `Copy`
/// type:
///
/// ```compile_fail,E0277
/// // Expected: E0277, `Mine: Sealed` is not satisfied (not a missing `Copy`).
/// #[derive(Clone, Copy)]
/// struct Mine;
/// impl geocoordinates::LatLon for Mine {
///     fn lat(self) ->f64 { 0.0 }
///     fn lon(self) ->f64 { 0.0 }
///     fn crs(self) ->geocoordinates::Crs { geocoordinates::Crs::Wgs84 }
/// }
/// ```
pub trait LatLon: sealed::Sealed + Copy {
    /// Latitude in decimal degrees.
    ///
    /// ```
    /// use geocoordinates::LatLon;
    /// use geocoordinates::china::Wgs84;
    ///
    /// assert_eq!(LatLon::lat(Wgs84::new(10.0, 20.0)?), 10.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    fn lat(self) -> f64;
    /// Longitude in decimal degrees.
    ///
    /// ```
    /// use geocoordinates::LatLon;
    /// use geocoordinates::china::Wgs84;
    ///
    /// assert_eq!(LatLon::lon(Wgs84::new(10.0, 20.0)?), 20.0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    fn lon(self) -> f64;
    /// Reference system of the position.
    ///
    /// ```
    /// use geocoordinates::china::Wgs84;
    /// use geocoordinates::{Crs, LatLon};
    ///
    /// assert_eq!(LatLon::crs(Wgs84::new(10.0, 20.0)?), Crs::Wgs84);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    fn crs(self) -> Crs;
}

impl sealed::Sealed for Coordinate {}

/// Sugar over the inherent getters of the same name.
impl LatLon for Coordinate {
    fn lat(self) -> f64 {
        self.lat
    }
    fn lon(self) -> f64 {
        self.lon
    }
    fn crs(self) -> Crs {
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

/// A height value, tagged by the surface it is measured from.
///
/// GNSS reports **ellipsoidal** height natively; humans expect **orthometric**
/// height (above the geoid / "sea level"). Converting between them requires a
/// geoid model — deferred behind the `geoid` feature (see
/// [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md)).
///
/// Only these two surfaces are modeled; tidal datums (MSL, MLLW, …) are out of
/// scope.
///
/// ```
/// use geocoordinates::{Coordinate, Height};
///
/// let summit = Coordinate::wgs84(27.9881, 86.925)?.with_height(Height::Orthometric(8848.86))?;
/// assert_eq!(summit.height(), Some(Height::Orthometric(8848.86)));
/// // The surface is part of the value: the same number above the ellipsoid differs.
/// assert_ne!(Height::Orthometric(8848.86), Height::Ellipsoidal(8848.86));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Height {
    /// Meters above the reference ellipsoid.
    Ellipsoidal(f64),
    /// Meters above the geoid (mean sea level).
    Orthometric(f64),
}

impl Height {
    /// The height as a [`Length`], whichever surface it is measured from;
    /// match on the variant to keep the surface.
    ///
    /// ```
    /// use geocoordinates::{Height, Length};
    ///
    /// assert_eq!(Height::Ellipsoidal(46.9).length(), Length::from_meters(46.9));
    /// assert_eq!(Height::Orthometric(-28.0).length().meters(), -28.0);
    /// ```
    #[must_use]
    pub const fn length(self) -> Length {
        match self {
            Height::Ellipsoidal(value) | Height::Orthometric(value) => Length::from_meters(value),
        }
    }
}

#[cfg(test)]
mod tests;
