//! Geodesic and rhumb-line navigation, ECEF, and local ENU/NED/AER frames.

use geocoordinates::geodesy::{
    Aer, Ecef, Ellipsoid, Enu, Ned, along_track_distance, cross_track_distance, destination,
    final_bearing, geodesic_distance, haversine_distance, initial_bearing, intermediate,
    intersection, midpoint, rhumb_bearing, rhumb_destination, rhumb_distance,
};
use geocoordinates::{Coordinate, Crs, Height};

use crate::support::{
    DemoResult, assert_aer_close, assert_close, assert_enu_close, assert_ned_close,
    assert_within_meters,
};

pub(crate) fn geodesics_and_local_frames() -> DemoResult {
    let equator_start = Coordinate::wgs84(0.0, 0.0)?;
    let equator_end = Coordinate::wgs84(0.0, 1.0)?;
    assert_close(
        geodesic_distance(&equator_start, &equator_end)?.meters(),
        111_319.49,
        0.5,
    );
    assert_close(
        haversine_distance(&equator_start, &equator_end)?.meters(),
        111_195.0,
        1.0,
    );
    assert_close(initial_bearing(&equator_start, &equator_end)?, 90.0, 1e-9);
    assert_close(final_bearing(&equator_start, &equator_end)?, 90.0, 1e-9);

    let route_start = Coordinate::wgs84(40.0, -75.0)?;
    let route_end = Coordinate::wgs84(41.0, -73.0)?;
    let direct = destination(
        &route_start,
        initial_bearing(&route_start, &route_end)?,
        geodesic_distance(&route_start, &route_end)?,
    )?;
    assert_within_meters(&direct, &route_end, 0.2);
    assert_close(
        midpoint(&Coordinate::wgs84(0.0, 0.0)?, &Coordinate::wgs84(0.0, 2.0)?)?.lon(),
        1.0,
        1e-6,
    );
    assert_close(
        intermediate(
            &Coordinate::wgs84(0.0, 0.0)?,
            &Coordinate::wgs84(0.0, 10.0)?,
            0.25,
        )?
        .lon(),
        2.5,
        1e-6,
    );
    let rhumb_end = Coordinate::wgs84(45.0, -70.0)?;
    let rhumb = rhumb_destination(
        &route_start,
        rhumb_bearing(&route_start, &rhumb_end)?,
        rhumb_distance(&route_start, &rhumb_end)?,
    )?;
    assert_within_meters(&rhumb, &rhumb_end, 0.2);

    let path_start = Coordinate::wgs84(0.0, 0.0)?;
    let path_end = Coordinate::wgs84(0.0, 10.0)?;
    let off_path = Coordinate::wgs84(1.0, 5.0)?;
    assert!(cross_track_distance(&off_path, &path_start, &path_end)?.meters() < 0.0);
    assert_close(
        along_track_distance(&off_path, &path_start, &path_end)?.meters(),
        5.0 * 111_195.0,
        2_000.0,
    );
    let crossing = intersection(
        &Coordinate::wgs84(0.0, -10.0)?,
        90.0,
        &Coordinate::wgs84(-10.0, 0.0)?,
        0.0,
    )?
    .expect("equator and prime meridian intersect");
    assert_within_meters(&crossing, &Coordinate::wgs84(0.0, 0.0)?, 0.02);

    for ellipsoid in [
        Ellipsoid::WGS84,
        Ellipsoid::GRS80,
        Ellipsoid::KRASOVSKY_1940,
        Ellipsoid::AIRY_1830,
        Ellipsoid::BESSEL_1841,
        Ellipsoid::CLARKE_1866,
    ] {
        assert!(ellipsoid.flattening() > 0.0);
        assert!(ellipsoid.semi_minor_m() < ellipsoid.semi_major_m);
        assert!((0.0..1.0).contains(&ellipsoid.eccentricity_sq()));
    }

    let origin = Coordinate::wgs84(40.0, -75.0)?.with_height(Height::Ellipsoidal(100.0))?;
    let target = Coordinate::wgs84(40.001, -74.999)?.with_height(Height::Ellipsoidal(125.0))?;
    let ecef = Ecef::try_from_coordinate(origin, Ellipsoid::WGS84)?;
    assert_within_meters(
        &ecef.try_to_coordinate(Ellipsoid::WGS84, Crs::Wgs84)?,
        &origin,
        0.001,
    );
    let rebuilt = Ecef::new(ecef.x, ecef.y, ecef.z);
    assert_eq!(rebuilt, ecef);

    let enu = Enu::try_from_coordinate(target, origin)?;
    assert_within_meters(&enu.try_to_coordinate(origin)?, &target, 0.001);
    let ned = enu.to_ned();
    let aer = enu.to_aer();
    assert_eq!(ned.to_enu(), enu);
    assert_within_meters(&ned.try_to_coordinate(origin)?, &target, 0.001);
    assert_within_meters(&aer.try_to_coordinate(origin)?, &target, 0.001);
    assert_enu_close(aer.to_enu(), enu);
    assert_ned_close(aer.to_ned(), ned);
    assert_aer_close(ned.to_aer(), aer);
    assert_eq!(Ned::try_from_coordinate(target, origin)?, ned);
    assert_aer_close(Aer::try_from_coordinate(target, origin)?, aer);
    let via_from_ned: Ned = enu.into();
    let via_from_aer: Aer = enu.into();
    let via_from_enu: Enu = via_from_ned.into();
    assert_eq!(via_from_ned, ned);
    assert_aer_close(via_from_aer, aer);
    assert_eq!(via_from_enu, enu);

    Ok("geodesics and local frames")
}
