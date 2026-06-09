//! Geoid models for ellipsoidal ↔ orthometric height conversion.
//!
//! GNSS reports **ellipsoidal** height; humans expect **orthometric** height
//! (above the geoid / sea level). The difference is the geoid undulation `N`,
//! looked up from a model (EGM96/EGM2008). Cargo feature `geoid`; requires
//! bundled or loaded grid data.

use crate::coord::Coordinate;
use crate::error::Result;

/// A geoid model providing undulation lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GeoidModel {
    /// EGM96 (15′ grid).
    Egm96,
    /// EGM2008 (1′ grid, higher resolution).
    Egm2008,
}

/// Geoid undulation `N` (meters) at a location for the given model.
///
/// `orthometric = ellipsoidal − N`.
///
/// # Errors
/// Returns an error if the model's grid data is not loaded or the point is out
/// of coverage.
pub fn undulation(coord: &Coordinate, model: GeoidModel) -> Result<f64> {
    todo!("TODO: load EGM grid; bilinear-interpolate N")
}

/// Convert a coordinate's ellipsoidal height to orthometric (above the geoid).
///
/// # Errors
/// Propagates [`undulation`] errors.
pub fn to_orthometric(coord: Coordinate, model: GeoidModel) -> Result<Coordinate> {
    todo!()
}

/// Convert a coordinate's orthometric height to ellipsoidal.
///
/// # Errors
/// Propagates [`undulation`] errors.
pub fn to_ellipsoidal(coord: Coordinate, model: GeoidModel) -> Result<Coordinate> {
    todo!()
}
