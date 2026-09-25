//! Classic geodetic datum transforms via the 7-parameter Helmert
//! (Bursa-Wolf) model.
//!
//! Unlike the GCJ-02/BD-09 obfuscation transforms — whose inverses are
//! iterative and lossy — a Helmert transform is a closed-form rigid rotation,
//! translation, and scale of the geocentric (ECEF) frame, so these conversions
//! return bare types, not [`Approx`](crate::Approx). Two separate accuracies
//! apply, and neither is zero:
//!
//! - **Computation.** Applying a transform is accurate to well under a
//!   millimeter at terrestrial heights; the residual comes from the ECEF →
//!   geodetic step (see [`Ecef::try_to_coordinate`]). Reversing one with
//!   [`Helmert::inverse`] / [`DatumTransform::inverse`] is exact for the
//!   translation-only catalog, but first-order — about a centimeter off — for a
//!   full seven-parameter set.
//! - **Parameters.** The built-in catalog ([`DatumTransform::to_wgs84`]) uses
//!   regional *mean*, translation-only shifts, which match a local realization
//!   of the datum only to several meters, and worse far from the region the
//!   mean was fitted to.
//!
//! Neither accuracy is reported as a bound: [`convert`](crate::convert::convert)
//! gives its Helmert legs a `max_error_m` of `0.0`, which means "no iterative
//! inversion", not "exact".
//!
//! This module owns only the lightweight parametric path (a small catalog of
//! common datums: NAD27, Tokyo, Pulkovo-1942). Helmert (run through ECEF) is the
//! *only* parametric model offered: the abridged **Molodensky /
//! Molodensky-Badekas** transforms are deliberately omitted, as the ECEF Helmert
//! path is more general and at least as accurate. Higher-accuracy **grid-based**
//! transforms (NTv2, NADCON5), national grid projections, and the full EPSG
//! registry are out of scope here and are delegated to the deferred `proj`
//! feature (see STABILIZATION.md).
//!
//! Transforms are **static**. Epoch / time-aware geodesy — plate-motion velocity
//! models, the 14-parameter (rate-of-change) transforms, and distinct ITRF
//! realizations — is out of scope for v1; sub-centimeter, time-varying work is
//! not a goal here.

use core::f64::consts::PI;

use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::geodesy::ecef::Ecef;
use crate::geodesy::ellipsoid::Ellipsoid;

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

    /// The inverse transform, by negating every parameter.
    ///
    /// **Approximate** unless the transform is translation-only (as every
    /// built-in catalog entry is), where negation is the exact inverse. With
    /// rotations or scale it is the standard first-order Bursa-Wolf inverse,
    /// and a there-and-back round trip leaves a residual of roughly
    /// `(θ + s)·|t| + (θ² + s²)·r`, where `θ` is the rotation in radians, `s`
    /// the scale difference, `|t|` the translation length, and `r` ≈ 6.4e6 m.
    /// For a full published set such as WGS-84 → OSGB36 (≈ 20 ppm, ≈ 720 m)
    /// that is about a centimeter.
    #[must_use]
    pub fn inverse(&self) -> Helmert {
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
}

/// A complete datum transformation: the source and target ellipsoids plus the
/// Helmert shift between their reference frames.
///
/// Applying it runs geodetic → ECEF (source ellipsoid) → [`Helmert`] →
/// geodetic (target ellipsoid).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DatumTransform {
    /// Source reference system.
    pub from_crs: Crs,
    /// Target reference system.
    pub to_crs: Crs,
    /// Ellipsoid of the source datum.
    pub from: Ellipsoid,
    /// Ellipsoid of the target datum.
    pub to: Ellipsoid,
    /// Helmert parameters carrying the source frame to the target frame.
    pub helmert: Helmert,
}

impl DatumTransform {
    /// The built-in transform carrying `datum` to WGS-84, if one is catalogued.
    ///
    /// Returns `None` for [`Crs::Wgs84`] (no shift needed), for the China
    /// obfuscation systems (use the [`china`](crate::china) typed conversions),
    /// and for datums reachable only through the deferred `proj` feature (see
    /// STABILIZATION.md).
    #[must_use]
    pub fn to_wgs84(datum: Crs) -> Option<DatumTransform> {
        // NIMA TR8350.2 mean translation-only (Molodensky) shifts. Exhaustive
        // match — adding a `Crs` variant must force a decision here, never a
        // silent WGS-84 fallthrough.
        let (from, helmert) = match datum {
            Crs::Nad27 => (
                Ellipsoid::CLARKE_1866,
                Helmert {
                    tx_m: -8.0,
                    ty_m: 160.0,
                    tz_m: 176.0,
                    ..Helmert::IDENTITY
                },
            ),
            Crs::Tokyo => (
                Ellipsoid::BESSEL_1841,
                Helmert {
                    tx_m: -148.0,
                    ty_m: 507.0,
                    tz_m: 685.0,
                    ..Helmert::IDENTITY
                },
            ),
            Crs::Pulkovo42 => (
                Ellipsoid::KRASOVSKY_1940,
                Helmert {
                    tx_m: 28.0,
                    ty_m: -130.0,
                    tz_m: -95.0,
                    ..Helmert::IDENTITY
                },
            ),
            // WGS-84 needs no shift; the China systems use the `china` typed
            // conversions, not a Helmert transform.
            Crs::Wgs84 | Crs::Gcj02 | Crs::Bd09 => return None,
        };
        Some(DatumTransform {
            from_crs: datum,
            to_crs: Crs::Wgs84,
            from,
            to: Ellipsoid::WGS84,
            helmert,
        })
    }

    /// Transform a geodetic coordinate from the source to the target datum,
    /// tagging the result with `self.to_crs`.
    ///
    /// This computes the modeled transform to well under a millimeter at
    /// terrestrial heights. It is **not** exact, and it is only as accurate as
    /// the Helmert parameters: the built-in mean shifts are good to several
    /// meters (see the [module docs](self)). `DatumTransform` holds only the
    /// two ellipsoids — which do not uniquely determine a [`Crs`] (e.g. GRS80
    /// backs both NAD83 and ETRS89) — so the target reference system is stored
    /// explicitly on `self.to_crs` rather than inferred from the ellipsoid.
    pub fn transform(&self, coord: Coordinate) -> Result<Coordinate> {
        if coord.crs != self.from_crs {
            return Err(Error::CrsMismatch {
                expected: self.from_crs,
                found: coord.crs,
            });
        }
        let had_height = coord.height.is_some();
        let ecef = Ecef::try_from_coordinate(coord, self.from)?;
        let shifted = self.helmert.apply_ecef(ecef);
        let mut result = shifted.try_to_coordinate(self.to, self.to_crs)?;
        if !had_height {
            result.height = None;
        }
        Ok(result)
    }

    /// The reverse transform (swaps ellipsoids and inverts the Helmert shift).
    ///
    /// Exact for translation-only shifts (the whole built-in catalog);
    /// otherwise it inherits the first-order error of [`Helmert::inverse`].
    #[must_use]
    pub fn inverse(&self) -> DatumTransform {
        DatumTransform {
            from_crs: self.to_crs,
            to_crs: self.from_crs,
            from: self.to,
            to: self.from,
            helmert: self.helmert.inverse(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coord::Height;
    use crate::test_support::assert_close;

    fn ecef(x: f64, y: f64, z: f64) -> Ecef {
        Ecef { x, y, z }
    }

    /// A synthetic Helmert with every parameter non-zero, for exercising all
    /// seven terms (rotations deliberately not proportional to any test vector).
    const FULL: Helmert = Helmert {
        tx_m: 10.0,
        ty_m: -20.0,
        tz_m: 30.0,
        rx_arcsec: 2.0,
        ry_arcsec: -3.0,
        rz_arcsec: 4.0,
        scale_ppm: 5.0,
    };

    #[test]
    fn identity_is_a_no_op() {
        let e = ecef(4_000_000.0, -2_000_000.0, 4_500_000.0);
        let r = Helmert::IDENTITY.apply_ecef(e);
        assert_close(r.x, e.x, 1e-9);
        assert_close(r.y, e.y, 1e-9);
        assert_close(r.z, e.z, 1e-9);
    }

    #[test]
    fn apply_ecef_translation_only() {
        let h = Helmert {
            tx_m: 10.0,
            ty_m: -20.0,
            tz_m: 30.0,
            ..Helmert::IDENTITY
        };
        let r = h.apply_ecef(ecef(1_000.0, 2_000.0, 3_000.0));
        assert_close(r.x, 1_010.0, 1e-9);
        assert_close(r.y, 1_980.0, 1e-9);
        assert_close(r.z, 3_030.0, 1e-9);
    }

    #[test]
    fn apply_ecef_full_seven_parameter() {
        // Independent reference (geodetic/ECEF Python): every term contributes.
        let r = FULL.apply_ecef(ecef(4_000_000.0, -2_000_000.0, 4_500_000.0));
        assert_close(r.x, 4_000_003.335_114_215_5, 1e-4);
        assert_close(r.y, -1_999_996.062_872_637_8, 1e-4);
        assert_close(r.z, 4_500_091.285_288_414, 1e-4);
    }

    #[test]
    fn inverse_negates_every_parameter() {
        let i = FULL.inverse();
        assert_close(i.tx_m, -10.0, 1e-12);
        assert_close(i.ty_m, 20.0, 1e-12);
        assert_close(i.tz_m, -30.0, 1e-12);
        assert_close(i.rx_arcsec, -2.0, 1e-12);
        assert_close(i.ry_arcsec, 3.0, 1e-12);
        assert_close(i.rz_arcsec, -4.0, 1e-12);
        assert_close(i.scale_ppm, -5.0, 1e-12);
    }

    #[test]
    fn helmert_inverse_round_trips() {
        let e = ecef(4_000_000.0, -2_000_000.0, 4_500_000.0);
        let back = FULL.inverse().apply_ecef(FULL.apply_ecef(e));
        // First-order inverse: sub-cm residual from the dropped second-order term.
        assert_close(back.x, e.x, 1e-2);
        assert_close(back.y, e.y, 1e-2);
        assert_close(back.z, e.z, 1e-2);
    }

    #[test]
    fn first_order_inverse_is_approximate_for_a_real_seven_parameter_set() {
        // WGS-84 → OSGB36 (EPSG:1314, position-vector convention): a published
        // full seven-parameter set, with the large scale and translation that
        // make the dropped cross terms visible. `Helmert::inverse` documents a
        // residual of about a centimeter for this set: nonzero, and under 2 cm.
        let osgb = Helmert {
            tx_m: -446.448,
            ty_m: 125.157,
            tz_m: -542.06,
            rx_arcsec: -0.1502,
            ry_arcsec: -0.247,
            rz_arcsec: -0.8421,
            scale_ppm: 20.4894,
        };
        let e = ecef(3_980_000.0, -10_000.0, 4_970_000.0); // near 51.5°N, 0°
        let back = osgb.inverse().apply_ecef(osgb.apply_ecef(e));
        let residual =
            ((back.x - e.x).powi(2) + (back.y - e.y).powi(2) + (back.z - e.z).powi(2)).sqrt();
        assert!(residual > 5e-3, "residual {residual} m should be visible");
        assert!(residual < 0.02, "residual {residual} m is over 2 cm");
    }

    #[test]
    fn catalog_has_the_three_classic_datums() {
        let nad27 = DatumTransform::to_wgs84(Crs::Nad27).expect("NAD27 catalogued");
        assert_eq!(nad27.from, Ellipsoid::CLARKE_1866);
        assert_eq!(nad27.to, Ellipsoid::WGS84);
        assert_close(nad27.helmert.tx_m, -8.0, 1e-12);
        assert_close(nad27.helmert.ty_m, 160.0, 1e-12);
        assert_close(nad27.helmert.tz_m, 176.0, 1e-12);

        let tokyo = DatumTransform::to_wgs84(Crs::Tokyo).expect("Tokyo catalogued");
        assert_eq!(tokyo.from, Ellipsoid::BESSEL_1841);
        assert_close(tokyo.helmert.tx_m, -148.0, 1e-12);
        assert_close(tokyo.helmert.ty_m, 507.0, 1e-12);
        assert_close(tokyo.helmert.tz_m, 685.0, 1e-12);

        let pulkovo = DatumTransform::to_wgs84(Crs::Pulkovo42).expect("Pulkovo catalogued");
        assert_eq!(pulkovo.from, Ellipsoid::KRASOVSKY_1940);
        assert_close(pulkovo.helmert.tx_m, 28.0, 1e-12);
        assert_close(pulkovo.helmert.ty_m, -130.0, 1e-12);
        assert_close(pulkovo.helmert.tz_m, -95.0, 1e-12);
    }

    #[test]
    fn non_helmert_systems_have_no_catalog_entry() {
        assert!(DatumTransform::to_wgs84(Crs::Wgs84).is_none());
        assert!(DatumTransform::to_wgs84(Crs::Gcj02).is_none());
        assert!(DatumTransform::to_wgs84(Crs::Bd09).is_none());
    }

    #[test]
    fn transform_nad27_to_wgs84_reference() {
        // Independent reference (Python): geodetic -> ECEF(Clarke66) -> shift ->
        // geodetic(WGS84) for (40°N, 100°W, 0 m). NAD27 → WGS84 in CONUS shifts
        // the longitude west by ~1.5″ and the ellipsoidal height down ~35 m.
        let dt = DatumTransform::to_wgs84(Crs::Nad27).unwrap();
        let nad27 = Coordinate::new(40.0, -100.0, Crs::Nad27).with_height(Height::Ellipsoidal(0.0));
        let w = dt.transform(nad27).unwrap();
        assert_eq!(w.crs, Crs::Wgs84);
        assert_close(w.lat, 40.000_009_482_759, 1e-8);
        assert_close(w.lon, -100.000_417_622_218_8, 1e-8);
        match w.height {
            Some(Height::Ellipsoidal(h)) => assert_close(h, -35.215_786_937_624_216, 1e-3),
            other => panic!("expected ellipsoidal height, got {other:?}"),
        }
    }

    #[test]
    fn transform_round_trips_through_inverse() {
        // Translation-only Helmert is exactly invertible, so a there-and-back
        // datum shift recovers the original geodetic coordinate.
        let dt = DatumTransform::to_wgs84(Crs::Tokyo).unwrap();
        let tokyo = Coordinate::new(35.0, 139.0, Crs::Tokyo).with_height(Height::Ellipsoidal(50.0));
        let w = dt.transform(tokyo).unwrap();
        let back = dt.inverse().transform(w).unwrap();
        assert_eq!(back.crs, Crs::Tokyo);
        assert_close(back.lat, 35.0, 1e-9);
        assert_close(back.lon, 139.0, 1e-9);
        match back.height {
            Some(Height::Ellipsoidal(h)) => assert_close(h, 50.0, 1e-4),
            other => panic!("expected ellipsoidal height, got {other:?}"),
        }
    }

    #[test]
    fn datum_transform_inverse_swaps_ellipsoids() {
        let dt = DatumTransform::to_wgs84(Crs::Nad27).unwrap();
        let inv = dt.inverse();
        assert_eq!(inv.from, Ellipsoid::WGS84);
        assert_eq!(inv.to, Ellipsoid::CLARKE_1866);
        assert_close(inv.helmert.tx_m, 8.0, 1e-12); // negated −8
        assert_close(inv.helmert.ty_m, -160.0, 1e-12);
        assert_close(inv.helmert.tz_m, -176.0, 1e-12);
    }

    #[test]
    fn transform_rejects_a_mismatched_source_crs_and_preserves_missing_height() {
        let dt = DatumTransform::to_wgs84(Crs::Nad27).unwrap();
        assert!(dt.transform(Coordinate::wgs84(40.0, -100.0)).is_err());
        let result = dt
            .transform(Coordinate::new(40.0, -100.0, Crs::Nad27))
            .unwrap();
        assert_eq!(result.crs, Crs::Wgs84);
        assert_eq!(result.height, None);
    }
}
