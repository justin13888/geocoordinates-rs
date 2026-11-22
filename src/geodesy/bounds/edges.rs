//! [`GeoBoundsEdges`], the named-edge input of [`GeoBounds::from_edges`].

use super::GeoBounds;
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};

/// The four edges and CRS of a box, unchecked: the input of
/// [`GeoBounds::from_edges`], in decimal degrees on `crs`.
///
/// Every edge is a named field, so a struct literal cannot put them in the
/// wrong order the way four positional `f64`s can (GeoJSON's `bbox`,
/// Nominatim's `boundingbox` and Overpass each order them differently). Like
/// [`Helmert`](crate::geodesy::Helmert), the record is exhaustive with public fields:
/// its field set is fixed by definition and it holds no invariant, so it is
/// validated where it is consumed, by `from_edges`. It is also the serde wire
/// shape of a [`GeoBounds`].
///
/// ```
/// use geocoordinates::{Crs, Error, geodesy::{GeoBounds, GeoBoundsEdges}};
///
/// let edges = GeoBoundsEdges { south: -21.0, west: 176.0, north: -12.0, east: -178.0, crs: Crs::Wgs84 };
/// let fiji = GeoBounds::try_from(edges)?;
/// assert!(fiji.crosses_antimeridian());
///
/// // The record itself holds any value; the constructor checks it.
/// let bad = GeoBoundsEdges { east: 181.0, ..edges };
/// assert!(matches!(GeoBounds::try_from(bad), Err(Error::InvalidValue { field: "east", .. })));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeoBoundsEdges {
    /// The south edge: the latitude of the box's southern boundary, in `[-90, 90]`.
    pub south: f64,
    /// The west edge: the longitude of the box's western boundary, in `[-180, 180]`.
    pub west: f64,
    /// The north edge: the latitude of the box's northern boundary, in `[-90, 90]`.
    pub north: f64,
    /// The east edge: the longitude of the box's eastern boundary, in
    /// `[-180, 180]`; less than `west` for a box that crosses the antimeridian.
    pub east: f64,
    /// The CRS the edges are on.
    pub crs: Crs,
}

impl TryFrom<GeoBoundsEdges> for GeoBounds {
    type Error = Error;

    /// Sugar for [`GeoBounds::from_edges`].
    fn try_from(edges: GeoBoundsEdges) -> Result<Self> {
        Self::from_edges(edges)
    }
}

impl GeoBounds {
    /// A box from its four named edges on `edges.crs`, in decimal degrees,
    /// through [`from_corners`](Self::from_corners): `south` and `west` are the
    /// south-west corner, `north` and `east` the north-east corner.
    ///
    /// A `west` greater than `east` crosses the antimeridian, not an error.
    ///
    /// # Errors
    ///
    /// Each edge is one axis, so an edge that is non-finite or out of range
    /// (`[-90, 90]` for `south` and `north`, `[-180, 180]` for `west` and
    /// `east`) is [`Error::InvalidValue`] naming it (`field: "south"`, `"west"`,
    /// `"north"` or `"east"`), checked before the corners are built. A `south`
    /// north of `north` is the `field: "south_west"` error of `from_corners`.
    ///
    /// ```
    /// use geocoordinates::{Crs, Error, geodesy::{GeoBounds, GeoBoundsEdges}};
    ///
    /// let fiji = GeoBounds::from_edges(GeoBoundsEdges {
    ///     south: -21.0,
    ///     west: 176.0,
    ///     north: -12.0,
    ///     east: -178.0,
    ///     crs: Crs::Wgs84,
    /// })?;
    /// assert!(fiji.crosses_antimeridian());
    ///
    /// let unit = GeoBoundsEdges { south: 0.0, west: 0.0, north: 1.0, east: 1.0, crs: Crs::Wgs84 };
    /// assert!(matches!(
    ///     GeoBounds::from_edges(GeoBoundsEdges { east: 181.0, ..unit }),
    ///     Err(Error::InvalidValue { field: "east", .. })
    /// ));
    /// assert!(matches!(
    ///     GeoBounds::from_edges(GeoBoundsEdges { south: 1.0, north: 0.0, ..unit }),
    ///     Err(Error::InvalidValue { field: "south_west", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn from_edges(edges: GeoBoundsEdges) -> Result<Self> {
        let GeoBoundsEdges {
            south,
            west,
            north,
            east,
            crs,
        } = edges;
        for (field, value, limit) in [
            ("south", south, 90.0),
            ("west", west, 180.0),
            ("north", north, 90.0),
            ("east", east, 180.0),
        ] {
            if !value.is_finite() || value.abs() > limit {
                return Err(Error::InvalidValue {
                    field,
                    detail: format!("{value} is not a finite value in [-{limit}, {limit}]"),
                });
            }
        }
        Self::from_corners(
            Coordinate::new(south, west, crs)?,
            Coordinate::new(north, east, crs)?,
        )
    }
}
