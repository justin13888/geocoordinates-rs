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
/// The result is wrapped in [`Approx`] because the worst-case path (e.g.
/// BD-09 → WGS-84) is approximate; for exact paths the reported
/// [`Approx::max_error_m`] is `0.0`.
///
/// # Errors
/// Returns [`crate::Error::UnsupportedConversion`] if no route is known between
/// the two systems.
pub fn convert(coord: Coordinate, to: Crs) -> Result<Approx<Coordinate>> {
    todo!("dispatch coord.crs -> to over the typed conversions; compose via WGS-84 as the hub")
}

/// Whether a conversion route exists between two reference systems.
#[must_use]
pub fn can_convert(from: Crs, to: Crs) -> bool {
    todo!()
}
