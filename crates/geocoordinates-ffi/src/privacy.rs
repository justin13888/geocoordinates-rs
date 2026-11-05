//! Location privacy: deterministic coarsening and exclusion zones.
//!
//! [`Precision`] and [`ExclusionZone`] are valid by construction in Rust, so they
//! cross as plain data checked on the way in, each with a `_validate` function.
//! `privacy::apply`, a name too generic for a flat foreign namespace, crosses as
//! [`apply_exclusion_zones`].

use geocoordinates as gc;

use crate::{Coordinate, Fix, GeoBounds, GeoError};

/// The discrete grid a [`Precision`] snaps to — mirror of
/// [`gc::privacy::SnapGrid`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum SnapGrid {
    /// Geohash cells, levels (code lengths) 1 to 12.
    Geohash,
    /// Uber H3 cells, resolutions 0 to 15.
    H3,
    /// Google S2 cells, levels 0 to 30.
    S2,
}

impl From<SnapGrid> for gc::privacy::SnapGrid {
    fn from(grid: SnapGrid) -> Self {
        match grid {
            SnapGrid::Geohash => Self::Geohash,
            SnapGrid::H3 => Self::H3,
            SnapGrid::S2 => Self::S2,
        }
    }
}

impl From<gc::privacy::SnapGrid> for SnapGrid {
    fn from(grid: gc::privacy::SnapGrid) -> Self {
        match grid {
            gc::privacy::SnapGrid::Geohash => Self::Geohash,
            gc::privacy::SnapGrid::H3 => Self::H3,
            gc::privacy::SnapGrid::S2 => Self::S2,
        }
    }
}

/// How coarse [`coarsen`] makes a position: a grid and a level on it — mirror of
/// [`gc::privacy::Precision`]. Plain data: every export taking one checks the
/// level against the grid, and [`precision_validate`] checks a hand-built one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Record)]
pub struct Precision {
    /// The grid to snap to.
    pub grid: SnapGrid,
    /// The geohash length, H3 resolution, or S2 level.
    pub level: u8,
}

impl From<gc::privacy::Precision> for Precision {
    fn from(precision: gc::privacy::Precision) -> Self {
        Self {
            grid: precision.grid().into(),
            level: precision.level(),
        }
    }
}

impl TryFrom<Precision> for gc::privacy::Precision {
    type Error = gc::Error;

    fn try_from(precision: Precision) -> Result<Self, gc::Error> {
        Self::new(precision.grid.into(), precision.level)
    }
}

/// Checks a hand-built [`Precision`] against its grid's levels.
///
/// # Errors
/// `GeoError::InvalidValue` (field `geohash length`, `H3 resolution` or
/// `S2 level`) when the grid does not define the level.
#[uniffi::export]
pub fn precision_validate(precision: Precision) -> Result<(), GeoError> {
    gc::privacy::Precision::try_from(precision)?;
    Ok(())
}

/// The finest geohash precision whose cells are at least `size_m` meters tall
/// everywhere (their width shrinks toward the poles).
///
/// # Errors
/// `GeoError::InvalidValue` (field `size`) when `size_m` is negative, NaN or
/// infinite, or taller than the coarsest geohash cell.
#[uniffi::export]
pub fn precision_at_least(size_m: f64) -> Result<Precision, GeoError> {
    Ok(gc::privacy::Precision::at_least(gc::Length::from_meters(size_m))?.into())
}

/// For a geohash precision, the north–south height in meters every one of its
/// cells has at least; `None` for H3 and S2.
///
/// # Errors
/// As [`precision_validate`], for an invalid [`Precision`] record.
#[uniffi::export]
pub fn precision_min_cell_height_m(precision: Precision) -> Result<Option<f64>, GeoError> {
    Ok(gc::privacy::Precision::try_from(precision)?
        .min_cell_height()
        .map(|height| height.meters()))
}

/// Snaps `coord` to the centre of the cell of `precision` holding it. The fix
/// has no height or timestamp, a `Bound` accuracy equal to the cell's
/// circumradius, and a source whose `raw` is `privacy:` and the cell.
///
/// # Errors
/// `GeoError::CrsMismatch` when `coord` is not on WGS-84; as
/// [`precision_validate`] for an invalid [`Precision`]; `GeoError::OutOfRange` /
/// `GeoError::InvalidValue` for an invalid coordinate record.
#[uniffi::export]
pub fn coarsen(coord: Coordinate, precision: Precision) -> Result<Fix, GeoError> {
    Ok(gc::privacy::coarsen(coord.try_into()?, precision.try_into()?)?.into())
}

/// A place whose fixes are never shared exactly — mirror of
/// [`gc::privacy::ExclusionZone`]. Plain data: every export taking one checks it,
/// and [`exclusion_zone_validate`] checks a hand-built one.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum ExclusionZone {
    /// Every point within `radius_m` meters (geodesic, edge included) of
    /// `center`, on `center`'s datum.
    Circle {
        /// The centre; not on GCJ-02 or BD-09.
        center: Coordinate,
        /// The radius in meters; finite and nonnegative.
        radius_m: f64,
    },
    /// Every point in a box, edges included.
    Bounds {
        /// The box.
        bounds: GeoBounds,
    },
}

impl TryFrom<ExclusionZone> for gc::privacy::ExclusionZone {
    type Error = gc::Error;

    fn try_from(zone: ExclusionZone) -> Result<Self, gc::Error> {
        match zone {
            ExclusionZone::Circle { center, radius_m } => {
                Self::from_circle(center.try_into()?, gc::Length::from_meters(radius_m))
            }
            ExclusionZone::Bounds { bounds } => Ok(Self::from_bounds(bounds.try_into()?)),
        }
    }
}

/// Checks a hand-built [`ExclusionZone`] against the core crate's rules.
///
/// # Errors
/// `GeoError::InvalidValue` (field `radius`) for a negative, NaN or infinite
/// radius; `GeoError::CrsMismatch` for a centre on GCJ-02 or BD-09; and the
/// coordinate's or box's own errors for an invalid record.
#[uniffi::export]
pub fn exclusion_zone_validate(zone: ExclusionZone) -> Result<(), GeoError> {
    gc::privacy::ExclusionZone::try_from(zone)?;
    Ok(())
}

/// Whether `point` lies in `zone`, edge included.
///
/// # Errors
/// `GeoError::CrsMismatch` when `point` is not on the zone's datum; as
/// [`exclusion_zone_validate`] for an invalid zone.
#[uniffi::export]
pub fn exclusion_zone_contains(zone: ExclusionZone, point: Coordinate) -> Result<bool, GeoError> {
    Ok(gc::privacy::ExclusionZone::try_from(zone)?.contains(point.try_into()?)?)
}

/// What [`apply_exclusion_zones`] does to a fix inside a zone — mirror of
/// [`gc::privacy::ZoneAction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum ZoneAction {
    /// Remove the fix.
    Strip,
    /// Replace the fix by its coarsened form at `precision`.
    Coarsen {
        /// The precision to coarsen to.
        precision: Precision,
    },
}

impl TryFrom<ZoneAction> for gc::privacy::ZoneAction {
    type Error = gc::Error;

    fn try_from(action: ZoneAction) -> Result<Self, gc::Error> {
        Ok(match action {
            ZoneAction::Strip => Self::Strip,
            ZoneAction::Coarsen { precision } => Self::Coarsen(precision.try_into()?),
        })
    }
}

/// The fix to share given exclusion `zones`: `fix` unchanged when it lies in no
/// zone, `None` when `action` is `Strip`, or the coarsened fix, which keeps the
/// timestamp, widens the bound by the fix's own horizontal accuracy, and drops
/// the height and source.
///
/// # Errors
/// `GeoError::CrsMismatch` when `fix` is not on the datum of every zone, or is
/// coarsened off WGS-84; the records' own errors for an invalid fix, zone or
/// precision.
#[uniffi::export]
pub fn apply_exclusion_zones(
    fix: Fix,
    zones: Vec<ExclusionZone>,
    action: ZoneAction,
) -> Result<Option<Fix>, GeoError> {
    let zones = zones
        .into_iter()
        .map(gc::privacy::ExclusionZone::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(gc::privacy::apply(fix.try_into()?, &zones, action.try_into()?)?.map(Into::into))
}
