use super::*;

// ----- TEST-003: an out-of-range position never reaches an entry point -----

#[test]
fn measurements_cannot_receive_an_out_of_range_position_on_either_side() {
    let ok = c(0.0, 0.0);
    for (lat, lon) in [(91.0, 0.0), (-90.5, 0.0), (0.0, 181.0), (0.0, -180.5)] {
        // Construction itself rejects the position, carrying its components,
        // so no measurement below can ever be handed it.
        let bad = || Coordinate::wgs84(lat, lon);
        assert_out_of_range(bad(), lat, lon);
        assert_out_of_range(bad().and_then(|b| geodesic_distance(&b, &ok)), lat, lon);
        assert_out_of_range(bad().and_then(|b| geodesic_distance(&ok, &b)), lat, lon);
        assert_out_of_range(bad().and_then(|b| haversine_distance(&b, &ok)), lat, lon);
        assert_out_of_range(bad().and_then(|b| haversine_distance(&ok, &b)), lat, lon);
        assert_out_of_range(bad().and_then(|b| initial_bearing(&b, &ok)), lat, lon);
        assert_out_of_range(bad().and_then(|b| initial_bearing(&ok, &b)), lat, lon);
        assert_out_of_range(bad().and_then(|b| final_bearing(&b, &ok)), lat, lon);
        assert_out_of_range(bad().and_then(|b| final_bearing(&ok, &b)), lat, lon);
        assert_out_of_range(bad().and_then(|b| rhumb_distance(&b, &ok)), lat, lon);
        assert_out_of_range(bad().and_then(|b| rhumb_distance(&ok, &b)), lat, lon);
        assert_out_of_range(bad().and_then(|b| rhumb_bearing(&b, &ok)), lat, lon);
        assert_out_of_range(bad().and_then(|b| rhumb_bearing(&ok, &b)), lat, lon);
        let end = c(0.0, 10.0);
        assert_out_of_range(
            bad().and_then(|b| cross_track_distance_spherical(&b, &ok, &end)),
            lat,
            lon,
        );
        assert_out_of_range(
            bad().and_then(|b| cross_track_distance_spherical(&end, &ok, &b)),
            lat,
            lon,
        );
        assert_out_of_range(
            bad().and_then(|b| along_track_distance_spherical(&b, &ok, &end)),
            lat,
            lon,
        );
        assert_out_of_range(
            bad().and_then(|b| along_track_distance_spherical(&end, &ok, &b)),
            lat,
            lon,
        );
    }
    // NaN components are out of range too (the range check is not `!(x > 90)`).
    assert!(matches!(
        Coordinate::wgs84(f64::NAN, 0.0).and_then(|b| geodesic_distance(&b, &ok)),
        Err(Error::OutOfRange { .. })
    ));
}

#[test]
fn producers_cannot_receive_an_out_of_range_position() {
    let one_km = Length::from_meters(1_000.0);
    let bad = || Coordinate::wgs84(91.0, 0.0);
    assert_out_of_range(bad(), 91.0, 0.0);
    assert_out_of_range(bad().and_then(|b| destination(&b, 0.0, one_km)), 91.0, 0.0);
    assert_out_of_range(
        bad().and_then(|b| rhumb_destination(&b, 0.0, one_km)),
        91.0,
        0.0,
    );
    assert_out_of_range(bad().and_then(|b| midpoint(&b, &c(0.0, 0.0))), 91.0, 0.0);
    assert_out_of_range(
        bad().and_then(|b| intermediate(&c(0.0, 0.0), &b, 0.5)),
        91.0,
        0.0,
    );
    assert_out_of_range(
        bad().and_then(|b| intersection_spherical(&c(0.0, 0.0), 0.0, &b, 90.0)),
        91.0,
        0.0,
    );
}

/// The datum newtypes are valid by construction, so an out-of-range one can no
/// longer reach the `&impl LatLon` measurements: it is rejected where it would
/// be built, with the same error the measurements report.
#[test]
fn an_out_of_range_datum_newtype_cannot_reach_the_measurements() {
    use crate::{Bd09, Gcj02, Wgs84};
    for (lat, lon) in [(91.0, 0.0), (0.0, -180.5)] {
        assert_out_of_range(Wgs84::new(lat, lon), lat, lon);
        assert_out_of_range(Gcj02::new(lat, lon), lat, lon);
        assert_out_of_range(Bd09::new(lat, lon), lat, lon);
    }
    let ok = Wgs84::new(0.0, 0.0).unwrap();
    let end = Wgs84::new(0.0, 10.0).unwrap();
    assert!(geodesic_distance(&ok, &end).is_ok());
    assert!(cross_track_distance_spherical(&ok, &ok, &end).is_ok());
}

// ----- Ellipsoidal measurements across the antimeridian and antipode -----

#[test]
fn ellipsoidal_measurements_across_the_antimeridian_and_antipode() {
    // a·π/180 along the equator, taken the short way across ±180°.
    assert_close(
        geodesic_distance(&c(0.0, 179.5), &c(0.0, -179.5))
            .unwrap()
            .meters(),
        111_319.490_793,
        1e-3,
    );
    assert_close(
        initial_bearing(&c(0.0, 179.5), &c(0.0, -179.5)).unwrap(),
        90.0,
        1e-9,
    );
    // Equatorial antipodes: the geodesic runs over a pole, length 2·(meridian
    // quadrant) on WGS-84.
    assert_close(
        geodesic_distance(&c(0.0, 0.0), &c(0.0, 180.0))
            .unwrap()
            .meters(),
        20_003_931.458_625,
        1e-3,
    );
    // Over the pole: leave due north, arrive due south.
    assert_close(
        initial_bearing(&c(89.0, 0.0), &c(89.0, 180.0)).unwrap(),
        0.0,
        1e-9,
    );
    assert_close(
        final_bearing(&c(89.0, 0.0), &c(89.0, 180.0)).unwrap(),
        180.0,
        1e-9,
    );
}

// ----- TEST-002: producers near the poles and across the antimeridian -----

#[test]
fn destination_over_a_pole_and_across_the_antimeridian() {
    let l = Length::from_meters;
    // 50 km due north from 89.9°N passes over the pole onto the 180° meridian.
    let p = destination(&c(89.9, 0.0), 0.0, l(50_000.0)).unwrap();
    assert_close(p.lat(), 89.652_348_254, 1e-8);
    assert_eq!(p.lon().abs(), 180.0);
    let p = destination(&c(89.9, 30.0), 10.0, l(50_000.0)).unwrap();
    assert_close(p.lat(), 89.650_397_515, 1e-8);
    assert_close(p.lon(), -162.847_011_805, 1e-8);
    // One equatorial degree east from 179.5° wraps to −179.5°.
    let p = destination(&c(0.0, 179.5), 90.0, l(111_319.49)).unwrap();
    assert_close(p.lat(), 0.0, 1e-12);
    assert_close(p.lon(), -179.5, 1e-7);
    // …and west from −179.9° wraps to the positive side.
    let p = destination(&c(10.0, -179.9), 270.0, l(50_000.0)).unwrap();
    assert_close(p.lat(), 9.999_687_606, 1e-8);
    assert_close(p.lon(), 179.643_959_703, 1e-8);
}

#[test]
fn midpoint_and_intermediate_across_the_antimeridian_and_over_a_pole() {
    // The short way between 170°E and 170°W crosses 180°.
    let m = midpoint(&c(0.0, 170.0), &c(0.0, -170.0)).unwrap();
    assert_close(m.lat(), 0.0, 1e-12);
    assert_close(m.lon().abs(), 180.0, 1e-9);
    let m = midpoint(&c(10.0, 179.0), &c(-10.0, -179.0)).unwrap();
    assert_close(m.lat(), 0.0, 1e-9);
    assert_close(m.lon().abs(), 180.0, 1e-9);
    let q = intermediate(&c(0.0, 170.0), &c(0.0, -170.0), 0.75).unwrap();
    assert_close(q.lat(), 0.0, 1e-12);
    assert_close(q.lon(), -175.0, 1e-9);
    // Two points on opposite meridians at 80°N: the geodesic runs over the
    // north pole, so the midpoint is the pole and a quarter is still at lon 0.
    let m = midpoint(&c(80.0, 0.0), &c(80.0, 180.0)).unwrap();
    assert_close(m.lat(), 90.0, 1e-9);
    let q = intermediate(&c(80.0, 0.0), &c(80.0, 180.0), 0.25).unwrap();
    assert_close(q.lat(), 85.000_381_965, 1e-8);
    assert_close(q.lon(), 0.0, 1e-9);
    // …and symmetrically over the south pole.
    let m = midpoint(&c(-89.0, 45.0), &c(-89.0, -135.0)).unwrap();
    assert_close(m.lat(), -90.0, 1e-9);
    // The endpoints of `intermediate` are the inputs.
    let (a, b) = (c(10.0, 20.0), c(30.0, 40.0));
    let p0 = intermediate(&a, &b, 0.0).unwrap();
    let p1 = intermediate(&a, &b, 1.0).unwrap();
    assert_close(p0.lat(), 10.0, 1e-12);
    assert_close(p0.lon(), 20.0, 1e-12);
    assert_close(p1.lat(), 30.0, 1e-12);
    assert_close(p1.lon(), 40.0, 1e-12);
}
