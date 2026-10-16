//! Classic datum transforms (Helmert) and projected grids (UTM, UPS, MGRS).

use geocoordinates::convert::convert;
use geocoordinates::geodesy::{DatumTransform, Ecef, Helmert};
use geocoordinates::grids::{
    Mgrs, MgrsPrecision, Ups, Utm, UtmHemisphere, utm_central_meridian_deg, utm_zone_for,
};
use geocoordinates::{Coordinate, Crs, Height};

use crate::support::{DemoResult, assert_close, assert_within_meters};

pub(crate) fn classic_datums_and_projected_grids() -> DemoResult {
    let ecef = Ecef::new(4_000_000.0, -2_000_000.0, 4_500_000.0);
    assert_eq!(Helmert::IDENTITY.apply_ecef(ecef), ecef);
    assert_eq!(Helmert::IDENTITY.inverse_first_order(), Helmert::IDENTITY);

    // Each catalogued shift carries NIMA TR8350.2's published accuracy (the
    // root-sum-square of its per-axis figures); Pulkovo-1942 has none.
    for (datum, accuracy_m) in [
        (Crs::Nad27, Some(86f64.sqrt())),
        (Crs::Tokyo, Some(825f64.sqrt())),
        (Crs::Pulkovo42, None),
    ] {
        let transform =
            DatumTransform::catalog_to_wgs84(datum).expect("classic datum is catalogued");
        assert_eq!(transform.accuracy_m(), accuracy_m);
        let source = Coordinate::new(40.0, -100.0, datum)?.with_height(Height::Ellipsoidal(0.0))?;
        let wgs = transform.apply(source)?;
        let restored = transform.inverse_first_order().apply(wgs)?;
        assert_eq!(restored.crs(), datum);
        assert_within_meters(&restored, &source, 1e-6);
        // `convert` reports that accuracy as the Helmert leg's bound.
        let converted = convert(source, Crs::Wgs84)?;
        assert_eq!(converted.max_error_m(), accuracy_m.unwrap_or(f64::INFINITY));
    }
    assert!(DatumTransform::catalog_to_wgs84(Crs::Wgs84).is_none());

    assert_eq!(utm_zone_for(&Coordinate::wgs84(40.0, -75.0)?)?, 18);
    assert_eq!(utm_zone_for(&Coordinate::wgs84(60.0, 5.0)?)?, 32);
    assert_eq!(utm_zone_for(&Coordinate::wgs84(78.0, 20.0)?)?, 33);
    assert_eq!(utm_central_meridian_deg(18)?, -75.0);

    let philadelphia = Coordinate::wgs84(40.0, -75.0)?;
    let utm = Utm::try_from_coordinate(philadelphia)?;
    assert_eq!(utm.zone(), 18);
    assert_eq!(utm.hemisphere(), UtmHemisphere::North);
    assert_close(utm.easting(), 500_000.0, 0.001);
    assert_close(utm.northing(), 4_427_757.218_8, 0.001);
    assert_within_meters(&utm.to_coordinate(), &philadelphia, 0.001);
    assert_eq!(Utm::try_from(philadelphia)?, utm);
    assert_eq!(
        Utm::new(18, UtmHemisphere::North, utm.easting(), utm.northing())?,
        utm
    );
    assert_eq!(
        Utm::try_from_coordinate(Coordinate::wgs84(-40.0, -75.0)?)?.hemisphere(),
        UtmHemisphere::South
    );

    let north_polar = Coordinate::wgs84(85.0, 0.0)?;
    let ups = Ups::try_from_coordinate(north_polar)?;
    assert_eq!(ups.hemisphere(), UtmHemisphere::North);
    assert_within_meters(&ups.to_coordinate(), &north_polar, 0.001);
    assert_eq!(Ups::try_from(north_polar)?, ups);
    assert_eq!(
        Ups::new(UtmHemisphere::North, ups.easting(), ups.northing())?,
        ups
    );
    assert_eq!(
        Ups::try_from_coordinate(Coordinate::wgs84(-85.0, 0.0)?)?.hemisphere(),
        UtmHemisphere::South
    );

    let mgrs = Mgrs::try_from_coordinate(philadelphia, MgrsPrecision::M1)?;
    assert_eq!(mgrs.as_str(), "18TWK0000027757");
    assert_eq!(mgrs.precision(), MgrsPrecision::M1);
    assert_eq!(mgrs.precision().meters(), 1);
    let parsed: Mgrs = "18t wk 00000 27757".parse()?;
    assert_eq!(parsed, mgrs);
    // Display writes the canonical string, which parses back to an equal value.
    assert_eq!(parsed.to_string(), "18TWK0000027757");
    assert_eq!(parsed.to_string().parse::<Mgrs>()?, mgrs);
    assert_eq!(Mgrs::try_from("18TWK0000027757")?, mgrs);
    let decoded = mgrs.to_coordinate();
    // Half the 1 m square's diagonal, in ground meters (grid / UTM k0).
    assert_eq!(
        decoded.max_error_m(),
        core::f64::consts::FRAC_1_SQRT_2 / 0.9996
    );
    assert_within_meters(decoded.value(), &philadelphia, 1.0);

    Ok("classic datums and projected grids")
}
