//! Calls every grid, projection, interchange, sensor, and discrete-global-grid
//! FFI export at least once with an asserted value.
//!
//! Encodings are pinned to their canonical published forms (Plus Code,
//! geohash, Maidenhead, MGRS, H3, S2); decoded centers and bounds are pinned to
//! what the core crate produces today.

use geocoordinates_ffi::*;

fn close(actual: f64, expected: f64, tol: f64) {
    assert!(
        (actual - expected).abs() <= tol,
        "expected {expected} ± {tol}, got {actual}"
    );
}

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Wgs84).unwrap()
}

fn gcj02(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Gcj02).unwrap()
}

fn bd09(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Bd09).unwrap()
}

// --- Plus Code, geohash, Maidenhead ---

#[test]
fn plus_code_encodes_and_decodes_its_cell() {
    let coord = wgs84(47.000_062_5, 8.000_062_5);
    assert_eq!(plus_code_encode(coord, 10).unwrap(), "8FVC2222+22");
    let area = plus_code_decode("8FVC2222+22".to_string()).unwrap();
    assert_eq!(area.coord, coord);
    close(area.max_error_m, 8.418_605_830_499_43, 1e-9);
    assert!(matches!(
        plus_code_decode("not a code".to_string()),
        Err(GeoError::InvalidGridRef { .. })
    ));
}

#[test]
fn geohash_encodes_and_decodes_its_cell() {
    assert_eq!(geohash_encode(wgs84(42.6, -5.6), 5).unwrap(), "ezs42");
    let cell = geohash_decode("ezs42".to_string()).unwrap();
    assert_eq!(cell.coord, wgs84(42.604_980_468_75, -5.603_027_343_75));
    close(cell.max_error_m, 3_034.794_837_835_592_3, 1e-6);
    assert!(matches!(
        geohash_decode("ail".to_string()),
        Err(GeoError::InvalidGridRef { .. })
    ));
    // Twelve characters is the longest geohash, for encode and decode alike.
    assert!(matches!(
        geohash_encode(wgs84(42.6, -5.6), 13),
        Err(GeoError::InvalidValue { .. })
    ));
    assert!(matches!(
        geohash_decode("ezs42ezs42ezs".to_string()),
        Err(GeoError::InvalidGridRef { .. })
    ));
}

#[test]
fn canonicalize_returns_the_canonical_code_or_rejects() {
    assert_eq!(
        plus_code_canonicalize(" 87g7x2vv+2v ".to_string()).unwrap(),
        "87G7X2VV+2V"
    );
    assert_eq!(geohash_canonicalize("EZS42".to_string()).unwrap(), "ezs42");
    assert_eq!(
        maidenhead_canonicalize("jn58TD".to_string()).unwrap(),
        "JN58td"
    );
    assert_eq!(
        mgrs_canonicalize("4q fj 1234 5678".to_string()).unwrap(),
        "04QFJ12345678"
    );
    for rejected in [
        plus_code_canonicalize("X2VV+2V".to_string()),
        geohash_canonicalize("ezs42ezs42ezs".to_string()),
        maidenhead_canonicalize("SN20".to_string()),
        mgrs_canonicalize("4QFJ123".to_string()),
    ] {
        assert!(
            matches!(rejected, Err(GeoError::InvalidGridRef { .. })),
            "{rejected:?}"
        );
    }
}

#[test]
fn maidenhead_encodes_and_decodes_its_square() {
    assert_eq!(maidenhead_encode(wgs84(40.5, -75.0), 2).unwrap(), "FN20");
    let square = maidenhead_decode("FN20".to_string()).unwrap();
    assert_eq!(square.coord, wgs84(40.5, -75.0));
    close(square.max_error_m, 101_592.817_120_622_36, 1e-6);
}

// --- UTM / UPS / MGRS ---

#[test]
fn utm_matches_published_values_and_round_trips() {
    let utm = utm_from_coordinate(wgs84(40.0, -75.0)).unwrap();
    assert_eq!((utm.zone, utm.hemisphere), (18, UtmHemisphere::North));
    close(utm.easting, 500_000.0, 1e-3);
    close(utm.northing, 4_427_757.218_8, 1e-3);
    let back = utm_to_coordinate(utm).unwrap();
    close(back.lat, 40.0, 1e-9);
    close(back.lon, -75.0, 1e-9);
    assert_eq!(back.crs, Crs::Wgs84);

    // Sydney, southern hemisphere (false northing applied).
    let sydney = utm_from_coordinate(wgs84(-33.8688, 151.2093)).unwrap();
    assert_eq!((sydney.zone, sydney.hemisphere), (56, UtmHemisphere::South));
    close(sydney.easting, 334_368.633_648_095_25, 1e-6);
    close(sydney.northing, 6_250_948.345_385_062, 1e-6);
}

#[test]
fn utm_rejects_polar_latitudes() {
    assert!(matches!(
        utm_from_coordinate(wgs84(85.0, 0.0)),
        Err(GeoError::OutsideDomain { operation, lat: 85.0, lon: 0.0 }) if operation == "UTM"
    ));
}

#[test]
fn utm_to_coordinate_validates_the_record_and_wraps_longitude() {
    let record = |zone, easting, northing| Utm {
        zone,
        hemisphere: UtmHemisphere::North,
        easting,
        northing,
    };
    // Zone 1 west of the antimeridian wraps to +179.86° instead of failing.
    let wrapped = utm_to_coordinate(record(1, 150_000.0, 0.0)).unwrap();
    close(wrapped.lon, 179.856_227_657, 1e-8);
    // Each out-of-domain field is named.
    for (utm, field) in [
        (record(0, 500_000.0, 0.0), "zone"),
        (record(31, 99_999.0, 0.0), "easting"),
        (record(1, 500_000.0, 9_999_000.0), "northing"),
    ] {
        assert!(
            matches!(&utm_to_coordinate(utm), Err(GeoError::InvalidValue { field: f, .. }) if f == field),
            "{field}"
        );
    }
}

#[test]
fn utm_zone_helpers_validate_their_input() {
    assert_eq!(utm_zone_for(wgs84(40.0, -75.0)).unwrap(), 18);
    assert_eq!(utm_zone_for(wgs84(78.0, 20.0)).unwrap(), 33); // Svalbard
    assert!(matches!(
        utm_zone_for(wgs84(84.0, 9.0)),
        Err(GeoError::OutsideDomain { operation, .. }) if operation == "UTM"
    ));
    assert!(matches!(
        utm_zone_for(coordinate_new(40.0, -75.0, Crs::Gcj02).unwrap()),
        Err(GeoError::CrsMismatch { .. })
    ));
    close(utm_central_meridian_deg(18).unwrap(), -75.0, 1e-12);
    for zone in [0, 61] {
        assert!(matches!(
            utm_central_meridian_deg(zone),
            Err(GeoError::InvalidValue { field, .. }) if field == "zone"
        ));
    }
}

#[test]
fn ups_covers_the_poles_and_round_trips() {
    let ups = ups_from_coordinate(wgs84(85.0, 0.0)).unwrap();
    assert_eq!(ups.hemisphere, UtmHemisphere::North);
    close(ups.easting, 2_000_000.0, 1e-6);
    close(ups.northing, 1_444_542.608_617_322_5, 1e-6);
    let back = ups_to_coordinate(ups).unwrap();
    close(back.lat, 85.0, 1e-9);
    close(back.lon, 0.0, 1e-9);
    assert_eq!(back.crs, Crs::Wgs84);
    assert!(matches!(
        ups_from_coordinate(wgs84(40.0, 0.0)),
        Err(GeoError::OutsideDomain { operation, .. }) if operation == "UPS"
    ));
    let off_grid = Ups {
        hemisphere: UtmHemisphere::South,
        easting: 2_000_000.0,
        northing: 4_000_001.0,
    };
    assert!(matches!(
        ups_to_coordinate(off_grid),
        Err(GeoError::InvalidValue { field, .. }) if field == "northing"
    ));
}

#[test]
fn mgrs_encodes_decodes_and_reports_precision() {
    let origin = wgs84(40.0, -75.0);
    assert_eq!(
        mgrs_from_coordinate(origin, MgrsPrecision::M1).unwrap(),
        "18TWK0000027757"
    );
    assert_eq!(
        mgrs_from_coordinate(origin, MgrsPrecision::Km1).unwrap(),
        "18TWK0027"
    );
    assert_eq!(
        mgrs_precision("18TWK000277".to_string()).unwrap(),
        MgrsPrecision::M100
    );
    // Every mirror variant crosses both ways and keeps its side length.
    let sides = [
        (MgrsPrecision::Km100, 100_000),
        (MgrsPrecision::Km10, 10_000),
        (MgrsPrecision::Km1, 1_000),
        (MgrsPrecision::M100, 100),
        (MgrsPrecision::M10, 10),
        (MgrsPrecision::M1, 1),
    ];
    for (precision, meters) in sides {
        assert_eq!(mgrs_precision_meters(precision), meters);
        let code = mgrs_from_coordinate(origin, precision).unwrap();
        assert_eq!(mgrs_precision(code).unwrap(), precision);
    }
    assert!(matches!(
        mgrs_from_coordinate(gcj02(40.0, 116.0), MgrsPrecision::M1),
        Err(GeoError::CrsMismatch { .. })
    ));
    let square = mgrs_to_coordinate("18TWK0000027757".to_string()).unwrap();
    // Half-diagonal of a 1 m square, scaled by the UTM central scale factor.
    close(square.max_error_m, 0.5f64.sqrt() / 0.9996, 1e-12);
    close(square.coord.lat, 40.000_002_534_090_015, 1e-9);
    close(square.coord.lon, -74.999_994_142_434_64, 1e-9);
    // A latitude a few ulps south of the equator encodes to a reference that
    // decodes back across the boundary (it was `31MEA…`, which failed to parse).
    let equator = mgrs_from_coordinate(wgs84(-8e-15, 3.0), MgrsPrecision::M1).unwrap();
    assert_eq!(equator, "31NEA0000000000");
    let center = mgrs_to_coordinate(equator).unwrap();
    assert!(center.coord.lat.abs() < 1e-5 && (center.coord.lon - 3.0).abs() < 1e-5);
    // A square the antimeridian truncates decodes inside its own zone, so it
    // encodes back to the same reference (its nominal center is in zone 60),
    // with a bound from that point to the square's farthest corner.
    let cut = mgrs_to_coordinate("01NAA".to_string()).unwrap();
    assert!(cut.coord.lon > -180.0 && cut.coord.lon < -179.0, "{cut:?}");
    assert_eq!(
        mgrs_from_coordinate(cut.coord, MgrsPrecision::Km100).unwrap(),
        "01NAA"
    );
    let half_diagonal = 100_000.0 * 0.5f64.sqrt() / 0.9996;
    assert!(cut.max_error_m > half_diagonal && cut.max_error_m <= 2.0 * half_diagonal);
    assert!(matches!(
        mgrs_to_coordinate("not-an-mgrs".to_string()),
        Err(GeoError::InvalidGridRef { .. })
    ));
    assert!(matches!(
        mgrs_precision("not-an-mgrs".to_string()),
        Err(GeoError::InvalidGridRef { .. })
    ));
}

// --- Interchange formats ---

#[test]
fn from_geojson_reads_lon_lat_order() {
    let fixes =
        from_geojson(r#"{"type":"Point","coordinates":[2.0,9.0,100.0]}"#.to_string()).unwrap();
    assert_eq!(fixes.len(), 1);
    assert_eq!((fixes[0].coord.lat, fixes[0].coord.lon), (9.0, 2.0));
    assert_eq!(fixes[0].coord.crs, Crs::Wgs84);
    assert_eq!(
        fixes[0].source.as_ref().unwrap().axis_order,
        Some(AxisOrder::LonLat)
    );
    assert!(matches!(
        from_geojson("not json".to_string()),
        Err(GeoError::Parse { .. })
    ));
}

#[test]
fn from_wkt_reads_every_vertex_in_the_stated_axis_order() {
    let points = |order| -> Vec<(f64, f64)> {
        from_wkt("LINESTRING(0 0, 10 20)".to_string(), order)
            .unwrap()
            .iter()
            .map(|f| (f.coord.lat, f.coord.lon))
            .collect()
    };
    assert_eq!(points(AxisOrder::LonLat), vec![(0.0, 0.0), (20.0, 10.0)]);
    assert_eq!(points(AxisOrder::LatLon), vec![(0.0, 0.0), (10.0, 20.0)]);
    let fixes = from_wkt("POINT Z(1 2 3)".to_string(), AxisOrder::LatLon).unwrap();
    let source = fixes[0].source.as_ref().unwrap();
    assert_eq!(source.axis_order, Some(AxisOrder::LatLon));
    assert_eq!(source.raw, "POINT Z(1 2 3)");
    assert_eq!(
        fixes[0].coord.height,
        Some(Height::Ellipsoidal { meters: 3.0 })
    );
}

#[test]
fn from_gpx_reads_a_waypoint() {
    let fixes = from_gpx(
        concat!(
            r#"<?xml version="1.0"?><gpx version="1.1" creator="t" "#,
            r#"xmlns="http://www.topografix.com/GPX/1/1"><wpt lat="48.2" lon="16.3">"#,
            "<ele>183</ele><time>2024-03-01T12:00:05Z</time><hdop>1.5</hdop></wpt></gpx>",
        )
        .to_string(),
    )
    .unwrap();
    assert_eq!(fixes.len(), 1);
    assert_eq!((fixes[0].coord.lat, fixes[0].coord.lon), (48.2, 16.3));
    assert_eq!(
        fixes[0].coord.height,
        Some(Height::Orthometric { meters: 183.0 })
    );
    assert_eq!(
        fixes[0].timestamp,
        Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_709_294_405))
    );
    close(fixes[0].accuracy.unwrap().horizontal_m.unwrap(), 7.5, 1e-12);
    // GPX labels its axes with attributes, so no listing order is recorded.
    assert_eq!(fixes[0].source.as_ref().unwrap().axis_order, None);
}

#[test]
fn from_kml_reads_a_placemark() {
    let fixes = from_kml(
        concat!(
            r#"<?xml version="1.0"?><kml xmlns="http://www.opengis.net/kml/2.2"><Document>"#,
            "<Placemark><Point><coordinates>16.3,48.2,183</coordinates></Point></Placemark>",
            "</Document></kml>",
        )
        .to_string(),
    )
    .unwrap();
    assert_eq!(fixes.len(), 1);
    assert_eq!((fixes[0].coord.lat, fixes[0].coord.lon), (48.2, 16.3));
    assert_eq!(
        fixes[0].coord.height,
        Some(Height::Orthometric { meters: 183.0 })
    );
    assert_eq!(
        fixes[0].source.as_ref().unwrap().raw,
        "<Point><coordinates>16.3,48.2,183</coordinates></Point>"
    );
}

/// The detail of a `GeoError::Parse`, panicking on anything else.
fn parse_detail(r: Result<Vec<Fix>, GeoError>) -> String {
    match r {
        Err(GeoError::Parse { detail }) => detail,
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn interchange_parsers_reject_short_positions_and_deep_nesting() {
    // A one-number GeoJSON position is rejected at its own document index.
    let detail = parse_detail(from_geojson(
        r#"{"type":"LineString","coordinates":[[0,0],[5],[0,200]]}"#.to_string(),
    ));
    assert!(
        detail.starts_with("geojson: invalid position at index 1: "),
        "{detail}"
    );
    // 100 000 levels of nesting is an error, not a stack overflow.
    let wkt = "GEOMETRYCOLLECTION(".repeat(100_000);
    let detail = parse_detail(from_wkt(wkt, AxisOrder::LonLat));
    assert_eq!(detail, "wkt: nesting deeper than 128 levels");
    let kml = "<Folder>".repeat(100_000) + &"</Folder>".repeat(100_000);
    let detail = parse_detail(from_kml(kml));
    assert_eq!(detail, "kml: nesting deeper than 32 levels");
}

// --- NMEA 0183 ---

#[test]
fn from_nmea_sentence_reads_a_gga_fix() {
    let fix = from_nmea_sentence(
        "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47".to_string(),
    )
    .unwrap();
    close(fix.coord.lat, 48.0 + 7.038 / 60.0, 1e-12);
    close(fix.coord.lon, 11.0 + 31.0 / 60.0, 1e-12);
    assert_eq!(
        fix.coord.height,
        Some(Height::Orthometric { meters: 545.4 })
    );
    // Horizontal accuracy is HDOP times a nominal 5 m UERE, a one-sigma estimate.
    let accuracy = fix.accuracy.unwrap();
    assert_eq!(accuracy.kind, AccuracyKind::OneSigma);
    close(accuracy.horizontal_m.unwrap(), 4.5, 1e-12);
    let source = fix.source.unwrap();
    assert_eq!(source.axis_order, Some(AxisOrder::LatLon));
    // The geoid separation N is typed: h = H + N.
    assert_eq!(source.geoid_separation_m, Some(46.9));
    assert!(source.notes.is_empty());
    assert!(matches!(
        from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1E".to_string()),
        Err(GeoError::Parse { .. })
    ));
}

#[test]
fn from_nmea_sentence_timestamps_rmc_and_rejects_no_fix() {
    let rmc = from_nmea_sentence(
        "$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,*10".to_string(),
    )
    .unwrap();
    // 1994-03-23T12:35:19Z crosses as the native timestamp.
    assert_eq!(
        rmc.timestamp,
        Some(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(764_426_119))
    );
    for no_fix in [
        "$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07",
        "$GPGGA,123519,,,,,0,00,,,M,,M,,",
    ] {
        assert!(matches!(
            from_nmea_sentence(no_fix.to_string()),
            Err(GeoError::Parse { .. })
        ));
    }
}

// --- H3 and S2 ---

#[test]
fn h3_encodes_the_canonical_index_and_decodes_its_center() {
    let cell = h3_encode(wgs84(40.7128, -74.006), 9).unwrap();
    assert_eq!(cell.value, 617_733_151_020_810_239);
    let center = h3_decode(cell).unwrap();
    close(center.coord.lat, 40.712_378_204_427_85, 1e-9);
    close(center.coord.lon, -74.005_642_577_171_1, 1e-9);
    close(center.max_error_m, 205.872_540_844_922_98, 1e-6);
    assert!(matches!(
        h3_encode(gcj02(40.7128, -74.006), 9),
        Err(GeoError::CrsMismatch { .. })
    ));
    assert!(matches!(
        h3_decode(H3Cell { value: 0 }),
        Err(GeoError::InvalidCellId { value: 0, .. })
    ));
    assert_eq!(h3_resolution(cell).unwrap(), 9);
    assert!(matches!(
        h3_resolution(H3Cell { value: 0 }),
        Err(GeoError::InvalidCellId { value: 0, .. })
    ));
}

#[test]
fn s2_encodes_the_canonical_id_and_decodes_its_center() {
    let cell = s2_encode(wgs84(40.7128, -74.006), 20).unwrap();
    assert_eq!(cell.value, 9_926_595_630_970_437_632);
    let center = s2_decode(cell).unwrap();
    close(center.coord.lat, 40.712_762_828_727_77, 1e-9);
    close(center.coord.lon, -74.006_004_052_624_14, 1e-9);
    close(center.max_error_m, 6.337_540_947_116_765, 1e-9);
    assert!(matches!(
        s2_encode(bd09(40.7128, -74.006), 20),
        Err(GeoError::CrsMismatch { .. })
    ));
    assert_eq!(s2_level(cell).unwrap(), 20);
    assert!(matches!(
        s2_level(S2CellId { value: 0 }),
        Err(GeoError::InvalidCellId { value: 0, .. })
    ));
}
