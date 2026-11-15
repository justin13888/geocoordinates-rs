//! `from_corners`: which corner gives which edge, and the swapped-corner and
//! mixed-CRS errors.

use super::*;

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

#[test]
fn from_corners_reads_each_edge_from_its_corner_and_drops_heights() {
    let south_west = Coordinate::new(1.0, 2.0, Crs::Nad27)
        .unwrap()
        .with_height(crate::Height::Ellipsoidal(5.0))
        .unwrap();
    let north_east = Coordinate::new(3.0, -4.0, Crs::Nad27).unwrap();
    let b = GeoBounds::from_corners(south_west, north_east).unwrap();
    assert_eq!(
        (b.south(), b.west(), b.north(), b.east()),
        (1.0, 2.0, 3.0, -4.0)
    );
    assert_eq!(b.crs(), Crs::Nad27);
    assert!(b.crosses_antimeridian());
}

#[test]
fn from_corners_rejects_swapped_corners_and_mixed_crs() {
    let (sw, ne) = (wgs84(1.0, 2.0), wgs84(3.0, 4.0));
    assert!(matches!(
        GeoBounds::from_corners(ne, sw),
        Err(Error::InvalidValue {
            field: "south_west",
            ..
        })
    ));
    let gcj = Coordinate::gcj02(3.0, 4.0).unwrap();
    assert!(matches!(
        GeoBounds::from_corners(sw, gcj),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Gcj02
        })
    ));
    // The CRS check comes first, so swapped corners on two CRSs name the CRSs.
    assert!(matches!(
        GeoBounds::from_corners(gcj, sw),
        Err(Error::CrsMismatch {
            expected: Crs::Gcj02,
            found: Crs::Wgs84
        })
    ));
    // Equal latitudes are one parallel, not an error.
    assert!(GeoBounds::from_corners(wgs84(1.0, 2.0), wgs84(1.0, 4.0)).is_ok());
}
