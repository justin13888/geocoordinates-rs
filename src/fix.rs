//! Observation metadata wrapping a [`Coordinate`].
//!
//! A [`Fix`] is what the ingestion layer (EXIF, NMEA, free-text, …) emits: a
//! position plus everything known *about* that position — accuracy, time, the
//! original raw input, and how confident the parse was. Keeping this out of
//! [`Coordinate`] means pure geodetic math never drags fix metadata around.

use std::time::SystemTime;

use crate::coord::Coordinate;
use crate::units::Length;
use crate::{Error, Result};

/// A coordinate plus all known observation metadata.
///
/// Scope is *positional* metadata only — accuracy, time, and parse provenance.
/// Motion/telemetry that some sources carry (heading from EXIF
/// `GPSImgDirection`, NMEA course and speed) is intentionally excluded to keep
/// the type lean. The planned `exif` feature carries a photo's capture direction
/// and motion in a wrapper around the `Fix`, not in the `Fix` itself.
///
/// `#[non_exhaustive]`, so a field can be added without a breaking release:
/// outside this crate a `Fix` is built with [`from_coord`](Self::from_coord)
/// and its public fields are then set one by one.
///
/// ```
/// use std::time::{Duration, SystemTime};
///
/// use geocoordinates::fix::{Accuracy, AccuracyKind, Fix};
/// use geocoordinates::{Coordinate, Length};
///
/// let mut fix = Fix::from_coord(Coordinate::wgs84(37.7749, -122.4194)?);
/// fix.accuracy = Some(Accuracy::new(
///     AccuracyKind::OneSigma,
///     Some(Length::from_meters(4.5)),
///     None,
/// )?);
/// fix.timestamp = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000));
/// assert_eq!(fix.accuracy.and_then(|a| a.horizontal()), Some(Length::from_meters(4.5)));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Fix {
    /// The observed position.
    pub coord: Coordinate,
    /// Positional accuracy, if reported.
    pub accuracy: Option<Accuracy>,
    /// Observation time, if known, as a [`SystemTime`] (UTC, no leap-second or
    /// GPS-week handling). Filled from GPX `<time>` and from an NMEA `RMC`
    /// sentence's date and time; `None` for sources that carry no full date.
    pub timestamp: Option<SystemTime>,
    /// The raw input and how confidently it was interpreted.
    pub source: Option<RawSource>,
}

impl Fix {
    /// Wrap a bare coordinate with no metadata.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::fix::Fix;
    ///
    /// let coord = Coordinate::wgs84(48.8584, 2.2945)?;
    /// let fix = Fix::from_coord(coord);
    /// assert_eq!(fix.coord, coord);
    /// assert!(fix.accuracy.is_none() && fix.timestamp.is_none() && fix.source.is_none());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn from_coord(coord: Coordinate) -> Self {
        Self {
            coord,
            accuracy: None,
            timestamp: None,
            source: None,
        }
    }
}

/// What an [`Accuracy`] magnitude means statistically.
///
/// Sources report error in incompatible ways — a hard cell bound, a one-sigma
/// estimate, a 50% radius, or a bare number with no stated confidence — so
/// every [`Accuracy`] carries the kind its producer reported, and two fixes'
/// magnitudes are comparable only when their kinds agree. Each parser in this
/// crate documents the kind it emits:
///
/// | Producer | Kind |
/// |---|---|
/// | A decoded grid reference (Plus Code, Geohash, Maidenhead, MGRS) | [`Bound`](Self::Bound) |
/// | NMEA `GGA` HDOP, GPX `<hdop>` (both `HDOP × 5 m`) | [`OneSigma`](Self::OneSigma) |
/// | A `geo:` URI's `u=` parameter | [`Unspecified`](Self::Unspecified) |
///
/// Exhaustive (no `#[non_exhaustive]`): the FFI mirror enumerates every
/// variant, so adding one here is a deliberate, compile-forcing change on both
/// sides.
///
/// ```
/// use geocoordinates::fix::AccuracyKind;
/// use geocoordinates::parse::parse_coordinate;
///
/// // A Plus Code decodes to its cell center; the cell bounds the error.
/// let fix = parse_coordinate("87G7PX7V+4J")?;
/// assert_eq!(fix.accuracy.map(|a| a.kind()), Some(AccuracyKind::Bound));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AccuracyKind {
    /// A hard maximum: the true position is never farther than the magnitude
    /// (e.g. the distance from a decoded grid cell's center to its farthest
    /// corner).
    Bound,
    /// A one-sigma (about 68% in one dimension, about 39% for a horizontal
    /// circle) estimate, such as `HDOP × UERE`.
    OneSigma,
    /// A circular error probable: the radius holding the true position 50% of
    /// the time.
    Cep50,
    /// A reported magnitude whose confidence level the source does not state
    /// (e.g. RFC 5870's `u=` uncertainty).
    Unspecified,
}

/// A positional accuracy estimate: a horizontal and / or vertical magnitude,
/// tagged with what those magnitudes mean.
///
/// Real fixes usually carry an error estimate; carrying and propagating it lets
/// the presentation layer avoid printing spurious precision. Full uncertainty
/// propagation (covariance matrices, DOP) is out of scope: accuracy stays a
/// scalar magnitude per axis, and its [`AccuracyKind`] says how to read it.
///
/// Valid by construction: [`new`](Self::new) (and, under the `serde` feature,
/// deserialization) rejects a negative, NaN or infinite magnitude, and an
/// estimate with neither magnitude — "no accuracy" is `Fix::accuracy = None`.
/// The JSON shape is `{"kind":"OneSigma","horizontal":{"meters":4.5},"vertical":null}`.
///
/// ```
/// use geocoordinates::fix::{Accuracy, AccuracyKind};
/// use geocoordinates::{Length, LengthUnit};
///
/// let a = Accuracy::new(
///     AccuracyKind::Cep50,
///     Some(Length::from_meters(3.0)),
///     Some(Length::from_unit(10.0, LengthUnit::Foot)),
/// )?;
/// assert_eq!(a.kind(), AccuracyKind::Cep50);
/// assert_eq!(a.horizontal().map(|h| h.meters()), Some(3.0));
/// assert_eq!(a.vertical().map(|v| v.meters()), Some(3.048));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "AccuracyRepr"))]
pub struct Accuracy {
    kind: AccuracyKind,
    horizontal: Option<Length>,
    vertical: Option<Length>,
}

/// The unchecked wire shape of an [`Accuracy`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct AccuracyRepr {
    kind: AccuracyKind,
    horizontal: Option<Length>,
    vertical: Option<Length>,
}

#[cfg(feature = "serde")]
impl TryFrom<AccuracyRepr> for Accuracy {
    type Error = Error;

    fn try_from(repr: AccuracyRepr) -> Result<Self> {
        Self::new(repr.kind, repr.horizontal, repr.vertical)
    }
}

impl Accuracy {
    /// Build an accuracy estimate of `kind` from its horizontal and vertical
    /// magnitudes; either may be absent, but not both.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] with field `horizontal` / `vertical`
    /// when that magnitude is negative, NaN or infinite, or with field
    /// `accuracy` when both are `None`.
    ///
    /// ```
    /// use geocoordinates::fix::{Accuracy, AccuracyKind};
    /// use geocoordinates::{Error, Length};
    ///
    /// let bound = Accuracy::new(AccuracyKind::Bound, Some(Length::from_meters(12.0)), None)?;
    /// assert_eq!(bound.vertical(), None);
    ///
    /// let negative = Accuracy::new(AccuracyKind::OneSigma, Some(Length::from_meters(-1.0)), None);
    /// assert!(matches!(negative, Err(Error::InvalidValue { field: "horizontal", .. })));
    /// let nan = Accuracy::new(AccuracyKind::OneSigma, None, Some(Length::from_meters(f64::NAN)));
    /// assert!(matches!(nan, Err(Error::InvalidValue { field: "vertical", .. })));
    /// let empty = Accuracy::new(AccuracyKind::Unspecified, None, None);
    /// assert!(matches!(empty, Err(Error::InvalidValue { field: "accuracy", .. })));
    /// # Ok::<(), Error>(())
    /// ```
    pub fn new(
        kind: AccuracyKind,
        horizontal: Option<Length>,
        vertical: Option<Length>,
    ) -> Result<Self> {
        check_magnitude(horizontal, "horizontal")?;
        check_magnitude(vertical, "vertical")?;
        if horizontal.is_none() && vertical.is_none() {
            return Err(Error::InvalidValue {
                field: "accuracy",
                detail: "needs a horizontal or a vertical magnitude".to_string(),
            });
        }
        Ok(Self {
            kind,
            horizontal,
            vertical,
        })
    }

    /// What the magnitudes mean statistically.
    ///
    /// ```
    /// use geocoordinates::fix::{Accuracy, AccuracyKind};
    /// use geocoordinates::Length;
    ///
    /// let a = Accuracy::new(AccuracyKind::Unspecified, Some(Length::from_meters(40.0)), None)?;
    /// assert_eq!(a.kind(), AccuracyKind::Unspecified);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn kind(&self) -> AccuracyKind {
        self.kind
    }

    /// The horizontal magnitude: a radius around the position, read as
    /// [`kind`](Self::kind) says. Finite and nonnegative.
    ///
    /// ```
    /// use geocoordinates::fix::{Accuracy, AccuracyKind};
    /// use geocoordinates::Length;
    ///
    /// let a = Accuracy::new(AccuracyKind::OneSigma, Some(Length::from_meters(4.5)), None)?;
    /// assert_eq!(a.horizontal(), Some(Length::from_meters(4.5)));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn horizontal(&self) -> Option<Length> {
        self.horizontal
    }

    /// The vertical magnitude, read as [`kind`](Self::kind) says. Finite and
    /// nonnegative.
    ///
    /// ```
    /// use geocoordinates::fix::{Accuracy, AccuracyKind};
    /// use geocoordinates::Length;
    ///
    /// let a = Accuracy::new(AccuracyKind::OneSigma, None, Some(Length::from_meters(8.0)))?;
    /// assert_eq!(a.vertical(), Some(Length::from_meters(8.0)));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn vertical(&self) -> Option<Length> {
        self.vertical
    }
}

/// Reject a present magnitude that is negative, NaN or infinite.
fn check_magnitude(magnitude: Option<Length>, field: &'static str) -> Result<()> {
    match magnitude {
        Some(m) if !(m.meters().is_finite() && m.meters() >= 0.0) => Err(Error::InvalidValue {
            field,
            detail: format!("must be finite and nonnegative, got {} m", m.meters()),
        }),
        _ => Ok(()),
    }
}

/// The original input a coordinate was parsed from, with parse confidence and
/// the ambiguities that were resolved.
///
/// `#[non_exhaustive]`, so a field can be added without a breaking release:
/// outside this crate a `RawSource` is built with [`new`](Self::new) and its
/// remaining public fields are then set one by one.
///
/// Parsers fill it in; read it off the [`Fix`] they return:
///
/// ```
/// use geocoordinates::fix::AxisOrder;
/// use geocoordinates::parse::parse_coordinate;
///
/// let fix = parse_coordinate("40.7128, -74.0060")?;
/// let source = fix.source.expect("the parser records its input");
/// assert_eq!(source.raw, "40.7128, -74.0060");
/// assert_eq!(source.axis_order, Some(AxisOrder::LatLon));
/// assert!(source.confidence.value() > 0.0);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct RawSource {
    /// The input this coordinate came from. For a single-coordinate input
    /// (free text, a `geo:` URI, a grid reference, an NMEA sentence) it is
    /// that input verbatim. For a structured document (GeoJSON, WKT, GPX, KML)
    /// it is this position's own fragment, re-rendered in the document's
    /// syntax — never the whole document.
    pub raw: String,
    /// How confidently `raw` was interpreted as this coordinate.
    pub confidence: Confidence,
    /// The order the source listed latitude and longitude in: `LatLon` for
    /// `geo:` URIs and NMEA, `LonLat` for GeoJSON and KML, the caller's choice
    /// for WKT, and for unlabelled free text the order the parser assumed.
    /// `None` when the axes are labelled rather than positional (a hemisphere
    /// letter in free text, GPX `lat`/`lon` attributes) or the source has no
    /// axis pair at all (a grid reference).
    pub axis_order: Option<AxisOrder>,
    /// A flagged datum ambiguity (e.g. China-EXIF possibly GCJ-02), when the
    /// source's reference system cannot be trusted as stated.
    pub datum_ambiguity: Option<DatumAmbiguity>,
    /// The geoid separation N the source reported: the height of the geoid
    /// above the WGS-84 ellipsoid at this position (negative where the geoid
    /// lies below it), filled from an NMEA `GGA` sentence. With the
    /// orthometric height H the source also reported, the ellipsoidal height
    /// is `h = H + N`.
    pub geoid_separation: Option<Length>,
    /// Free-text notes about anything else resolved during parsing.
    pub notes: Vec<String>,
}

impl RawSource {
    /// Record `raw` as interpreted with `confidence`, with no axis order,
    /// datum ambiguity, geoid separation or notes; set those fields afterwards
    /// where the source supplies them.
    ///
    /// ```
    /// use geocoordinates::fix::{AxisOrder, Confidence, RawSource};
    ///
    /// let mut source = RawSource::new("51.5, -0.12", Confidence::new(0.9)?);
    /// source.axis_order = Some(AxisOrder::LatLon);
    /// assert_eq!(source.raw, "51.5, -0.12");
    /// assert!(source.notes.is_empty() && source.geoid_separation.is_none());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn new(raw: impl Into<String>, confidence: Confidence) -> Self {
        Self {
            raw: raw.into(),
            confidence,
            axis_order: None,
            datum_ambiguity: None,
            geoid_separation: None,
            notes: Vec::new(),
        }
    }
}

/// The order latitude and longitude are listed in a textual/structured
/// coordinate.
///
/// GeoJSON and KML are **lon-lat (X,Y)**; WKT defines no order, so its parser
/// takes one from the caller; humans, `geo:` URIs, NMEA, and many EPSG CRS are
/// lat-first. Parsers record the listing order on [`RawSource::axis_order`] so
/// the application can decide whether to prompt the user.
///
/// ```
/// use geocoordinates::fix::AxisOrder;
/// use geocoordinates::parse::from_geo_uri;
///
/// let fix = from_geo_uri("geo:48.2082,16.3738")?;
/// assert_eq!(fix.source.and_then(|s| s.axis_order), Some(AxisOrder::LatLon));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AxisOrder {
    /// Latitude first (human/EPSG convention).
    LatLon,
    /// Longitude first (GeoJSON/KML X,Y convention, and most WKT).
    LonLat,
}

/// A flagged uncertainty about which datum a source's coordinates are in.
///
/// Some Chinese-market devices/apps embed **GCJ-02** in metadata (e.g. EXIF)
/// rather than WGS-84, plotting ~50–500 m off. Callers should resolve this
/// before trusting the datum.
///
/// Exhaustive (no `#[non_exhaustive]`): the FFI mirror enumerates every variant,
/// so adding one here is a deliberate, compile-forcing change on both sides.
///
/// ```
/// use geocoordinates::fix::{Confidence, DatumAmbiguity, RawSource};
///
/// let mut source = RawSource::new("39.915, 116.404", Confidence::new(0.6)?);
/// source.datum_ambiguity = Some(DatumAmbiguity::PossiblyGcj02);
/// source.notes.push("camera sold in mainland China".to_string());
/// assert_eq!(source.datum_ambiguity, Some(DatumAmbiguity::PossiblyGcj02));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DatumAmbiguity {
    /// Coordinate is in China's bounding box; datum may be GCJ-02, not WGS-84.
    PossiblyGcj02,
}

/// Parse confidence on a 0.0–1.0 scale.
///
/// The free-text parser must report this so the application can decide whether
/// to prompt the user (e.g. when axis order or locale was ambiguous).
///
/// Valid by construction: [`new`](Self::new) (and its [`TryFrom<f64>`] sugar)
/// rejects NaN and anything outside `[0.0, 1.0]`, and under the `serde` feature
/// deserialization goes through the same check. The JSON shape is a bare
/// number.
///
/// ```
/// use geocoordinates::fix::Confidence;
///
/// let c = Confidence::try_from(0.75)?;
/// assert_eq!(f64::from(c), 0.75);
/// assert!(Confidence::new(0.9)? > c);
/// assert!(Confidence::try_from(1.5).is_err());
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "f64", into = "f64"))]
pub struct Confidence(f64);

/// Thin sugar over [`Confidence::new`].
impl TryFrom<f64> for Confidence {
    type Error = Error;

    fn try_from(value: f64) -> Result<Self> {
        Self::new(value)
    }
}

/// Thin sugar over [`Confidence::value`].
impl From<Confidence> for f64 {
    fn from(confidence: Confidence) -> Self {
        confidence.value()
    }
}

impl Confidence {
    /// Full confidence: the input admits exactly one reading.
    pub(crate) const CERTAIN: Self = Self(1.0);

    /// A confidence from a crate constant already known to lie in `[0, 1]`.
    pub(crate) const fn from_constant(value: f64) -> Self {
        Self(value)
    }

    /// Construct a confidence from a value in `[0.0, 1.0]`.
    ///
    /// # Errors
    /// Returns [`Error::InvalidValue`] (field `confidence`) when `value` is
    /// NaN or outside `[0.0, 1.0]`.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::fix::Confidence;
    ///
    /// assert_eq!(Confidence::new(0.4)?.value(), 0.4);
    /// assert!(matches!(
    ///     Confidence::new(1.5),
    ///     Err(Error::InvalidValue { field: "confidence", .. })
    /// ));
    /// assert!(Confidence::new(f64::NAN).is_err());
    /// # Ok::<(), Error>(())
    /// ```
    pub fn new(value: f64) -> Result<Self> {
        if (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(Error::InvalidValue {
                field: "confidence",
                detail: format!("must be in [0, 1], got {value}"),
            })
        }
    }

    /// The confidence value, in `0.0–1.0`.
    ///
    /// ```
    /// use geocoordinates::fix::Confidence;
    ///
    /// assert_eq!(Confidence::new(0.25)?.value(), 0.25);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Accuracy, AccuracyKind, Confidence};
    use crate::{Error, Length};

    #[test]
    fn confidence_rejects_nan_and_out_of_range() {
        for bad in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY, -0.01, 1.01, 2.0] {
            assert!(
                matches!(
                    Confidence::new(bad),
                    Err(Error::InvalidValue {
                        field: "confidence",
                        ..
                    })
                ),
                "{bad}"
            );
            assert!(Confidence::try_from(bad).is_err(), "{bad}");
        }
        assert_eq!(Confidence::new(0.0).unwrap().value(), 0.0);
        assert_eq!(Confidence::new(1.0).unwrap(), Confidence::CERTAIN);
        assert_eq!(f64::from(Confidence::try_from(0.25).unwrap()), 0.25);
    }

    #[test]
    fn accuracy_rejects_invalid_magnitudes() {
        let m = |v| Some(Length::from_meters(v));
        for bad in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                matches!(
                    Accuracy::new(AccuracyKind::Bound, m(bad), None),
                    Err(Error::InvalidValue {
                        field: "horizontal",
                        ..
                    })
                ),
                "{bad}"
            );
            assert!(
                matches!(
                    Accuracy::new(AccuracyKind::Bound, m(1.0), m(bad)),
                    Err(Error::InvalidValue {
                        field: "vertical",
                        ..
                    })
                ),
                "{bad}"
            );
        }
        assert!(matches!(
            Accuracy::new(AccuracyKind::Cep50, None, None),
            Err(Error::InvalidValue {
                field: "accuracy",
                ..
            })
        ));
        let zero = Accuracy::new(AccuracyKind::Unspecified, Some(Length::ZERO), None).unwrap();
        assert_eq!(zero.horizontal(), Some(Length::ZERO));
        let vertical_only = Accuracy::new(AccuracyKind::OneSigma, None, m(2.0)).unwrap();
        assert_eq!(
            (
                vertical_only.kind(),
                vertical_only.horizontal(),
                vertical_only.vertical()
            ),
            (AccuracyKind::OneSigma, None, m(2.0))
        );
    }

    /// Deserialization validates like `new`, and the wire shape is a bare number.
    #[cfg(feature = "serde")]
    #[test]
    fn confidence_serde_validates_and_keeps_its_shape() {
        for bad in ["1.5", "-0.5"] {
            assert!(serde_json::from_str::<Confidence>(bad).is_err(), "{bad}");
        }
        let c: Confidence = serde_json::from_str("0.5").unwrap();
        assert_eq!(c.value(), 0.5);
        assert_eq!(serde_json::to_string(&c).unwrap(), "0.5");
    }

    /// Deserialization validates like `new`, and the kind travels with the
    /// magnitudes.
    #[cfg(feature = "serde")]
    #[test]
    fn accuracy_serde_validates_and_round_trips() {
        let a =
            Accuracy::new(AccuracyKind::OneSigma, Some(Length::from_meters(4.5)), None).unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"OneSigma","horizontal":{"meters":4.5},"vertical":null}"#
        );
        assert_eq!(serde_json::from_str::<Accuracy>(&json).unwrap(), a);
        for bad in [
            r#"{"kind":"Bound","horizontal":{"meters":-1.0},"vertical":null}"#,
            r#"{"kind":"Bound","horizontal":null,"vertical":{"meters":-0.5}}"#,
            r#"{"kind":"Bound","horizontal":null,"vertical":null}"#,
            r#"{"kind":"Sometimes","horizontal":{"meters":1.0},"vertical":null}"#,
        ] {
            assert!(serde_json::from_str::<Accuracy>(bad).is_err(), "{bad}");
        }
    }
}
