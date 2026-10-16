//! Discrete global grid indexing: Uber H3 and Google S2.
//!
//! These index a coordinate to a hierarchical cell for spatial joins, binning,
//! and proximity. Backed by external crates (`h3o`, `s2`) and gated behind the
//! `h3` / `s2` cargo features. A cell covers an area, so decoding a cell to a
//! representative point returns [`Approx`].
//!
use crate::approx::Approx;
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::geodesy::farthest_geodesic_m;
use crate::grids::require_wgs84;

/// An H3 cell index at a given resolution (0–15), valid by construction.
///
/// The index is private: build one with [`encode`](Self::encode), or from a
/// raw `u64` with [`new`](Self::new) (or `TryFrom<u64>`), which rejects an
/// index H3 does not define. Under the `serde` feature it serializes as the
/// bare `u64` and deserializing validates it the same way.
///
/// ```
/// use geocoordinates::Coordinate;
/// use geocoordinates::dgg::H3Cell;
///
/// let cell = H3Cell::encode(Coordinate::wgs84(40.7128, -74.006)?, 9)?;
/// assert_eq!(format!("{:x}", cell.as_u64()), "892a1072893ffff");
/// assert_eq!(H3Cell::new(cell.as_u64())?, cell);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[cfg(feature = "h3")]
#[cfg_attr(docsrs, doc(cfg(feature = "h3")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "u64", into = "u64")
)]
pub struct H3Cell(u64);

#[cfg(feature = "h3")]
impl H3Cell {
    /// Wrap a raw 64-bit H3 cell index, validating it.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::dgg::H3Cell;
    ///
    /// let cell = H3Cell::new(0x892a_1072_893f_fff)?;
    /// assert_eq!(cell.as_u64(), 0x892a_1072_893f_fff);
    /// assert!(matches!(H3Cell::new(0), Err(Error::InvalidCellId { grid: "H3", .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::InvalidCellId`] (`grid: "H3"`) when `value` is not a valid H3
    /// cell index (for example `0`, or a vertex / edge index).
    pub fn new(value: u64) -> Result<Self> {
        h3o::CellIndex::try_from(value)
            .map(|cell| Self(u64::from(cell)))
            .map_err(|_| Error::InvalidCellId { grid: "H3", value })
    }

    /// The raw 64-bit H3 cell index.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::dgg::H3Cell;
    ///
    /// let cell = H3Cell::encode(Coordinate::wgs84(40.7128, -74.006)?, 9)?;
    /// assert_eq!(cell.as_u64(), 617_733_151_020_810_239);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// The cell's resolution, 0 (coarsest) through 15 (finest): the
    /// `resolution` it was encoded at.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::dgg::H3Cell;
    ///
    /// let cell = H3Cell::encode(Coordinate::wgs84(40.7128, -74.006)?, 9)?;
    /// assert_eq!(cell.resolution(), 9);
    /// assert_eq!(H3Cell::new(0x8029_fff_ffff_ffff)?.resolution(), 0);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn resolution(self) -> u8 {
        let cell = h3o::CellIndex::try_from(self.0)
            .expect("H3Cell holds a validated index (checked in H3Cell::new)");
        u8::from(cell.resolution())
    }

    /// Encode (index) a WGS-84 coordinate to its H3 cell at `resolution`
    /// (0–15).
    ///
    /// ```
    /// use geocoordinates::dgg::H3Cell;
    /// use geocoordinates::{Coordinate, Error};
    ///
    /// let nyc = Coordinate::wgs84(40.7128, -74.006)?;
    /// assert_eq!(H3Cell::encode(nyc, 9)?.as_u64(), 0x892a_1072_893f_fff);
    /// assert!(matches!(H3Cell::encode(nyc, 16), Err(Error::InvalidValue { .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    /// [`Error::CrsMismatch`] for a non-WGS-84 coordinate, and
    /// [`Error::InvalidValue`] for a `resolution` above 15.
    pub fn encode(coord: Coordinate, resolution: u8) -> Result<Self> {
        use h3o::{LatLng, Resolution};

        require_wgs84(&coord)?;
        let res = Resolution::try_from(resolution).map_err(|_| Error::InvalidValue {
            field: "H3 resolution",
            detail: "must be in 0..=15".into(),
        })?;
        let cell = LatLng::new(coord.lat(), coord.lon())
            .map_err(|error| Error::InvalidValue {
                field: "coordinate",
                detail: error.to_string(),
            })?
            .to_cell(res);
        Ok(H3Cell(u64::from(cell)))
    }

    /// Decode to the cell's center coordinate; the error bound is the greatest
    /// ellipsoidal (WGS-84 geodesic) distance from the center to the cell's
    /// boundary vertices.
    ///
    /// Infallible: the index was validated when this value was built.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::dgg::H3Cell;
    ///
    /// let area = H3Cell::encode(Coordinate::wgs84(40.7128, -74.006)?, 9)?.decode();
    /// assert!((area.value().lat() - 40.7128).abs() < 0.003);
    /// assert!((205.0..206.0).contains(&area.max_error().meters()));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn decode(self) -> Approx<Coordinate> {
        use h3o::{CellIndex, LatLng};

        let cell = CellIndex::try_from(self.0)
            .expect("H3Cell holds a validated index (checked in H3Cell::new)");
        let center = LatLng::from(cell);
        // h3o yields latitude in [-90, 90] and longitude in [-180, 180].
        let center = Coordinate::from_valid(center.lat(), center.lng(), None, Crs::Wgs84);
        let radius = farthest_geodesic_m(
            &center,
            cell.boundary()
                .iter()
                .map(|vertex| (vertex.lat(), vertex.lng())),
        );
        Approx::from_valid(center, radius)
    }
}

#[cfg(feature = "h3")]
impl TryFrom<u64> for H3Cell {
    type Error = Error;

    /// Sugar for [`H3Cell::new`].
    fn try_from(value: u64) -> Result<Self> {
        Self::new(value)
    }
}

#[cfg(feature = "h3")]
impl From<H3Cell> for u64 {
    /// Sugar for [`H3Cell::as_u64`].
    fn from(cell: H3Cell) -> Self {
        cell.as_u64()
    }
}

/// An S2 cell id at a given level (0–30), valid by construction.
///
/// The id is private: build one with [`encode`](Self::encode), or from a raw
/// `u64` with [`new`](Self::new) (or `TryFrom<u64>`), which rejects an id S2
/// does not define. Under the `serde` feature it serializes as the bare `u64`
/// and deserializing validates it the same way.
///
/// ```
/// use geocoordinates::Coordinate;
/// use geocoordinates::dgg::S2CellId;
///
/// let cell = S2CellId::encode(Coordinate::wgs84(40.7128, -74.006)?, 20)?;
/// assert_eq!(S2CellId::new(cell.as_u64())?, cell);
/// assert!(cell.decode().max_error().meters() < 7.0);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[cfg(feature = "s2")]
#[cfg_attr(docsrs, doc(cfg(feature = "s2")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "u64", into = "u64")
)]
pub struct S2CellId(u64);

#[cfg(feature = "s2")]
impl S2CellId {
    /// Wrap a raw 64-bit S2 cell id, validating it.
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::dgg::S2CellId;
    ///
    /// let cell = S2CellId::new(9_926_595_630_970_437_632)?;
    /// assert_eq!(cell.as_u64(), 9_926_595_630_970_437_632);
    /// assert!(matches!(S2CellId::new(0), Err(Error::InvalidCellId { grid: "S2", .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::InvalidCellId`] (`grid: "S2"`) when `value` is not a valid S2
    /// cell id (for example `0`, or an id with a face above 5 or no trailing
    /// level marker bit).
    pub fn new(value: u64) -> Result<Self> {
        if s2::cellid::CellID(value).is_valid() {
            Ok(Self(value))
        } else {
            Err(Error::InvalidCellId { grid: "S2", value })
        }
    }

    /// The raw 64-bit S2 cell id.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::dgg::S2CellId;
    ///
    /// let cell = S2CellId::encode(Coordinate::wgs84(40.7128, -74.006)?, 20)?;
    /// assert_eq!(cell.as_u64(), 9_926_595_630_970_437_632);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// The cell's level, 0 (a whole cube face) through 30 (a leaf cell): the
    /// `level` it was encoded at.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::dgg::S2CellId;
    ///
    /// let cell = S2CellId::encode(Coordinate::wgs84(40.7128, -74.006)?, 20)?;
    /// assert_eq!(cell.level(), 20);
    /// assert_eq!(S2CellId::new(1 << 60)?.level(), 0); // face 0
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn level(self) -> u8 {
        // A valid id's level is at most 30, so it fits a `u8`.
        s2::cellid::CellID(self.0).level() as u8
    }

    /// Encode (index) a WGS-84 coordinate to its S2 cell at `level` (0–30).
    ///
    /// ```
    /// use geocoordinates::dgg::S2CellId;
    /// use geocoordinates::{Coordinate, Error};
    ///
    /// let nyc = Coordinate::wgs84(40.7128, -74.006)?;
    /// assert_eq!(S2CellId::encode(nyc, 20)?.as_u64(), 9_926_595_630_970_437_632);
    /// assert!(matches!(S2CellId::encode(nyc, 31), Err(Error::InvalidValue { .. })));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    ///
    /// # Errors
    /// [`Error::CrsMismatch`] for a non-WGS-84 coordinate, and
    /// [`Error::InvalidValue`] for a `level` above 30.
    pub fn encode(coord: Coordinate, level: u8) -> Result<Self> {
        use s2::cellid::CellID;
        use s2::latlng::LatLng;

        require_wgs84(&coord)?;
        if level > 30 {
            return Err(Error::InvalidValue {
                field: "S2 level",
                detail: "must be in 0..=30".into(),
            });
        }
        let leaf = CellID::from(LatLng::from_degrees(coord.lat(), coord.lon()));
        Ok(S2CellId(leaf.parent(u64::from(level)).0))
    }

    /// Decode to the cell's center coordinate; the error bound is the greatest
    /// ellipsoidal (WGS-84 geodesic) distance from the center to the cell's
    /// four actual vertices.
    ///
    /// Infallible: the id was validated when this value was built.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::dgg::S2CellId;
    ///
    /// let area = S2CellId::encode(Coordinate::wgs84(40.7128, -74.006)?, 20)?.decode();
    /// assert!((area.value().lat() - 40.7128).abs() < 1e-4);
    /// assert!((6.0..7.0).contains(&area.max_error().meters()));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn decode(self) -> Approx<Coordinate> {
        use s2::cell::Cell;
        use s2::cellid::CellID;
        use s2::latlng::LatLng;

        let cell = CellID(self.0);
        let center = LatLng::from(cell);
        // `LatLng` from a unit vector is `atan2`-based: latitude in [-90, 90],
        // longitude in [-180, 180].
        let center = Coordinate::from_valid(center.lat.deg(), center.lng.deg(), None, Crs::Wgs84);
        let radius = farthest_geodesic_m(
            &center,
            Cell::from(cell).vertices().iter().map(|vertex| {
                let vertex = LatLng::from(*vertex);
                (vertex.lat.deg(), vertex.lng.deg())
            }),
        );
        Approx::from_valid(center, radius)
    }
}

#[cfg(feature = "s2")]
impl TryFrom<u64> for S2CellId {
    type Error = Error;

    /// Sugar for [`S2CellId::new`].
    fn try_from(value: u64) -> Result<Self> {
        Self::new(value)
    }
}

#[cfg(feature = "s2")]
impl From<S2CellId> for u64 {
    /// Sugar for [`S2CellId::as_u64`].
    fn from(cell: S2CellId) -> Self {
        cell.as_u64()
    }
}

#[cfg(test)]
mod tests;
