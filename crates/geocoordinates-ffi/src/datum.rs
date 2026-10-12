//! Classic datums: Helmert (Bursa-Wolf) parameters and the datum transforms
//! built on them.

use geocoordinates as gc;

use crate::{Coordinate, Crs, Ecef, GeoError};

/// The seven Bursa-Wolf parameters of a Helmert transform — mirror of
/// [`gc::Helmert`](gc::geodesy::Helmert). Translations in meters, rotations in
/// arc-seconds (position-vector convention), scale in parts-per-million.
///
/// Unvalidated until used: [`helmert_apply_ecef`] and
/// [`helmert_inverse_first_order`] apply the parameters as given, and a
/// [`DatumTransform`] validates them (all finite) as it crosses in.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
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

/// A complete datum transform — mirror of
/// [`gc::DatumTransform`](gc::geodesy::DatumTransform): a source and a target
/// reference system, the Helmert shift between them, and the shift's
/// published accuracy. The ellipsoids are those of the two systems
/// ([`ellipsoid_for_crs`](crate::ellipsoid_for_crs)), never separate fields.
///
/// Unvalidated until used: every export that takes one validates it as the
/// core `DatumTransform::new` does (both systems geodetic datums rather than
/// GCJ-02 / BD-09, Helmert parameters finite, accuracy finite and
/// non-negative). Build one with [`datum_transform_new`] to fail at
/// construction instead.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct DatumTransform {
    /// Source reference system.
    pub from_crs: Crs,
    /// Target reference system.
    pub to_crs: Crs,
    /// Helmert parameters carrying the source frame to the target frame.
    pub helmert: Helmert,
    /// Accuracy of the parameters in meters, or `None` when no estimate is
    /// published (see [`datum_transform_catalog_to_wgs84`]).
    pub accuracy_m: Option<f64>,
}

impl From<gc::geodesy::Helmert> for Helmert {
    fn from(h: gc::geodesy::Helmert) -> Self {
        Helmert {
            tx_m: h.tx_m,
            ty_m: h.ty_m,
            tz_m: h.tz_m,
            rx_arcsec: h.rx_arcsec,
            ry_arcsec: h.ry_arcsec,
            rz_arcsec: h.rz_arcsec,
            scale_ppm: h.scale_ppm,
        }
    }
}
impl From<Helmert> for gc::geodesy::Helmert {
    fn from(h: Helmert) -> Self {
        gc::geodesy::Helmert {
            tx_m: h.tx_m,
            ty_m: h.ty_m,
            tz_m: h.tz_m,
            rx_arcsec: h.rx_arcsec,
            ry_arcsec: h.ry_arcsec,
            rz_arcsec: h.rz_arcsec,
            scale_ppm: h.scale_ppm,
        }
    }
}
impl From<gc::geodesy::DatumTransform> for DatumTransform {
    fn from(d: gc::geodesy::DatumTransform) -> Self {
        DatumTransform {
            from_crs: d.from_crs().into(),
            to_crs: d.to_crs().into(),
            helmert: d.helmert().into(),
            accuracy_m: d.accuracy_m(),
        }
    }
}
impl TryFrom<DatumTransform> for gc::geodesy::DatumTransform {
    type Error = GeoError;

    fn try_from(d: DatumTransform) -> Result<Self, GeoError> {
        gc::geodesy::DatumTransform::new(
            d.from_crs.into(),
            d.to_crs.into(),
            d.helmert.into(),
            d.accuracy_m,
        )
        .map_err(GeoError::from)
    }
}

/// The identity Helmert transform (no translation, rotation, or scale).
#[uniffi::export]
pub fn helmert_identity() -> Helmert {
    gc::geodesy::Helmert::IDENTITY.into()
}

/// Apply a Helmert transform to a geocentric (ECEF) position.
#[uniffi::export]
pub fn helmert_apply_ecef(helmert: Helmert, ecef: Ecef) -> Ecef {
    gc::geodesy::Helmert::from(helmert)
        .apply_ecef(ecef.into())
        .into()
}

/// The first-order inverse Helmert transform (negated parameters). Exact for
/// translation-only transforms, as every catalogued one is; otherwise
/// first-order, leaving about a centimeter of round-trip residual for a full
/// seven-parameter set such as WGS-84 → OSGB36.
#[uniffi::export]
pub fn helmert_inverse_first_order(helmert: Helmert) -> Helmert {
    gc::geodesy::Helmert::from(helmert)
        .inverse_first_order()
        .into()
}

/// Build a validated datum transform — the FFI form of `DatumTransform::new`.
///
/// # Errors
/// `GeoError::CrsMismatch` (expecting WGS-84) when either system is GCJ-02 or
/// BD-09; `GeoError::InvalidValue` (field `helmert`) for a non-finite
/// parameter, and (field `accuracy_m`) for a negative or non-finite accuracy.
#[uniffi::export]
pub fn datum_transform_new(
    from_crs: Crs,
    to_crs: Crs,
    helmert: Helmert,
    accuracy_m: Option<f64>,
) -> Result<DatumTransform, GeoError> {
    gc::geodesy::DatumTransform::try_from(DatumTransform {
        from_crs,
        to_crs,
        helmert,
        accuracy_m,
    })
    .map(Into::into)
}

/// The catalogued transform carrying `datum` to WGS-84, or `None` when none is
/// built in (WGS-84 itself and the China obfuscation systems).
///
/// Each is a NIMA TR8350.2 (3rd ed., Amendment 1) mean translation-only shift
/// whose `accuracy_m` is the root-sum-square of the published per-axis
/// figures: √86 ≈ 9.27 m for NAD27 (CONUS), √825 ≈ 28.72 m for Tokyo, and
/// `None` for Pulkovo-1942 (Russia), for which TR8350.2 publishes no estimate.
#[uniffi::export]
pub fn datum_transform_catalog_to_wgs84(datum: Crs) -> Option<DatumTransform> {
    gc::geodesy::DatumTransform::catalog_to_wgs84(datum.into()).map(Into::into)
}

/// Transform a geodetic coordinate from the source to the target datum,
/// tagging the result with `to_crs`. The computation is exact to round-off;
/// the result is only as accurate as the parameters (`accuracy_m`).
///
/// # Errors
/// `GeoError::CrsMismatch` when `coord` is not in `from_crs`;
/// `GeoError::InvalidValue` (field `height`) for an orthometric height; and
/// the errors of [`datum_transform_new`] for a malformed transform record.
#[uniffi::export]
pub fn datum_transform_apply(
    transform: DatumTransform,
    coord: Coordinate,
) -> Result<Coordinate, GeoError> {
    gc::geodesy::DatumTransform::try_from(transform)?
        .apply(coord.try_into()?)
        .map(Into::into)
        .map_err(Into::into)
}

/// The reverse datum transform: swaps the systems and takes the first-order
/// inverse of the Helmert shift (exact for translation-only shifts), keeping
/// the accuracy.
///
/// # Errors
/// The errors of [`datum_transform_new`] for a malformed transform record.
#[uniffi::export]
pub fn datum_transform_inverse_first_order(
    transform: DatumTransform,
) -> Result<DatumTransform, GeoError> {
    Ok(gc::geodesy::DatumTransform::try_from(transform)?
        .inverse_first_order()
        .into())
}
