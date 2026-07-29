//! Full-surface, self-verifying field lab for `geocoordinates`.
//!
//! Run with:
//!
//! ```text
//! cargo run --all-features --example flagship
//! ```
//!
//! Every section uses only public API. Assertions make this executable
//! documentation fail loudly if a reference value, round trip, error bound,
//! axis order, or typed failure regresses.

use std::error::Error as StdError;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use geocoordinates::angle::{
    Axis, Dd, Ddm, Dms, Hemisphere as AngleHemisphere, clamp_latitude, normalize_degrees,
    wrap_longitude,
};
use geocoordinates::china::out_of_china;
use geocoordinates::convert::{can_convert, convert};
use geocoordinates::dgg::{H3Cell, S2CellId};
use geocoordinates::fix::{AxisOrder, DatumAmbiguity};
use geocoordinates::format::{
    FormatOptions, HemisphereStyle, Representation, SymbolStyle, format, format_fix,
};
use geocoordinates::geodesy::{
    Aer, DatumTransform, Ecef, Ellipsoid, Enu, Helmert, Ned, along_track_distance,
    cross_track_distance, destination, final_bearing, geodesic_distance, haversine_distance,
    initial_bearing, intermediate, intersection, midpoint, rhumb_bearing, rhumb_destination,
    rhumb_distance,
};
use geocoordinates::grids::utm::{Hemisphere as GridHemisphere, central_meridian_deg, zone_for};
use geocoordinates::grids::{Geohash, Maidenhead, Mgrs, PlusCode, Ups, Utm};
use geocoordinates::parse::interchange::{from_geojson, from_gpx, from_kml, from_wkt};
use geocoordinates::parse::sensors::from_nmea_sentence;
use geocoordinates::parse::text::{TextParseOptions, parse as parse_text, parse_with};
use geocoordinates::parse::{from_geo_uri, parse_coordinate};
use geocoordinates::{
    Accuracy, Approx, BaiduMercator, Bd09, Confidence, Coordinate, Crs, Error, Fix, Gcj02, Height,
    LatLon, Length, LengthUnit, RawSource, Wgs84,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

type DemoResult = Result<&'static str, Box<dyn StdError>>;

const NEW_YORK: Coordinate = Coordinate {
    lat: 40.7128,
    lon: -74.006,
    height: None,
    crs: Crs::Wgs84,
};

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} ± {tolerance}, got {actual}"
    );
}

fn assert_within_meters(actual: &impl LatLon, expected: &impl LatLon, max_m: f64) {
    let distance = geodesic_distance(actual, expected)
        .expect("flagship compares valid positions in one CRS")
        .meters();
    assert!(
        distance <= max_m + 1e-6,
        "points are {distance} m apart, beyond {max_m} m"
    );
}

fn assert_mercator_close(actual: BaiduMercator, expected: BaiduMercator, tolerance_m: f64) {
    assert_close(actual.x, expected.x, tolerance_m);
    assert_close(actual.y, expected.y, tolerance_m);
}

fn assert_enu_close(actual: Enu, expected: Enu) {
    assert_close(actual.east, expected.east, 1e-9);
    assert_close(actual.north, expected.north, 1e-9);
    assert_close(actual.up, expected.up, 1e-9);
}

fn assert_ned_close(actual: Ned, expected: Ned) {
    assert_close(actual.north, expected.north, 1e-9);
    assert_close(actual.east, expected.east, 1e-9);
    assert_close(actual.down, expected.down, 1e-9);
}

fn assert_aer_close(actual: Aer, expected: Aer) {
    assert_close(actual.azimuth_deg, expected.azimuth_deg, 1e-9);
    assert_close(actual.elevation_deg, expected.elevation_deg, 1e-9);
    assert_close(actual.range.meters(), expected.range.meters(), 1e-9);
}

fn assert_serde<T>()
where
    T: Serialize + DeserializeOwned,
{
}

fn all_public_value_types_are_serde() {
    assert_serde::<AngleHemisphere>();
    assert_serde::<Axis>();
    assert_serde::<Dd>();
    assert_serde::<Dms>();
    assert_serde::<Ddm>();
    assert_serde::<Approx<Coordinate>>();
    assert_serde::<BaiduMercator>();
    assert_serde::<Bd09>();
    assert_serde::<Gcj02>();
    assert_serde::<Wgs84>();
    assert_serde::<Coordinate>();
    assert_serde::<Crs>();
    assert_serde::<Height>();
    assert_serde::<Accuracy>();
    assert_serde::<Confidence>();
    assert_serde::<Fix>();
    assert_serde::<RawSource>();
    assert_serde::<AxisOrder>();
    assert_serde::<DatumAmbiguity>();
    assert_serde::<SystemTime>();
    assert_serde::<Length>();
    assert_serde::<LengthUnit>();
    assert_serde::<Representation>();
    assert_serde::<SymbolStyle>();
    assert_serde::<HemisphereStyle>();
    assert_serde::<FormatOptions>();
    assert_serde::<TextParseOptions>();
    assert_serde::<Ellipsoid>();
    assert_serde::<Ecef>();
    assert_serde::<Enu>();
    assert_serde::<Ned>();
    assert_serde::<Aer>();
    assert_serde::<Helmert>();
    assert_serde::<DatumTransform>();
    assert_serde::<PlusCode>();
    assert_serde::<Geohash>();
    assert_serde::<Maidenhead>();
    assert_serde::<Mgrs>();
    assert_serde::<Utm>();
    assert_serde::<Ups>();
    assert_serde::<GridHemisphere>();
    assert_serde::<H3Cell>();
    assert_serde::<S2CellId>();
}

fn primitives_and_serde() -> DemoResult {
    let coordinate = Coordinate::wgs84(40.7128, -74.006).with_height(Height::Ellipsoidal(12.5));
    coordinate.validate()?;
    assert!(!coordinate.is_null_island());
    assert!(Coordinate::wgs84(0.0, 0.0).is_null_island());
    assert_eq!(coordinate.crs.to_string(), "WGS84");
    assert_close(coordinate.lat(), 40.7128, 1e-12);
    assert_close(coordinate.lon(), -74.006, 1e-12);

    let dd = Dd(-73.9857);
    let dms = dd.try_to_dms(Axis::Longitude)?;
    assert_eq!(dms.hemisphere, AngleHemisphere::West);
    assert_close(dms.try_to_dd()?.0, dd.0, 1e-12);
    let ddm = dms.try_to_ddm()?;
    assert_eq!(dd.try_to_ddm(Axis::Longitude)?, ddm);
    assert_close(ddm.try_to_dd()?.0, dd.0, 1e-12);
    assert_eq!(ddm.try_to_dms()?.hemisphere, AngleHemisphere::West);
    assert_eq!(
        Dd(40.0).try_to_dms(Axis::Latitude)?.hemisphere,
        AngleHemisphere::North
    );
    assert_eq!(AngleHemisphere::North.sign(), 1.0);
    assert_eq!(AngleHemisphere::East.sign(), 1.0);
    assert_eq!(AngleHemisphere::South.sign(), -1.0);
    assert_eq!(AngleHemisphere::West.sign(), -1.0);

    assert_eq!(wrap_longitude(181.0), -179.0);
    assert_eq!(clamp_latitude(95.0), 90.0);
    assert_eq!(normalize_degrees(-10.0), 350.0);

    for unit in [
        LengthUnit::Meter,
        LengthUnit::Kilometer,
        LengthUnit::Foot,
        LengthUnit::UsSurveyFoot,
        LengthUnit::NauticalMile,
    ] {
        let one = Length::from_unit(1.0, unit);
        assert_close(one.to_unit(unit), 1.0, 1e-12);
    }
    assert_eq!(Length::ZERO.meters(), 0.0);
    assert_eq!(
        Length::from_unit(1.0, LengthUnit::NauticalMile).meters(),
        1_852.0
    );
    let arithmetic =
        (Length::from_meters(10.0) + Length::from_meters(5.0) - Length::from_meters(3.0)) * 2.0;
    assert_eq!(arithmetic.meters(), 24.0);

    assert_eq!(Confidence::new(-1.0).value(), 0.0);
    assert_eq!(Confidence::new(2.0).value(), 1.0);
    let fix = Fix {
        coord: coordinate,
        accuracy: Some(Accuracy {
            horizontal_m: Some(3.0),
            vertical_m: Some(5.0),
        }),
        timestamp: Some(UNIX_EPOCH + Duration::from_secs(1_700_000_000)),
        source: Some(RawSource {
            raw: "40.7128, -74.006".to_owned(),
            confidence: Confidence::new(0.9),
            axis_order: Some(AxisOrder::LatLon),
            datum_ambiguity: Some(DatumAmbiguity::PossiblyGcj02),
            notes: vec!["flagship fixture".to_owned()],
        }),
    };
    assert_eq!(Fix::from_coord(coordinate).coord, coordinate);

    all_public_value_types_are_serde();
    let json = serde_json::to_string(&fix)?;
    let decoded: Fix = serde_json::from_str(&json)?;
    assert_eq!(decoded, fix);
    let axis_json = serde_json::to_string(&Axis::Longitude)?;
    assert_eq!(serde_json::from_str::<Axis>(&axis_json)?, Axis::Longitude);

    Ok("primitives and serde")
}

fn formatting_and_parsing() -> DemoResult {
    assert_eq!(
        format(&NEW_YORK, &FormatOptions::default())?,
        "40.712800, -74.006000"
    );
    assert_eq!(NEW_YORK.to_string(), "40.712800, -74.006000");
    let from_str: Coordinate = "40.7128, -74.006".parse()?;
    assert_within_meters(&from_str, &NEW_YORK, 0.01);

    for representation in [
        Representation::DecimalDegrees,
        Representation::Dms,
        Representation::Ddm,
        Representation::PlusCode,
    ] {
        let rendered = format(
            &NEW_YORK,
            &FormatOptions {
                representation,
                ..FormatOptions::default()
            },
        )?;
        assert!(!rendered.is_empty());
    }

    let symbols = [
        (SymbolStyle::Unicode, "10°30′00″N 20°15′00″E"),
        (SymbolStyle::Ascii, "10°30'00\"N 20°15'00\"E"),
        (SymbolStyle::Letters, "10d30m00sN 20d15m00sE"),
    ];
    for (symbol_style, expected) in symbols {
        let rendered = format(
            &Coordinate::wgs84(10.5, 20.25),
            &FormatOptions {
                representation: Representation::Dms,
                precision: Some(0),
                symbol_style,
                hemisphere_style: HemisphereStyle::Cardinal,
                locale: None,
            },
        )?;
        assert_eq!(rendered, expected);
    }
    assert_eq!(
        format(
            &NEW_YORK,
            &FormatOptions {
                hemisphere_style: HemisphereStyle::Cardinal,
                ..FormatOptions::default()
            },
        )?,
        "40.712800 N, 74.006000 W"
    );
    assert_eq!(
        format(
            &NEW_YORK,
            &FormatOptions {
                locale: Some("de-DE".to_owned()),
                ..FormatOptions::default()
            },
        )?,
        "40,712800 -74,006000"
    );
    let low_accuracy = Fix {
        coord: NEW_YORK,
        accuracy: Some(Accuracy {
            horizontal_m: Some(1_000.0),
            vertical_m: None,
        }),
        timestamp: None,
        source: None,
    };
    assert_eq!(
        format_fix(
            &low_accuracy,
            &FormatOptions {
                precision: None,
                ..FormatOptions::default()
            }
        )?,
        "40.71, -74.01"
    );

    let dms = parse_text("40°42′46″N 74°00′22″W")?;
    assert_close(dms.coord.lat, 40.0 + 42.0 / 60.0 + 46.0 / 3_600.0, 1e-9);
    assert_close(dms.coord.lon, -(74.0 + 22.0 / 3_600.0), 1e-9);
    assert_eq!(dms.source.as_ref().and_then(|s| s.axis_order), None);
    let ddm = parse_text("40 42.766 N 74 0.456 W")?;
    assert_close(ddm.coord.lat, 40.0 + 42.766 / 60.0, 1e-9);
    assert_close(ddm.coord.lon, -(74.0 + 0.456 / 60.0), 1e-9);

    let lon_lat = parse_with(
        "-74.006, 40.7128",
        &TextParseOptions {
            default_axis_order: AxisOrder::LonLat,
            decimal_comma: false,
        },
    )?;
    assert_eq!(
        lon_lat.source.as_ref().and_then(|s| s.axis_order),
        Some(AxisOrder::LonLat)
    );
    assert_within_meters(&lon_lat.coord, &NEW_YORK, 0.01);
    let decimal_comma = parse_with(
        "40,7128 -74,006",
        &TextParseOptions {
            default_axis_order: AxisOrder::LatLon,
            decimal_comma: true,
        },
    )?;
    assert_within_meters(&decimal_comma.coord, &NEW_YORK, 0.01);

    let uri = from_geo_uri("geo:48.2,16.3,183;crs=wgs84;u=40")?;
    assert_eq!(uri.coord.height, Some(Height::Ellipsoidal(183.0)));
    assert_eq!(
        uri.accuracy.and_then(|accuracy| accuracy.horizontal_m),
        Some(40.0)
    );
    let plus = parse_coordinate("8FVC2222+22")?;
    assert_eq!(plus.source.as_ref().and_then(|s| s.axis_order), None);
    assert!(
        (5.0..15.0).contains(
            &plus
                .accuracy
                .and_then(|accuracy| accuracy.horizontal_m)
                .expect("Plus Code reports its cell bound")
        )
    );

    let rendered = format(&NEW_YORK, &FormatOptions::default())?;
    let reparsed = parse_coordinate(&rendered)?;
    assert_within_meters(&reparsed.coord, &NEW_YORK, 0.2);

    Ok("formatting and parsing")
}

fn interchange_and_nmea() -> DemoResult {
    let geojson = from_geojson(r#"{"type":"Point","coordinates":[2.0,9.0,100.0]}"#)?;
    assert_eq!(geojson.len(), 1);
    assert_eq!(geojson[0].coord.height, Some(Height::Ellipsoidal(100.0)));
    assert_close(geojson[0].coord.lat, 9.0, 1e-12);
    assert_close(geojson[0].coord.lon, 2.0, 1e-12);
    assert_eq!(
        geojson[0].source.as_ref().and_then(|s| s.axis_order),
        Some(AxisOrder::LonLat)
    );

    let wkt = from_wkt("LINESTRING(0 0, 10 20)")?;
    assert_eq!(wkt.len(), 2);
    assert_close(wkt[1].coord.lat, 20.0, 1e-12);
    assert_close(wkt[1].coord.lon, 10.0, 1e-12);

    let gpx = from_gpx(
        r#"<?xml version="1.0"?>
        <gpx version="1.1" creator="flagship" xmlns="http://www.topografix.com/GPX/1/1">
          <wpt lat="48.2" lon="16.3"><ele>183.0</ele></wpt>
        </gpx>"#,
    )?;
    assert_eq!(gpx.len(), 1);
    assert_eq!(gpx[0].coord.height, Some(Height::Ellipsoidal(183.0)));
    assert_eq!(
        gpx[0].source.as_ref().and_then(|s| s.axis_order),
        Some(AxisOrder::LatLon)
    );

    let kml = from_kml(
        r#"<?xml version="1.0"?>
        <kml xmlns="http://www.opengis.net/kml/2.2"><Document>
          <Placemark><Point><coordinates>16.3,48.2,183</coordinates></Point></Placemark>
        </Document></kml>"#,
    )?;
    assert_eq!(kml.len(), 1);
    assert_close(kml[0].coord.lat, 48.2, 1e-9);
    assert_close(kml[0].coord.lon, 16.3, 1e-9);
    assert_eq!(
        kml[0].source.as_ref().and_then(|s| s.axis_order),
        Some(AxisOrder::LonLat)
    );

    let nmea =
        from_nmea_sentence("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47")?;
    assert_close(nmea.coord.lat, 48.0 + 7.038 / 60.0, 1e-9);
    assert_close(nmea.coord.lon, 11.0 + 31.0 / 60.0, 1e-9);
    assert_eq!(nmea.coord.height, Some(Height::Orthometric(545.4)));
    assert_close(
        nmea.accuracy
            .and_then(|accuracy| accuracy.horizontal_m)
            .expect("GGA HDOP provides horizontal accuracy"),
        4.5,
        1e-9,
    );

    Ok("interchange and NMEA")
}

fn china_datums_and_dispatch() -> DemoResult {
    const WGS: Wgs84 = Wgs84 {
        lat: 39.915,
        lon: 116.404,
    };
    const EXPECTED_GCJ: Gcj02 = Gcj02 {
        lat: 39.916_404_281_501_64,
        lon: 116.410_244_499_169_38,
    };
    const EXPECTED_BD: Bd09 = Bd09 {
        lat: 39.922_699_552_216_216,
        lon: 116.416_627_243_787_33,
    };

    let gcj = WGS.try_to_gcj02()?;
    assert_eq!(Wgs84::new(WGS.lat, WGS.lon), WGS);
    assert_eq!(Coordinate::gcj02(gcj.lat, gcj.lon).crs, Crs::Gcj02);
    assert_close(gcj.lat, EXPECTED_GCJ.lat, 1e-9);
    assert_close(gcj.lon, EXPECTED_GCJ.lon, 1e-9);
    let gcj_from = Gcj02::try_from(WGS)?;
    assert_eq!(gcj_from, gcj);
    let bd = WGS.try_to_bd09()?;
    assert_eq!(Bd09::new(bd.lat, bd.lon), bd);
    assert_eq!(Coordinate::bd09(bd.lat, bd.lon).crs, Crs::Bd09);
    assert_close(bd.lat, EXPECTED_BD.lat, 1e-9);
    assert_close(bd.lon, EXPECTED_BD.lon, 1e-9);
    let bd_from_gcj = Bd09::try_from(gcj)?;
    assert_eq!(bd_from_gcj, gcj.try_to_bd09()?);

    let fast_wgs = gcj.try_to_wgs84_fast()?;
    let refined_wgs = gcj.try_to_wgs84_refined()?;
    assert!(fast_wgs.max_error_m() > refined_wgs.max_error_m());
    assert_within_meters(&*fast_wgs, &WGS, fast_wgs.max_error_m());
    assert_within_meters(&*refined_wgs, &WGS, refined_wgs.max_error_m());
    let fast_gcj = bd.try_to_gcj02_fast()?;
    let refined_gcj = bd.try_to_gcj02_refined()?;
    assert_within_meters(&*fast_gcj, &gcj, fast_gcj.max_error_m());
    assert_within_meters(&*refined_gcj, &gcj, refined_gcj.max_error_m());
    let refined_from_bd = bd.try_to_wgs84_refined()?;
    assert_within_meters(&*refined_from_bd, &WGS, refined_from_bd.max_error_m());

    let mercator = BaiduMercator::try_from_bd09(bd)?;
    assert_eq!(BaiduMercator::new(mercator.x, mercator.y), mercator);
    let mercator_from = BaiduMercator::try_from(bd)?;
    assert_eq!(mercator_from, mercator);
    assert_within_meters(&mercator.try_to_bd09()?, &bd, 1.0);
    let tagged = mercator.try_to_coordinate()?;
    assert_eq!(tagged.crs, Crs::Bd09);
    assert_mercator_close(BaiduMercator::try_from_coordinate(tagged)?, mercator, 0.02);
    assert_mercator_close(BaiduMercator::try_from(tagged)?, mercator, 0.02);

    let london = Wgs84::new(51.5074, -0.1278);
    assert!(out_of_china(london.lat, london.lon));
    assert_eq!(london.try_to_gcj02()?, Gcj02::new(london.lat, london.lon));

    let systems = [
        Crs::Wgs84,
        Crs::Gcj02,
        Crs::Bd09,
        Crs::Nad27,
        Crs::Tokyo,
        Crs::Pulkovo42,
    ];
    for from in systems {
        let source = Coordinate::new(39.915, 116.404, from).with_height(Height::Ellipsoidal(12.0));
        for to in systems {
            assert!(can_convert(from, to));
            let converted = convert(source, to)?;
            assert_eq!(converted.crs, to);
            assert!(converted.lat.is_finite() && converted.lon.is_finite());
            assert!(converted.height.is_some());
            let is_approximate = (from == Crs::Gcj02 && to != Crs::Gcj02 && to != Crs::Bd09)
                || (from == Crs::Bd09 && to != Crs::Bd09);
            if is_approximate {
                assert!(converted.max_error_m() > 0.0);
            } else {
                assert_eq!(converted.max_error_m(), 0.0);
            }
        }
    }

    Ok("China datums and runtime dispatch")
}

fn geodesics_and_local_frames() -> DemoResult {
    let equator_start = Coordinate::wgs84(0.0, 0.0);
    let equator_end = Coordinate::wgs84(0.0, 1.0);
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

    let route_start = Coordinate::wgs84(40.0, -75.0);
    let route_end = Coordinate::wgs84(41.0, -73.0);
    let direct = destination(
        &route_start,
        initial_bearing(&route_start, &route_end)?,
        geodesic_distance(&route_start, &route_end)?,
    )?;
    assert_within_meters(&direct, &route_end, 0.2);
    assert_close(
        midpoint(&Coordinate::wgs84(0.0, 0.0), &Coordinate::wgs84(0.0, 2.0))?.lon,
        1.0,
        1e-6,
    );
    assert_close(
        intermediate(
            &Coordinate::wgs84(0.0, 0.0),
            &Coordinate::wgs84(0.0, 10.0),
            0.25,
        )?
        .lon,
        2.5,
        1e-6,
    );
    let rhumb_end = Coordinate::wgs84(45.0, -70.0);
    let rhumb = rhumb_destination(
        &route_start,
        rhumb_bearing(&route_start, &rhumb_end)?,
        rhumb_distance(&route_start, &rhumb_end)?,
    )?;
    assert_within_meters(&rhumb, &rhumb_end, 0.2);

    let path_start = Coordinate::wgs84(0.0, 0.0);
    let path_end = Coordinate::wgs84(0.0, 10.0);
    let off_path = Coordinate::wgs84(1.0, 5.0);
    assert!(cross_track_distance(&off_path, &path_start, &path_end)?.meters() < 0.0);
    assert_close(
        along_track_distance(&off_path, &path_start, &path_end)?.meters(),
        5.0 * 111_195.0,
        2_000.0,
    );
    let crossing = intersection(
        &Coordinate::wgs84(0.0, -10.0),
        90.0,
        &Coordinate::wgs84(-10.0, 0.0),
        0.0,
    )?
    .expect("equator and prime meridian intersect");
    assert_within_meters(&crossing, &Coordinate::wgs84(0.0, 0.0), 0.02);

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

    let origin = Coordinate::wgs84(40.0, -75.0).with_height(Height::Ellipsoidal(100.0));
    let target = Coordinate::wgs84(40.001, -74.999).with_height(Height::Ellipsoidal(125.0));
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

fn classic_datums_and_projected_grids() -> DemoResult {
    let ecef = Ecef::new(4_000_000.0, -2_000_000.0, 4_500_000.0);
    assert_eq!(Helmert::IDENTITY.apply_ecef(ecef), ecef);
    assert_eq!(Helmert::IDENTITY.inverse(), Helmert::IDENTITY);

    for datum in [Crs::Nad27, Crs::Tokyo, Crs::Pulkovo42] {
        let transform = DatumTransform::to_wgs84(datum).expect("classic datum is catalogued");
        let source = Coordinate::new(40.0, -100.0, datum).with_height(Height::Ellipsoidal(0.0));
        let wgs = transform.transform(source)?;
        let restored = transform.inverse().transform(wgs)?;
        assert_eq!(restored.crs, datum);
        assert_within_meters(&restored, &source, 0.001);
    }
    assert!(DatumTransform::to_wgs84(Crs::Wgs84).is_none());

    assert_eq!(zone_for(40.0, -75.0), 18);
    assert_eq!(zone_for(60.0, 5.0), 32);
    assert_eq!(zone_for(78.0, 20.0), 33);
    assert_eq!(central_meridian_deg(18), -75.0);

    let philadelphia = Coordinate::wgs84(40.0, -75.0);
    let utm = Utm::try_from_coordinate(philadelphia)?;
    assert_eq!(utm.zone, 18);
    assert_eq!(utm.hemisphere, GridHemisphere::North);
    assert_close(utm.easting, 500_000.0, 0.001);
    assert_close(utm.northing, 4_427_757.218_8, 0.001);
    assert_within_meters(&utm.try_to_coordinate()?, &philadelphia, 0.001);
    assert_eq!(Utm::try_from(philadelphia)?, utm);
    assert_eq!(
        Utm::try_from_coordinate(Coordinate::wgs84(-40.0, -75.0))?.hemisphere,
        GridHemisphere::South
    );

    let north_polar = Coordinate::wgs84(85.0, 0.0);
    let ups = Ups::try_from_coordinate(north_polar)?;
    assert_eq!(ups.hemisphere, GridHemisphere::North);
    assert_within_meters(&ups.try_to_coordinate()?, &north_polar, 0.001);
    assert_eq!(Ups::try_from(north_polar)?, ups);
    assert_eq!(
        Ups::try_from_coordinate(Coordinate::wgs84(-85.0, 0.0))?.hemisphere,
        GridHemisphere::South
    );

    let mgrs = Mgrs::try_from_coordinate(philadelphia, 1)?;
    assert_eq!(mgrs.as_str(), "18TWK0000027757");
    assert_eq!(mgrs.precision_m(), 1);
    let parsed: Mgrs = "18t wk 00000 27757".parse()?;
    assert_eq!(parsed, mgrs);
    assert_eq!(Mgrs::try_from("18TWK0000027757")?, mgrs);
    let decoded = mgrs.to_coordinate();
    assert_eq!(decoded.max_error_m(), core::f64::consts::FRAC_1_SQRT_2);
    assert_within_meters(decoded.value(), &philadelphia, 1.0);

    Ok("classic datums and projected grids")
}

fn encoded_and_global_grids() -> DemoResult {
    let plus = PlusCode::encode(Coordinate::wgs84(47.000_062_5, 8.000_062_5), 10)?;
    assert_eq!(plus.as_str(), "8FVC2222+22");
    assert_eq!(PlusCode::try_from("8FVC2222+22")?, plus);
    assert_eq!("8FVC2222+22".parse::<PlusCode>()?, plus);
    let plus_area = plus.decode();
    assert_within_meters(
        plus_area.value(),
        &Coordinate::wgs84(47.000_062_5, 8.000_062_5),
        plus_area.max_error_m(),
    );
    assert!(plus_area.lat.is_finite());
    let bound = plus_area.max_error_m();
    let mapped = plus_area.map(|coordinate| (coordinate.lat, coordinate.lon));
    assert_eq!(mapped.max_error_m(), bound);
    let inner = plus_area.into_inner();
    assert!(inner.lat.is_finite() && inner.lon.is_finite());

    let geohash = Geohash::encode(Coordinate::wgs84(42.6, -5.6), 5)?;
    assert_eq!(geohash.as_str(), "ezs42");
    assert_eq!(Geohash::try_from("EZS42")?, geohash);
    assert_eq!("ezs42".parse::<Geohash>()?, geohash);
    let geohash_area = geohash.decode();
    assert_within_meters(
        geohash_area.value(),
        &Coordinate::wgs84(42.6, -5.6),
        geohash_area.max_error_m(),
    );

    let maidenhead = Maidenhead::encode(Coordinate::wgs84(40.5, -75.0), 2)?;
    assert_eq!(maidenhead.as_str(), "FN20");
    assert_eq!(Maidenhead::try_from("fn20")?, maidenhead);
    assert_eq!("FN20".parse::<Maidenhead>()?, maidenhead);
    let maidenhead_area = maidenhead.decode();
    assert_within_meters(
        maidenhead_area.value(),
        &Coordinate::wgs84(40.5, -75.0),
        maidenhead_area.max_error_m(),
    );

    let h3 = H3Cell::encode(NEW_YORK, 9)?;
    assert_eq!(h3.0, 617_733_151_020_810_239);
    let h3_area = h3.decode()?;
    assert!((205.0..206.0).contains(&h3_area.max_error_m()));
    assert_within_meters(h3_area.value(), &NEW_YORK, h3_area.max_error_m());
    assert!(
        H3Cell::encode(NEW_YORK, 12)?.decode()?.max_error_m()
            < H3Cell::encode(NEW_YORK, 5)?.decode()?.max_error_m()
    );

    let s2 = S2CellId::encode(NEW_YORK, 20)?;
    assert_eq!(s2.0, 9_926_595_630_970_437_632);
    let s2_area = s2.decode()?;
    assert!((6.0..7.0).contains(&s2_area.max_error_m()));
    assert_within_meters(s2_area.value(), &NEW_YORK, s2_area.max_error_m());
    assert!(
        S2CellId::encode(NEW_YORK, 25)?.decode()?.max_error_m()
            < S2CellId::encode(NEW_YORK, 5)?.decode()?.max_error_m()
    );

    Ok("encoded and global grids")
}

fn boundaries_and_typed_errors() -> DemoResult {
    Coordinate::wgs84(90.0, 180.0).validate()?;
    Coordinate::wgs84(-90.0, -180.0).validate()?;
    assert_eq!(wrap_longitude(180.0), -180.0);
    assert_eq!(wrap_longitude(-181.0), 179.0);

    let antimeridian_distance = geodesic_distance(
        &Coordinate::wgs84(0.0, 179.9),
        &Coordinate::wgs84(0.0, -179.9),
    )?;
    assert!(antimeridian_distance.meters() < 25_000.0);
    let reflected = rhumb_destination(
        &Coordinate::wgs84(80.0, 0.0),
        0.0,
        Length::from_meters(1_668_000.0),
    )?;
    assert!((80.0..90.0).contains(&reflected.lat));

    for polar in [
        Coordinate::wgs84(90.0, 180.0),
        Coordinate::wgs84(-90.0, -180.0),
    ] {
        let plus = PlusCode::encode(polar, 10)?.decode();
        assert!(plus.lat.is_finite() && plus.lon.is_finite());
        let h3 = H3Cell::encode(polar, 9)?.decode()?;
        assert!(h3.lat.is_finite() && h3.lon.is_finite());
        let s2 = S2CellId::encode(polar, 20)?.decode()?;
        assert!(s2.lat.is_finite() && s2.lon.is_finite());
    }

    assert!(matches!(
        Coordinate::wgs84(91.0, 0.0).validate(),
        Err(Error::OutOfRange { .. })
    ));
    assert!(matches!(
        BaiduMercator::try_from_coordinate(Coordinate::wgs84(39.915, 116.404)),
        Err(Error::CrsMismatch { .. })
    ));
    assert!(matches!(
        Wgs84::try_from(Coordinate::gcj02(39.915, 116.404)),
        Err(Error::CrsMismatch { .. })
    ));
    assert!(matches!(
        PlusCode::try_from("not-a-plus-code"),
        Err(Error::InvalidGridRef(_))
    ));
    assert!(matches!(
        parse_coordinate("not a coordinate"),
        Err(Error::Parse(_))
    ));
    assert!(matches!(
        Utm::try_from_coordinate(Coordinate::wgs84(85.0, 0.0)),
        Err(Error::OutOfRange { .. })
    ));
    assert!(matches!(
        Ups::try_from_coordinate(Coordinate::wgs84(40.0, -75.0)),
        Err(Error::OutOfRange { .. })
    ));
    assert!(matches!(
        from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1E"),
        Err(Error::Parse(_))
    ));

    Ok("boundaries and typed errors")
}

#[cfg(not(test))]
fn main() -> Result<(), Box<dyn StdError>> {
    println!(
        "geocoordinates {} — full-surface field lab",
        env!("CARGO_PKG_VERSION")
    );
    for section in [
        primitives_and_serde()?,
        formatting_and_parsing()?,
        interchange_and_nmea()?,
        china_datums_and_dispatch()?,
        geodesics_and_local_frames()?,
        classic_datums_and_projected_grids()?,
        encoded_and_global_grids()?,
        boundaries_and_typed_errors()?,
    ] {
        println!("[ok] {section}");
    }
    println!("All 8 shipped Rust capability groups passed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn StdError>>;

    #[test]
    fn primitives_and_serde_work() -> TestResult {
        primitives_and_serde().map(|_| ())
    }

    #[test]
    fn formatting_and_parsing_work() -> TestResult {
        formatting_and_parsing().map(|_| ())
    }

    #[test]
    fn interchange_and_nmea_work() -> TestResult {
        interchange_and_nmea().map(|_| ())
    }

    #[test]
    fn china_datums_and_dispatch_work() -> TestResult {
        china_datums_and_dispatch().map(|_| ())
    }

    #[test]
    fn geodesics_and_local_frames_work() -> TestResult {
        geodesics_and_local_frames().map(|_| ())
    }

    #[test]
    fn classic_datums_and_projected_grids_work() -> TestResult {
        classic_datums_and_projected_grids().map(|_| ())
    }

    #[test]
    fn encoded_and_global_grids_work() -> TestResult {
        encoded_and_global_grids().map(|_| ())
    }

    #[test]
    fn boundaries_and_typed_errors_work() -> TestResult {
        boundaries_and_typed_errors().map(|_| ())
    }
}
