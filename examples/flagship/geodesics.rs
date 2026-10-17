//! Geodesic and rhumb-line navigation, ECEF, and local ENU/NED/AER frames.

use geocoordinates::geodesy::{
    Aer, Ecef, Ellipsoid, Enu, Intersection, Ned, along_track_distance_spherical,
    cross_track_distance_spherical, destination, final_bearing, geodesic_distance,
    haversine_distance, initial_bearing, intermediate, intersection_spherical, midpoint,
    rhumb_bearing, rhumb_destination, rhumb_distance,
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
        geodesic_distance(equator_start, equator_end)?.meters(),
        111_319.49,
        0.5,
    );
    assert_close(
        haversine_distance(equator_start, equator_end)?.meters(),
        111_195.0,
        1.0,
    );
    assert_close(initial_bearing(equator_start, equator_end)?, 90.0, 1e-9);
    assert_close(final_bearing(equator_start, equator_end)?, 90.0, 1e-9);

    let route_start = Coordinate::wgs84(40.0, -75.0)?;
    let route_end = Coordinate::wgs84(41.0, -73.0)?;
    let direct = destination(
        route_start,
        initial_bearing(route_start, route_end)?,
        geodesic_distance(route_start, route_end)?,
    )?;
    assert_within_meters(direct, route_end, 0.2);
    assert_close(
        midpoint(Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 2.0)?)?.lon(),
        1.0,
        1e-6,
    );
    assert_close(
        intermediate(
            Coordinate::wgs84(0.0, 0.0)?,
            Coordinate::wgs84(0.0, 10.0)?,
            0.25,
        )?
        .lon(),
        2.5,
        1e-6,
    );
    let rhumb_end = Coordinate::wgs84(45.0, -70.0)?;
    let rhumb = rhumb_destination(
        route_start,
        rhumb_bearing(route_start, rhumb_end)?,
        rhumb_distance(route_start, rhumb_end)?,
    )?;
    assert_within_meters(rhumb, rhumb_end, 0.2);

    let path_start = Coordinate::wgs84(0.0, 0.0)?;
    let path_end = Coordinate::wgs84(0.0, 10.0)?;
    let off_path = Coordinate::wgs84(1.0, 5.0)?;
    assert!(cross_track_distance_spherical(off_path, path_start, path_end)?.meters() < 0.0);
    assert_close(
        along_track_distance_spherical(off_path, path_start, path_end)?.meters(),
        5.0 * 111_195.0,
        2_000.0,
    );
    // A path from one point to itself fixes no great circle.
    assert!(cross_track_distance_spherical(off_path, path_start, path_start).is_err());
    let Intersection::Crossing(crossing) = intersection_spherical(
        Coordinate::wgs84(0.0, -10.0)?,
        90.0,
        Coordinate::wgs84(-10.0, 0.0)?,
        0.0,
    )?
    else {
        panic!("equator and prime meridian intersect");
    };
    assert_within_meters(crossing, Coordinate::wgs84(0.0, 0.0)?, 0.02);
    // From a pole, a bearing follows the meridian of the given longitude: 180°
    // from (90, 10) runs down 10°E to the equator.
    let Intersection::Crossing(from_pole) = intersection_spherical(
        Coordinate::wgs84(90.0, 10.0)?,
        180.0,
        Coordinate::wgs84(0.0, 0.0)?,
        90.0,
    )?
    else {
        panic!("the meridian meets the equator");
    };
    assert_within_meters(from_pole, Coordinate::wgs84(0.0, 10.0)?, 0.02);
    assert_eq!(
        intersection_spherical(
            Coordinate::wgs84(0.0, 0.0)?,
            90.0,
            Coordinate::wgs84(0.0, 5.0)?,
            90.0
        )?,
        Intersection::SameGreatCircle
    );
    // Geodesics are measured on the CRS's own ellipsoid: a Tokyo-datum pair on
    // Bessel 1841, which differs from the same numbers on WGS-84.
    let tokyo = |lat, lon| Coordinate::new(lat, lon, Crs::Tokyo);
    let on_bessel = geodesic_distance(tokyo(35.0, 139.0)?, tokyo(36.0, 140.0)?)?;
    let on_wgs84 = geodesic_distance(
        Coordinate::wgs84(35.0, 139.0)?,
        Coordinate::wgs84(36.0, 140.0)?,
    )?;
    assert!((on_wgs84.meters() - on_bessel.meters()).abs() > 1.0);
    // The same place has no bearing.
    assert!(initial_bearing(route_start, route_start).is_err());

    for ellipsoid in [
        Ellipsoid::WGS84,
        Ellipsoid::GRS80,
        Ellipsoid::KRASOVSKY_1940,
        Ellipsoid::AIRY_1830,
        Ellipsoid::BESSEL_1841,
        Ellipsoid::CLARKE_1866,
    ] {
        assert!(ellipsoid.flattening() > 0.0);
        assert!(ellipsoid.semi_minor() < ellipsoid.semi_major());
        assert!((0.0..1.0).contains(&ellipsoid.eccentricity_sq()));
        // Valid by construction: rebuilding from the parameters round-trips.
        assert_eq!(
            Ellipsoid::new(ellipsoid.semi_major(), ellipsoid.inverse_flattening())?,
            ellipsoid
        );
    }
    // Every geodetic datum names its own ellipsoid; ECEF and datum transforms
    // derive it from the coordinate's CRS rather than taking it as an argument.
    assert_eq!(Ellipsoid::for_crs(Crs::Nad27), Some(Ellipsoid::CLARKE_1866));

    let origin = Coordinate::wgs84(40.0, -75.0)?.with_height(Height::Ellipsoidal(100.0))?;
    let target = Coordinate::wgs84(40.001, -74.999)?.with_height(Height::Ellipsoidal(125.0))?;
    let ecef = Ecef::try_from_coordinate(origin)?;
    // The inverse is exact to round-off at any altitude.
    assert_within_meters(ecef.try_to_coordinate(Crs::Wgs84)?, origin, 1e-6);
    let gnss = Coordinate::wgs84(45.0, 10.0)?.with_height(Height::Ellipsoidal(20_200_000.0))?;
    let gnss_back = Ecef::try_from_coordinate(gnss)?.try_to_coordinate(Crs::Wgs84)?;
    assert_close(gnss_back.lat(), 45.0, 1e-12);
    let rebuilt = Ecef::new(ecef.x_m, ecef.y_m, ecef.z_m);
    assert_eq!(rebuilt, ecef);

    let enu = Enu::try_from_coordinate(target, origin)?;
    assert_within_meters(enu.try_to_coordinate(origin)?, target, 0.001);
    let ned = enu.to_ned();
    let aer = enu.to_aer();
    assert_eq!(ned.to_enu(), enu);
    assert_within_meters(ned.try_to_coordinate(origin)?, target, 0.001);
    assert_within_meters(aer.try_to_coordinate(origin)?, target, 0.001);
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
    // Frames work on any classic datum both points share, and are valid by
    // construction.
    let nad27_origin = Coordinate::new(origin.lat(), origin.lon(), Crs::Nad27)?;
    let nad27_target = Coordinate::new(target.lat(), target.lon(), Crs::Nad27)?;
    let nad27_enu = Enu::try_from_coordinate(nad27_target, nad27_origin)?;
    assert_within_meters(
        nad27_enu.try_to_coordinate(nad27_origin)?,
        nad27_target,
        0.001,
    );
    assert_eq!(Enu::new(enu.east_m(), enu.north_m(), enu.up_m())?, enu);
    assert!(Aer::new(0.0, 90.5, aer.range()).is_err());

    Ok("geodesics and local frames")
}
