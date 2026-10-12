//! Interchange formats (GeoJSON, WKT, GPX, KML) and NMEA sentences.

use geocoordinates::Height;
use geocoordinates::fix::AxisOrder;
use geocoordinates::parse::interchange::{from_geojson, from_gpx, from_kml, from_wkt};
use geocoordinates::parse::sensors::from_nmea_sentence;

use crate::support::{DemoResult, assert_close};

pub(crate) fn interchange_and_nmea() -> DemoResult {
    let geojson = from_geojson(r#"{"type":"Point","coordinates":[2.0,9.0,100.0]}"#)?;
    assert_eq!(geojson.len(), 1);
    assert_eq!(geojson[0].coord.height(), Some(Height::Ellipsoidal(100.0)));
    assert_close(geojson[0].coord.lat(), 9.0, 1e-12);
    assert_close(geojson[0].coord.lon(), 2.0, 1e-12);
    assert_eq!(
        geojson[0].source.as_ref().and_then(|s| s.axis_order),
        Some(AxisOrder::LonLat)
    );

    // Each fix's raw source is its own position, not the whole document.
    assert_eq!(
        geojson[0].source.as_ref().map(|s| s.raw.as_str()),
        Some(r#"{"type":"Point","coordinates":[2.0,9.0,100.0]}"#)
    );

    // WKT defines no axis order, so the caller states it.
    let wkt = from_wkt("LINESTRING(0 0, 10 20)", AxisOrder::LonLat)?;
    assert_eq!(wkt.len(), 2);
    assert_close(wkt[1].coord.lat(), 20.0, 1e-12);
    assert_close(wkt[1].coord.lon(), 10.0, 1e-12);
    let epsg_4326 = from_wkt("POINT(20 10)", AxisOrder::LatLon)?;
    assert_close(epsg_4326[0].coord.lat(), 20.0, 1e-12);

    let gpx = from_gpx(
        r#"<?xml version="1.0"?>
        <gpx version="1.1" creator="flagship" xmlns="http://www.topografix.com/GPX/1/1">
          <wpt lat="48.2" lon="16.3"><ele>183.0</ele>
            <time>2024-03-01T12:00:05Z</time><hdop>1.5</hdop></wpt>
        </gpx>"#,
    )?;
    assert_eq!(gpx.len(), 1);
    // GPX <ele> is mean-sea-level elevation; its axes are labelled attributes.
    assert_eq!(gpx[0].coord.height(), Some(Height::Orthometric(183.0)));
    assert_eq!(gpx[0].source.as_ref().and_then(|s| s.axis_order), None);
    assert!(gpx[0].timestamp.is_some());
    assert_close(
        gpx[0]
            .accuracy
            .and_then(|accuracy| accuracy.horizontal_m)
            .expect("GPX HDOP provides horizontal accuracy"),
        7.5,
        1e-9,
    );

    let kml = from_kml(
        r#"<?xml version="1.0"?>
        <kml xmlns="http://www.opengis.net/kml/2.2"><Document>
          <Placemark><Point><coordinates>16.3,48.2,183</coordinates></Point></Placemark>
        </Document></kml>"#,
    )?;
    assert_eq!(kml.len(), 1);
    assert_close(kml[0].coord.lat(), 48.2, 1e-9);
    assert_close(kml[0].coord.lon(), 16.3, 1e-9);
    // KML altitude is above sea level.
    assert_eq!(kml[0].coord.height(), Some(Height::Orthometric(183.0)));
    assert_eq!(
        kml[0].source.as_ref().and_then(|s| s.axis_order),
        Some(AxisOrder::LonLat)
    );

    let nmea =
        from_nmea_sentence("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47")?;
    assert_close(nmea.coord.lat(), 48.0 + 7.038 / 60.0, 1e-9);
    assert_close(nmea.coord.lon(), 11.0 + 31.0 / 60.0, 1e-9);
    assert_eq!(nmea.coord.height(), Some(Height::Orthometric(545.4)));
    assert_close(
        nmea.accuracy
            .and_then(|accuracy| accuracy.horizontal_m)
            .expect("GGA HDOP provides horizontal accuracy"),
        4.5,
        1e-9,
    );

    Ok("interchange and NMEA")
}
