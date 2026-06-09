//! Central runtime conversion dispatch over [`Crs`].
//!
//! This is the *runtime* path: it dispatches on a [`Crs`] tag and returns an
//! [`Approx`] because some target systems (GCJ-02/BD-09 inverses) are only
//! reachable approximately. When the source and target are known at compile
//! time, prefer the typed newtype conversions (e.g. [`crate::Wgs84`] →
//! [`crate::Gcj02`] via [`From`]), which return exact bare types where the math
//! is exact.

use crate::approx::Approx;
use crate::coord::{Coordinate, Crs};
use crate::error::Result;

/// Convert `coord` from its own [`Crs`] to `to`.
///
/// Routes through WGS-84 as the hub: China systems use the typed
/// [`china`](crate::china) conversions; classic datums (NAD27, Tokyo,
/// Pulkovo-1942) use the 7-parameter Helmert transforms from
/// [`geodesy::datum`](crate::geodesy::datum). The full EPSG/national-grid long
/// tail is delegated to the optional `proj` feature.
///
/// The result is wrapped in [`Approx`] because the worst-case path (e.g.
/// BD-09 → WGS-84) is approximate; for exact paths — including Helmert datum
/// shifts — the reported [`Approx::max_error_m`] is `0.0`.
///
/// # Errors
/// Returns [`crate::Error::UnsupportedConversion`] if no route is known between
/// the two systems.
pub fn convert(coord: Coordinate, to: Crs) -> Result<Approx<Coordinate>> {
    todo!(
        "dispatch coord.crs -> to via WGS-84 hub: china typed conversions, \
         datum::DatumTransform::to_wgs84 for classic datums, proj for the rest"
    )
}

/// Whether a conversion route exists between two reference systems.
#[must_use]
pub fn can_convert(from: Crs, to: Crs) -> bool {
    todo!()
}
