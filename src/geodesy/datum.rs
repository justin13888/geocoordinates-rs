//! Classic geodetic datum transforms via the 7-parameter Helmert
//! (Bursa-Wolf) model.
//!
//! Unlike the GCJ-02/BD-09 obfuscation transforms — whose inverses are
//! iterative and lossy — a Helmert transform is **exact** within its published
//! parameters: a rigid rotation, translation, and scale of the geocentric
//! (ECEF) frame. So these conversions return bare types, not
//! [`Approx`](crate::Approx).
//!
//! This module owns only the lightweight parametric path (a small catalog of
//! common datums: NAD27, Tokyo, Pulkovo-1942). Helmert (run through ECEF) is the
//! *only* parametric model offered: the abridged **Molodensky /
//! Molodensky-Badekas** transforms are deliberately omitted, as the ECEF Helmert
//! path is more general and at least as accurate. Higher-accuracy **grid-based**
//! transforms (NTv2, NADCON5), national grid projections, and the full EPSG
//! registry are out of scope here and are delegated to the optional `proj`
//! feature.
//!
//! Transforms are **static**. Epoch / time-aware geodesy — plate-motion velocity
//! models, the 14-parameter (rate-of-change) transforms, and distinct ITRF
//! realizations — is out of scope for v1; sub-centimeter, time-varying work is
//! not a goal here.

use core::f64::consts::PI;

use crate::coord::{Coordinate, Crs};
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

    /// The inverse transform (negated parameters; exact to first order in the
    /// small rotation angles, which is the standard Bursa-Wolf approximation).
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
    /// and for datums reachable only through the optional `proj` feature.
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
            from,
            to: Ellipsoid::WGS84,
            helmert,
        })
    }

    /// Transform a geodetic coordinate from the source to the target datum,
    /// tagging the result with `to`.
    ///
    /// Exact within the published parameters. `DatumTransform` holds only the
    /// two ellipsoids — which do not uniquely determine a [`Crs`] (e.g. GRS80
    /// backs both NAD83 and ETRS89) — so the target reference system is supplied
    /// explicitly rather than inferred.
    #[must_use]
    pub fn transform(&self, coord: Coordinate, to: Crs) -> Coordinate {
        let ecef = Ecef::from_coordinate(coord, self.from);
        let shifted = self.helmert.apply_ecef(ecef);
        // `Ecef::to_coordinate` is datum-agnostic and tags its output WGS-84;
        // re-tag with the caller-supplied target reference system.
        let mut result = shifted.to_coordinate(self.to);
        result.crs = to;
        result
    }

    /// The reverse transform (swaps ellipsoids and inverts the Helmert shift).
    #[must_use]
    pub fn inverse(&self) -> DatumTransform {
        DatumTransform {
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
        let w = dt.transform(nad27, Crs::Wgs84);
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
        let w = dt.transform(tokyo, Crs::Wgs84);
        let back = dt.inverse().transform(w, Crs::Tokyo);
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
}
