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
        assert!(approx.max_error().meters() > 0.0);
        assert_within_geodesic_meters(
            approx.value(),
            &c(40.7128, -74.006),
            approx.max_error().meters(),
        );
    }
    // Finer resolutions give tighter bounds.
    assert!(
        H3Cell::encode(c(40.7128, -74.006), 12)
            .unwrap()
            .decode()
            .max_error()
            .meters()
            < H3Cell::encode(c(40.7128, -74.006), 5)
                .unwrap()
                .decode()
                .max_error()
                .meters()
    );
    // A resolution-9 cell's farthest boundary vertex is ≈ 205 m away.
    let bound9 = H3Cell::encode(c(40.7128, -74.006), 9)
        .unwrap()
        .decode()
        .max_error()
        .meters();
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
    assert_bound_is_ellipsoidal(approx.value(), &vertices, approx.max_error().meters());
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
        assert!(approx.max_error().meters() > 0.0);
        assert_within_geodesic_meters(
            approx.value(),
            &c(40.7128, -74.006),
            approx.max_error().meters(),
        );
    }
    assert!(
        S2CellId::encode(c(40.7128, -74.006), 25)
            .unwrap()
            .decode()
            .max_error()
            .meters()
            < S2CellId::encode(c(40.7128, -74.006), 5)
                .unwrap()
                .decode()
                .max_error()
                .meters()
    );
    // A level-20 cell's farthest corner is ≈ 6–7 m away.
    let bound20 = S2CellId::encode(c(40.7128, -74.006), 20)
        .unwrap()
        .decode()
        .max_error()
        .meters();
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
    assert_bound_is_ellipsoidal(approx.value(), &vertices, approx.max_error().meters());
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

/// Each cell reports the level it was encoded at, at every level, and hashes
/// by its id, so cells bin in a `HashSet`.
#[test]
fn cells_report_their_level_and_hash_by_id() {
    use std::collections::HashSet;

    let (nyc, sydney) = (c(40.7128, -74.006), c(-33.8688, 151.2093));
    #[cfg(feature = "h3")]
    for res in 0..=15 {
        let cell = H3Cell::encode(nyc, res).unwrap();
        assert_eq!(cell.resolution(), res);
        let far = H3Cell::encode(sydney, res).unwrap();
        assert_eq!(
            HashSet::from([cell, far, H3Cell::new(cell.0).unwrap()]).len(),
            2
        );
    }
    #[cfg(feature = "s2")]
    for level in 0..=30 {
        let cell = S2CellId::encode(nyc, level).unwrap();
        assert_eq!(cell.level(), level);
        let far = S2CellId::encode(sydney, level).unwrap();
        assert_eq!(
            HashSet::from([cell, far, S2CellId::new(cell.0).unwrap()]).len(),
            2
        );
    }
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
    let bound = approx.max_error().meters();
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
