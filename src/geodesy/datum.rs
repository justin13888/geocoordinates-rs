//! Classic geodetic datum transforms via the 7-parameter Helmert
//! (Bursa-Wolf) model.
//!
//! Unlike the GCJ-02/BD-09 obfuscation transforms — whose inverses are
//! iterative and lossy — a Helmert transform is a closed-form rigid rotation,
//! translation, and scale of the geocentric (ECEF) frame, so these conversions
//! return bare types, not [`Approx`](crate::Approx). Two separate accuracies
//! apply:
//!
//! - **Computation.** Applying a transform is exact to floating-point
//!   round-off: geodetic → ECEF and ECEF → geodetic are both closed forms (see
//!   [`Ecef`]). Reversing one with [`Helmert::inverse_first_order`] /
//!   [`DatumTransform::inverse_first_order`] is exact for the translation-only
//!   catalog, but first-order — about a centimeter off — for a full
//!   seven-parameter set.
//! - **Parameters.** The built-in catalog ([`DatumTransform::catalog_to_wgs84`])
//!   uses regional *mean*, translation-only shifts from NIMA TR8350.2, which
//!   match a local realization of the datum only to meters or tens of meters.
//!   Each carries that published accuracy as
//!   [`DatumTransform::accuracy_m`], and
//!   [`convert`](crate::convert::convert) reports it as the leg's
//!   `max_error_m`.
//!
//! This module owns only the lightweight parametric path (a small catalog of
//! common datums: NAD27, Tokyo, Pulkovo-1942). Helmert (run through ECEF) is the
//! *only* parametric model offered: the abridged **Molodensky /
//! Molodensky-Badekas** transforms are deliberately omitted, as the ECEF Helmert
//! path is more general and at least as accurate. Higher-accuracy **grid-based**
//! transforms (NTv2, NADCON5), national grid projections, and the full EPSG
//! registry are out of scope here and are delegated to the deferred `proj`
//! feature (see
//! [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md)).
//!
//! Transforms are **static**. Epoch / time-aware geodesy — plate-motion velocity
//! models, the 14-parameter (rate-of-change) transforms, and distinct ITRF
//! realizations — is out of scope for v1; sub-centimeter, time-varying work is
//! not a goal here.

use core::f64::consts::PI;

use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::geodesy::ecef::{Ecef, ellipsoid_of};

/// Arc-seconds → radians (the rotation parameters are published in arc-seconds).
const ARCSEC_TO_RAD: f64 = PI / (180.0 * 3600.0);

/// The seven Bursa-Wolf parameters of a Helmert datum transformation.
///
/// Translations are in meters, rotations in arc-seconds, and scale in
/// parts-per-million — the convention published by EPSG and national geodetic
/// agencies. Rotations use the **position-vector** (`PV`) sign convention.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Helmert {
    /// X-axis translation, meters.
    pub tx_m: f64,
    /// Y-axis translation, meters.
    pub ty_m: f64,
    /// Z-axis translation, meters.
    pub tz_m: f64,
    /// X-axis rotation, arc-seconds (position-vector convention).
    pub rx_arcsec: f64,
    /// Y-axis rotation, arc-seconds (position-vector convention).
    pub ry_arcsec: f64,
    /// Z-axis rotation, arc-seconds (position-vector convention).
    pub rz_arcsec: f64,
    /// Scale difference, parts-per-million.
    pub scale_ppm: f64,
}

impl Helmert {
    /// The identity transform (no translation, rotation, or scale).
    pub const IDENTITY: Helmert = Helmert {
        tx_m: 0.0,
        ty_m: 0.0,
        tz_m: 0.0,
        rx_arcsec: 0.0,
        ry_arcsec: 0.0,
        rz_arcsec: 0.0,
        scale_ppm: 0.0,
    };

    /// Apply the transform to a geocentric (ECEF) position.
    ///
    /// Position-vector (`PV`) convention, linearized for the small rotation
    /// angles: `x' = T + (1 + s)·R·x`, where `R = I + Ω` and `Ω` is the
    /// skew-symmetric matrix of the rotation vector (so `R·x ≈ x + ω × x`).
    #[must_use]
    pub fn apply_ecef(&self, ecef: Ecef) -> Ecef {
        let rx = self.rx_arcsec * ARCSEC_TO_RAD;
        let ry = self.ry_arcsec * ARCSEC_TO_RAD;
        let rz = self.rz_arcsec * ARCSEC_TO_RAD;
        let s = 1.0 + self.scale_ppm * 1e-6;
        let (x, y, z) = (ecef.x, ecef.y, ecef.z);
        Ecef {
            x: self.tx_m + s * (x - rz * y + ry * z),
            y: self.ty_m + s * (rz * x + y - rx * z),
            z: self.tz_m + s * (-ry * x + rx * y + z),
        }
    }

    /// The first-order inverse transform, by negating every parameter.
    ///
    /// **Approximate** unless the transform is translation-only (as every
    /// built-in catalog entry is), where negation is the exact inverse. With
    /// rotations or scale it is the standard first-order Bursa-Wolf inverse,
    /// and a there-and-back round trip leaves a residual of roughly
    /// `(θ + s)·|t| + (θ² + s²)·r`, where `θ` is the rotation in radians, `s`
    /// the scale difference, `|t|` the translation length, and `r` ≈ 6.4e6 m.
    /// For a full published set such as WGS-84 → OSGB36 (≈ 20 ppm, ≈ 720 m)
    /// that is about a centimeter.
    ///
    /// ```
    /// use geocoordinates::geodesy::{Ecef, Helmert};
    ///
    /// let shift = Helmert { tx_m: 10.0, ..Helmert::IDENTITY };
    /// let p = Ecef::new(1.0, 2.0, 3.0);
    /// assert_eq!(shift.inverse_first_order().apply_ecef(shift.apply_ecef(p)), p);
    /// ```
    #[must_use]
    pub fn inverse_first_order(&self) -> Helmert {
        Helmert {
            tx_m: -self.tx_m,
            ty_m: -self.ty_m,
            tz_m: -self.tz_m,
            rx_arcsec: -self.rx_arcsec,
            ry_arcsec: -self.ry_arcsec,
            rz_arcsec: -self.rz_arcsec,
            scale_ppm: -self.scale_ppm,
        }
    }

    /// Whether all seven parameters are finite.
    fn is_finite(&self) -> bool {
        [
            self.tx_m,
            self.ty_m,
            self.tz_m,
            self.rx_arcsec,
            self.ry_arcsec,
            self.rz_arcsec,
            self.scale_ppm,
        ]
        .iter()
        .all(|v| v.is_finite())
    }
}

/// A complete datum transformation: a source and a target reference system,
/// the Helmert shift between their frames, and the shift's published accuracy.
///
/// Applying it runs geodetic → ECEF → [`Helmert`] → geodetic, on the
/// ellipsoid of each reference system ([`Ellipsoid::for_crs`]), so the
/// ellipsoids can never contradict the systems they belong to.
///
/// [`Ellipsoid::for_crs`]: crate::geodesy::Ellipsoid::for_crs
///
/// Valid by construction: build one with [`DatumTransform::new`] or take a
/// catalogued one from [`DatumTransform::catalog_to_wgs84`]. Both systems are
/// geodetic datums (not GCJ-02 or BD-09), the Helmert parameters are finite,
/// and the accuracy, when given, is finite and non-negative. Deserialization
/// goes through [`DatumTransform::new`].
///
/// ```
/// use geocoordinates::{Coordinate, Crs, geodesy::DatumTransform};
///
/// let nad27_to_wgs84 = DatumTransform::catalog_to_wgs84(Crs::Nad27).unwrap();
/// let wgs = nad27_to_wgs84.apply(Coordinate::new(40.0, -100.0, Crs::Nad27)?)?;
/// assert_eq!(wgs.crs(), Crs::Wgs84);
/// // TR8350.2 publishes ±5 m, ±5 m, ±6 m for this mean shift.
/// assert_eq!(nad27_to_wgs84.accuracy_m(), Some(86f64.sqrt()));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "DatumTransformRecord"))]
pub struct DatumTransform {
    from_crs: Crs,
    to_crs: Crs,
    helmert: Helmert,
    accuracy_m: Option<f64>,
}

/// Unvalidated serde shape of [`DatumTransform`]; deserialization goes
/// through [`DatumTransform::new`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct DatumTransformRecord {
    from_crs: Crs,
    to_crs: Crs,
    helmert: Helmert,
    accuracy_m: Option<f64>,
}

#[cfg(feature = "serde")]
impl TryFrom<DatumTransformRecord> for DatumTransform {
    type Error = Error;

    fn try_from(r: DatumTransformRecord) -> Result<Self> {
        DatumTransform::new(r.from_crs, r.to_crs, r.helmert, r.accuracy_m)
    }
}

impl DatumTransform {
    /// Build a transform carrying `from_crs` to `to_crs` by `helmert`, with
    /// `accuracy_m` the accuracy of the parameters themselves in meters
    /// (`None` when no estimate is published).
    ///
    /// # Errors
    /// - [`Error::CrsMismatch`] (expecting [`Crs::Wgs84`]) when either system
    ///   is GCJ-02 or BD-09, which are not geodetic datums and have no
    ///   ellipsoid;
    /// - [`Error::InvalidValue`] (field `"helmert"`) when a Helmert parameter
    ///   is not finite, and (field `"accuracy_m"`) when `accuracy_m` is
    ///   negative or not finite.
    ///
    /// ```
    /// use geocoordinates::{Crs, geodesy::{DatumTransform, Helmert}};
    ///
    /// let shift = Helmert { tx_m: -8.0, ty_m: 160.0, tz_m: 176.0, ..Helmert::IDENTITY };
    /// let t = DatumTransform::new(Crs::Nad27, Crs::Wgs84, shift, Some(10.0))?;
    /// assert_eq!(t.to_crs(), Crs::Wgs84);
    /// assert!(DatumTransform::new(Crs::Gcj02, Crs::Wgs84, shift, None).is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(
        from_crs: Crs,
        to_crs: Crs,
        helmert: Helmert,
        accuracy_m: Option<f64>,
    ) -> Result<Self> {
        ellipsoid_of(from_crs)?;
        ellipsoid_of(to_crs)?;
        if !helmert.is_finite() {
            return Err(Error::InvalidValue {
                field: "helmert",
                detail: "every parameter must be finite".into(),
            });
        }
        if accuracy_m.is_some_and(|a| !a.is_finite() || a < 0.0) {
            return Err(Error::InvalidValue {
                field: "accuracy_m",
                detail: "must be finite and non-negative".into(),
            });
        }
        Ok(DatumTransform {
            from_crs,
            to_crs,
            helmert,
            accuracy_m,
        })
    }

    /// The built-in transform carrying `datum` to WGS-84, if one is catalogued.
    ///
    /// Each is a regional mean, translation-only shift from NIMA TR8350.2
    /// (Third Edition, Amendment 1, 3 January 2000), with its published
    /// accuracy in [`accuracy_m`](Self::accuracy_m):
    ///
    /// | Datum | Entry | Shift (m) | Accuracy |
    /// |---|---|---|---|
    /// | NAD27 | App. B.6, NAS-C, mean solution (CONUS) | −8, 160, 176 | ±5, ±5, ±6 m → √86 ≈ 9.27 m |
    /// | Tokyo | App. B.3, TOY-M, mean solution (Japan, Okinawa, South Korea) | −148, 507, 685 | ±20, ±5, ±20 m → √825 ≈ 28.72 m |
    /// | Pulkovo-1942 | App. C.2, PUK, Russia | 28, −130, −95 | none published (`None`) |
    ///
    /// Returns `None` for [`Crs::Wgs84`] (no shift needed), for the China
    /// obfuscation systems (use the [`china`](crate::china) typed conversions),
    /// and for datums reachable only through the deferred `proj` feature (see
    /// [STABILIZATION.md](https://github.com/justin13888/geocoordinates-rs/blob/master/STABILIZATION.md)).
    ///
    /// ```
    /// use geocoordinates::{Crs, geodesy::DatumTransform};
    ///
    /// let tokyo = DatumTransform::catalog_to_wgs84(Crs::Tokyo).unwrap();
    /// assert_eq!(tokyo.from_crs(), Crs::Tokyo);
    /// assert!(DatumTransform::catalog_to_wgs84(Crs::Wgs84).is_none());
    /// ```
    #[must_use]
    pub fn catalog_to_wgs84(datum: Crs) -> Option<DatumTransform> {
        match HubRoute::of(datum) {
            HubRoute::Helmert(transform) => Some(transform),
            HubRoute::Wgs84 | HubRoute::Gcj02 | HubRoute::Bd09 => None,
        }
    }

    /// The source reference system.
    #[must_use]
    pub fn from_crs(&self) -> Crs {
        self.from_crs
    }

    /// The target reference system.
    #[must_use]
    pub fn to_crs(&self) -> Crs {
        self.to_crs
    }

    /// The Helmert parameters carrying the source frame to the target frame.
    #[must_use]
    pub fn helmert(&self) -> Helmert {
        self.helmert
    }

    /// The accuracy of the parameters, in meters: how far the modeled shift
    /// may be from the local realization of the datum, or `None` when no
    /// estimate is published.
    ///
    /// For the catalog this is the root-sum-square of TR8350.2's per-axis
    /// `±` figures for ΔX, ΔY and ΔZ (see
    /// [`catalog_to_wgs84`](Self::catalog_to_wgs84)). Those are statistical
    /// estimates over the stations the mean was fitted to, not a hard maximum,
    /// and a point far from that region can be worse.
    #[must_use]
    pub fn accuracy_m(&self) -> Option<f64> {
        self.accuracy_m
    }

    /// Transform a geodetic coordinate from the source to the target datum,
    /// tagging the result with [`to_crs`](Self::to_crs).
    ///
    /// The computation is exact to round-off; the result is only as accurate
    /// as the Helmert parameters ([`accuracy_m`](Self::accuracy_m)). A 2-D
    /// input gives a 2-D output: the shifted ellipsoidal height is dropped.
    ///
    /// # Errors
    /// - [`Error::CrsMismatch`] when `coord` is not in
    ///   [`from_crs`](Self::from_crs);
    /// - [`Error::InvalidValue`] (field `"height"`) when `coord` carries an
    ///   orthometric height, which needs a geoid model to become ellipsoidal.
    pub fn apply(&self, coord: Coordinate) -> Result<Coordinate> {
        if coord.crs() != self.from_crs {
            return Err(Error::CrsMismatch {
                expected: self.from_crs,
                found: coord.crs(),
            });
        }
        let had_height = coord.height().is_some();
        let shifted = self.helmert.apply_ecef(Ecef::try_from_coordinate(coord)?);
        let result = shifted.try_to_coordinate(self.to_crs)?;
        if had_height {
            Ok(result)
        } else {
            Ok(Coordinate::from_valid(
                result.lat(),
                result.lon(),
                None,
                result.crs(),
            ))
        }
    }

    /// The reverse transform: swaps the reference systems and takes
    /// [`Helmert::inverse_first_order`] of the shift, keeping the accuracy.
    ///
    /// Exact for translation-only shifts (the whole built-in catalog);
    /// otherwise it inherits the first-order error of
    /// [`Helmert::inverse_first_order`], about a centimeter for a full
    /// seven-parameter set.
    ///
    /// ```
    /// use geocoordinates::{Crs, geodesy::DatumTransform};
    ///
    /// let back = DatumTransform::catalog_to_wgs84(Crs::Nad27).unwrap().inverse_first_order();
    /// assert_eq!((back.from_crs(), back.to_crs()), (Crs::Wgs84, Crs::Nad27));
    /// ```
    #[must_use]
    pub fn inverse_first_order(&self) -> DatumTransform {
        DatumTransform {
            from_crs: self.to_crs,
            to_crs: self.from_crs,
            helmert: self.helmert.inverse_first_order(),
            accuracy_m: self.accuracy_m,
        }
    }
}

/// How a reference system reaches the WGS-84 hub of
/// [`convert`](crate::convert::convert): one variant per route, so the match
/// over [`Crs`] in [`HubRoute::of`] is the single, exhaustive catalog lookup.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HubRoute {
    /// Already WGS-84; no shift.
    Wgs84,
    /// GCJ-02, reached by the [`china`](crate::china) typed offset.
    Gcj02,
    /// BD-09, reached by the [`china`](crate::china) typed offset.
    Bd09,
    /// A classic datum, reached by its catalogued Helmert transform.
    Helmert(DatumTransform),
}

/// Root-sum-square of a translation's published per-axis `±` figures.
fn rss_m(x: f64, y: f64, z: f64) -> f64 {
    (x * x + y * y + z * z).sqrt()
}

impl HubRoute {
    /// The route `datum` takes to WGS-84. Exhaustive: adding a [`Crs`]
    /// variant must force a decision here, never a silent WGS-84
    /// fallthrough.
    pub(crate) fn of(datum: Crs) -> HubRoute {
        // NIMA TR8350.2 (3rd ed., Amendment 1) mean translation-only shifts;
        // see `DatumTransform::catalog_to_wgs84` for the table entries.
        let helmert = |tx_m, ty_m, tz_m, accuracy_m| {
            HubRoute::Helmert(DatumTransform {
                from_crs: datum,
                to_crs: Crs::Wgs84,
                helmert: Helmert {
                    tx_m,
                    ty_m,
                    tz_m,
                    ..Helmert::IDENTITY
                },
                accuracy_m,
            })
        };
        match datum {
            Crs::Wgs84 => HubRoute::Wgs84,
            Crs::Gcj02 => HubRoute::Gcj02,
            Crs::Bd09 => HubRoute::Bd09,
            Crs::Nad27 => helmert(-8.0, 160.0, 176.0, Some(rss_m(5.0, 5.0, 6.0))),
            Crs::Tokyo => helmert(-148.0, 507.0, 685.0, Some(rss_m(20.0, 5.0, 20.0))),
            // Appendix C.2: "error estimates are not available".
            Crs::Pulkovo42 => helmert(28.0, -130.0, -95.0, None),
        }
    }
}

#[cfg(test)]
mod tests;
