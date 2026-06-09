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
//! common datums: NAD27, Tokyo, Pulkovo-1942). Higher-accuracy **grid-based**
//! transforms (NTv2, NADCON5), national grid projections, and the full EPSG
//! registry are out of scope here and are delegated to the optional `proj`
//! feature.

use crate::coord::{Coordinate, Crs};
use crate::geodesy::ecef::Ecef;
use crate::geodesy::ellipsoid::Ellipsoid;

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
    #[must_use]
    pub fn apply_ecef(&self, ecef: Ecef) -> Ecef {
        todo!("position-vector 7-param: scale·R·x + T")
    }

    /// The inverse transform (negated parameters; exact to first order in the
    /// small rotation angles, which is the standard Bursa-Wolf approximation).
    #[must_use]
    pub fn inverse(&self) -> Helmert {
        todo!("negate all seven parameters")
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
        todo!("catalog: Nad27, Tokyo, Pulkovo42 -> Wgs84; None otherwise")
    }

    /// Transform a geodetic coordinate from the source to the target datum.
    ///
    /// Exact within the published parameters; the result carries the target
    /// [`Crs`].
    #[must_use]
    pub fn transform(&self, coord: Coordinate) -> Coordinate {
        todo!("geodetic->ECEF(from) -> helmert -> ECEF->geodetic(to)")
    }

    /// The reverse transform (swaps ellipsoids and inverts the Helmert shift).
    #[must_use]
    pub fn inverse(&self) -> DatumTransform {
        todo!("swap from/to; helmert.inverse()")
    }
}
