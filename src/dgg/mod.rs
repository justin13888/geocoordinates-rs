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

fn validate_encoding_coordinate(coord: Coordinate) -> Result<()> {
    if coord.crs() != Crs::Wgs84 {
        return Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: coord.crs(),
        });
    }
    Ok(())
}

/// The greatest WGS-84 geodesic distance from `center` to any of `vertices`
/// (each a `(lat, lon)` pair in degrees).
#[cfg(any(feature = "h3", feature = "s2"))]
fn farthest_vertex_m(center: &Coordinate, vertices: impl IntoIterator<Item = (f64, f64)>) -> f64 {
    use geographiclib_rs::{Geodesic, InverseGeodesic};

    let geodesic = Geodesic::wgs84();
    vertices
        .into_iter()
        .map(|(lat, lon)| -> f64 { geodesic.inverse(center.lat(), center.lon(), lat, lon) })
        .fold(0.0, f64::max)
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

        validate_encoding_coordinate(coord)?;
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
    /// assert!((area.lat() - 40.7128).abs() < 0.003);
    /// assert!((205.0..206.0).contains(&area.max_error_m()));
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
        let radius = farthest_vertex_m(
            &center,
            cell.boundary()
                .iter()
                .map(|vertex| (vertex.lat(), vertex.lng())),
        );
        Approx::new(center, radius)
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
/// assert!(cell.decode().max_error_m() < 7.0);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[cfg(feature = "s2")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

        validate_encoding_coordinate(coord)?;
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
    /// assert!((area.lat() - 40.7128).abs() < 1e-4);
    /// assert!((6.0..7.0).contains(&area.max_error_m()));
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
        let radius = farthest_vertex_m(
            &center,
            Cell::from(cell).vertices().iter().map(|vertex| {
                let vertex = LatLng::from(*vertex);
                (vertex.lat.deg(), vertex.lng.deg())
            }),
        );
        Approx::new(center, radius)
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
mod tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::test_support::assert_within_geodesic_meters;

    fn c(lat: f64, lon: f64) -> Coordinate {
        Coordinate::wgs84(lat, lon).unwrap()
    }

    #[cfg(feature = "h3")]
    #[test]
    fn h3_matches_the_reference_index() {
        // (40.7128, −74.006) at resolution 9, per the canonical H3 library
        // (0x892a1072893ffff).
        let cell = H3Cell::encode(c(40.7128, -74.006), 9).unwrap();
        assert_eq!(cell.0, 617_733_151_020_810_239);
    }

    #[cfg(feature = "h3")]
    #[test]
    fn h3_round_trips_within_its_bound() {
        for res in [0u8, 5, 9, 12, 15] {
            let approx = H3Cell::encode(c(40.7128, -74.006), res).unwrap().decode();
            assert!(approx.max_error_m() > 0.0);
            assert_within_geodesic_meters(
                approx.value(),
                &c(40.7128, -74.006),
                approx.max_error_m(),
            );
        }
        // Finer resolutions give tighter bounds.
        assert!(
            H3Cell::encode(c(40.7128, -74.006), 12)
                .unwrap()
                .decode()
                .max_error_m()
                < H3Cell::encode(c(40.7128, -74.006), 5)
                    .unwrap()
                    .decode()
                    .max_error_m()
        );
        // A resolution-9 cell's farthest boundary vertex is ≈ 205 m away.
        let bound9 = H3Cell::encode(c(40.7128, -74.006), 9)
            .unwrap()
            .decode()
            .max_error_m();
        assert!((205.0..206.0).contains(&bound9), "res-9 bound {bound9}");
    }

    /// Asserts `bound` covers the ellipsoidal distance from `center` to every
    /// vertex, and that a spherical measure would have fallen short of it.
    #[cfg(any(feature = "h3", feature = "s2"))]
    fn assert_bound_is_ellipsoidal(center: &Coordinate, vertices: &[Coordinate], bound: f64) {
        use crate::geodesy::{geodesic_distance, haversine_distance};

        let farthest = |distance: fn(&Coordinate, &Coordinate) -> Result<crate::Length>| {
            vertices
                .iter()
                .map(|vertex| distance(center, vertex).unwrap().meters())
                .fold(0.0, f64::max)
        };
        let ellipsoidal = farthest(geodesic_distance);
        let spherical = farthest(haversine_distance);
        assert!(
            bound >= ellipsoidal,
            "bound {bound} < geodesic {ellipsoidal}"
        );
        assert!(
            spherical < ellipsoidal,
            "spherical {spherical} does not under-state geodesic {ellipsoidal}"
        );
    }

    #[cfg(feature = "h3")]
    #[test]
    fn h3_bound_covers_the_ellipsoidal_vertex_distance() {
        // Near the pole the ellipsoid's meridional curvature radius exceeds the
        // mean sphere's by about 0.5%, so a haversine bound falls short there.
        let cell = H3Cell::encode(c(89.5, 10.0), 5).unwrap();
        let approx = cell.decode();
        let vertices: Vec<_> = h3o::CellIndex::try_from(cell.0)
            .unwrap()
            .boundary()
            .iter()
            .map(|vertex| c(vertex.lat(), vertex.lng()))
            .collect();
        assert_bound_is_ellipsoidal(approx.value(), &vertices, approx.max_error_m());
    }

    #[cfg(feature = "s2")]
    #[test]
    fn s2_matches_the_reference_id() {
        // (40.7128, −74.006) at level 20, per the canonical S2 library — this
        // also confirms the `s2` crate indexes correctly.
        let cell = S2CellId::encode(c(40.7128, -74.006), 20).unwrap();
        assert_eq!(cell.0, 9_926_595_630_970_437_632);
    }

    #[cfg(feature = "s2")]
    #[test]
    fn s2_round_trips_within_its_bound() {
        for level in [1u8, 10, 20, 30] {
            let approx = S2CellId::encode(c(40.7128, -74.006), level)
                .unwrap()
                .decode();
            assert!(approx.max_error_m() > 0.0);
            assert_within_geodesic_meters(
                approx.value(),
                &c(40.7128, -74.006),
                approx.max_error_m(),
            );
        }
        assert!(
            S2CellId::encode(c(40.7128, -74.006), 25)
                .unwrap()
                .decode()
                .max_error_m()
                < S2CellId::encode(c(40.7128, -74.006), 5)
                    .unwrap()
                    .decode()
                    .max_error_m()
        );
        // A level-20 cell's farthest corner is ≈ 6–7 m away.
        let bound20 = S2CellId::encode(c(40.7128, -74.006), 20)
            .unwrap()
            .decode()
            .max_error_m();
        assert!((6.0..7.0).contains(&bound20), "level-20 bound {bound20}");
    }

    #[cfg(feature = "s2")]
    #[test]
    fn s2_bound_covers_the_ellipsoidal_vertex_distance() {
        use s2::cell::Cell;
        use s2::cellid::CellID;
        use s2::latlng::LatLng;

        // Near the pole the ellipsoid's meridional curvature radius exceeds the
        // mean sphere's by about 0.5%, so a haversine bound falls short there.
        let cell = S2CellId::encode(c(89.5, 10.0), 12).unwrap();
        let approx = cell.decode();
        let vertices: Vec<_> = Cell::from(CellID(cell.0))
            .vertices()
            .iter()
            .map(|vertex| {
                let vertex = LatLng::from(*vertex);
                c(vertex.lat.deg(), vertex.lng.deg())
            })
            .collect();
        assert_bound_is_ellipsoidal(approx.value(), &vertices, approx.max_error_m());
    }

    #[test]
    fn invalid_ids_and_levels_are_rejected() {
        #[cfg(feature = "h3")]
        {
            assert!(H3Cell::encode(c(0.0, 0.0), 16).is_err());
            assert!(matches!(
                H3Cell::new(0),
                Err(Error::InvalidCellId {
                    grid: "H3",
                    value: 0
                })
            ));
            assert!(matches!(
                H3Cell::encode(Coordinate::gcj02(39.9, 116.4).unwrap(), 9),
                Err(Error::CrsMismatch {
                    expected: crate::Crs::Wgs84,
                    found: crate::Crs::Gcj02,
                })
            ));
        }
        #[cfg(feature = "s2")]
        {
            assert!(S2CellId::encode(c(0.0, 0.0), 31).is_err());
            assert!(matches!(
                S2CellId::new(0),
                Err(Error::InvalidCellId {
                    grid: "S2",
                    value: 0
                })
            ));
            assert!(matches!(
                S2CellId::encode(Coordinate::bd09(39.9, 116.4).unwrap(), 20),
                Err(Error::CrsMismatch {
                    expected: crate::Crs::Wgs84,
                    found: crate::Crs::Bd09,
                })
            ));
        }
    }

    #[cfg(feature = "h3")]
    #[test]
    fn h3_raw_ids_are_validated_and_round_trip() {
        const NYC_RES9: u64 = 617_733_151_020_810_239;
        let cell = H3Cell::new(NYC_RES9).unwrap();
        assert_eq!(cell.as_u64(), NYC_RES9);
        assert_eq!(u64::from(cell), NYC_RES9);
        assert_eq!(H3Cell::try_from(NYC_RES9).unwrap(), cell);
        assert_eq!(cell, H3Cell::encode(c(40.7128, -74.006), 9).unwrap());
        assert!(matches!(
            H3Cell::try_from(u64::MAX),
            Err(Error::InvalidCellId {
                grid: "H3",
                value: u64::MAX
            })
        ));
    }

    #[cfg(feature = "s2")]
    #[test]
    fn s2_raw_ids_are_validated_and_round_trip() {
        const NYC_LEVEL20: u64 = 9_926_595_630_970_437_632;
        let cell = S2CellId::new(NYC_LEVEL20).unwrap();
        assert_eq!(cell.as_u64(), NYC_LEVEL20);
        assert_eq!(u64::from(cell), NYC_LEVEL20);
        assert_eq!(S2CellId::try_from(NYC_LEVEL20).unwrap(), cell);
        assert_eq!(cell, S2CellId::encode(c(40.7128, -74.006), 20).unwrap());
        assert!(matches!(
            S2CellId::try_from(u64::MAX),
            Err(Error::InvalidCellId {
                grid: "S2",
                value: u64::MAX
            })
        ));
    }

    #[cfg(all(feature = "serde", feature = "h3"))]
    #[test]
    fn h3_serde_is_the_bare_index_and_validates() {
        let cell = H3Cell::encode(c(40.7128, -74.006), 9).unwrap();
        let json = serde_json::to_string(&cell).unwrap();
        assert_eq!(json, "617733151020810239");
        assert_eq!(serde_json::from_str::<H3Cell>(&json).unwrap(), cell);
        assert!(serde_json::from_str::<H3Cell>("0").is_err());
    }

    #[cfg(all(feature = "serde", feature = "s2"))]
    #[test]
    fn s2_serde_is_the_bare_id_and_validates() {
        let cell = S2CellId::encode(c(40.7128, -74.006), 20).unwrap();
        let json = serde_json::to_string(&cell).unwrap();
        assert_eq!(json, "9926595630970437632");
        assert_eq!(serde_json::from_str::<S2CellId>(&json).unwrap(), cell);
        assert!(serde_json::from_str::<S2CellId>("0").is_err());
    }

    /// Points on both poles and on either side of the antimeridian.
    #[cfg(any(feature = "h3", feature = "s2"))]
    const BOUNDARY_POINTS: [(f64, f64); 5] = [
        (90.0, 0.0),
        (-90.0, 0.0),
        (0.0, 180.0),
        (0.0, -180.0),
        (-16.5, 179.999_9),
    ];

    /// Asserts a decoded cell holds `point` within its bound, and that the
    /// bound is a real (positive, finite) distance.
    #[cfg(any(feature = "h3", feature = "s2"))]
    fn assert_decodes_around(approx: &Approx<Coordinate>, point: &Coordinate) {
        let bound = approx.max_error_m();
        assert!(bound > 0.0 && bound.is_finite(), "bound {bound}");
        assert_eq!(approx.value().crs(), Crs::Wgs84);
        assert_within_geodesic_meters(approx.value(), point, bound);
    }

    #[cfg(feature = "h3")]
    #[test]
    fn h3_decodes_at_the_poles_and_antimeridian() {
        for (lat, lon) in BOUNDARY_POINTS {
            for res in [0u8, 7, 15] {
                let point = c(lat, lon);
                let approx = H3Cell::encode(point, res).unwrap().decode();
                assert_decodes_around(&approx, &point);
            }
        }
    }

    #[cfg(feature = "s2")]
    #[test]
    fn s2_decodes_at_the_poles_and_antimeridian() {
        for (lat, lon) in BOUNDARY_POINTS {
            for level in [0u8, 15, 30] {
                let point = c(lat, lon);
                let approx = S2CellId::encode(point, level).unwrap().decode();
                assert_decodes_around(&approx, &point);
            }
        }
    }
}
