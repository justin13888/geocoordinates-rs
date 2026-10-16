//! Structured interchange parsers: geometry walks, axis order, altitude
//! meaning, per-position raw fragments, GPX metadata, and indexed rejections.

use super::interchange::*;
use crate::coord::Height;
use crate::error::{Error, Result};
#[cfg(any(feature = "geojson", feature = "wkt", feature = "kml"))]
use crate::fix::AxisOrder;
use crate::fix::{Fix, RawSource};
use crate::test_support::assert_close;

/// The `(lat, lon)` of every fix, in emission order.
#[cfg(any(feature = "geojson", feature = "wkt", feature = "gpx"))]
fn lat_lons(fixes: &[Fix]) -> Vec<(f64, f64)> {
    fixes
        .iter()
        .map(|f| (f.coord.lat(), f.coord.lon()))
        .collect()
}

fn source(fix: &Fix) -> &RawSource {
    fix.source.as_ref().unwrap()
}

// The two assertion helpers are macros rather than functions: a `()`-returning
// helper is a mutation target whose deletion no test can observe.

/// A rejection is `Parse` whose message starts with `prefix`.
macro_rules! assert_parse_err {
    ($r:expr, $prefix:expr $(,)?) => {{
        let r: Result<Vec<Fix>> = $r;
        assert!(
            matches!(&r, Err(Error::Parse(m)) if m.starts_with($prefix)),
            "{r:?}"
        );
    }};
}

/// Every fix's raw fragment parses back, alone, to that same fix.
#[cfg(any(feature = "geojson", feature = "wkt", feature = "kml"))]
macro_rules! assert_fragments_reparse {
    ($fixes:expr, $parse:expr $(,)?) => {{
        let parse = $parse;
        for fix in $fixes.iter() {
            let again: Vec<Fix> = parse(&source(fix).raw).unwrap();
            assert_eq!(again, std::slice::from_ref(fix), "{}", source(fix).raw);
        }
    }};
}

#[cfg(feature = "geojson")]
#[test]
fn geojson_point_and_linestring() {
    // A single Point with altitude: lon-lat order, ellipsoidal height.
    let pt = from_geojson(r#"{"type":"Point","coordinates":[2.0,9.0,100.0]}"#).unwrap();
    assert_eq!(pt.len(), 1);
    assert_close(pt[0].coord.lat(), 9.0, 1e-12); // lat is the SECOND element
    assert_close(pt[0].coord.lon(), 2.0, 1e-12);
    assert_eq!(pt[0].coord.height(), Some(Height::Ellipsoidal(100.0)));
    let src = source(&pt[0]);
    assert_eq!(src.axis_order, Some(AxisOrder::LonLat));
    assert_close(src.confidence.value(), 1.0, 1e-12);
    assert!(src.notes.is_empty());
    // A FeatureCollection with a 3-vertex LineString yields three fixes.
    let fc = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","geometry":{"type":"LineString",
         "coordinates":[[0.0,0.0],[1.0,1.0],[2.0,3.0]]},"properties":null}]}"#;
    let line = from_geojson(fc).unwrap();
    assert_eq!(line.len(), 3);
    assert_close(line[2].coord.lat(), 3.0, 1e-12);
    assert_close(line[2].coord.lon(), 2.0, 1e-12);
}

#[cfg(feature = "geojson")]
#[test]
fn geojson_raw_is_each_positions_own_point() {
    let line = r#"{"type":"LineString","coordinates":[[1.5,2.25,7.0],[3,4]]}"#;
    let fixes = from_geojson(line).unwrap();
    let raws: Vec<_> = fixes.iter().map(|f| source(f).raw.clone()).collect();
    assert_eq!(
        raws,
        [
            r#"{"type":"Point","coordinates":[1.5,2.25,7.0]}"#,
            r#"{"type":"Point","coordinates":[3.0,4.0]}"#,
        ]
    );
    assert_fragments_reparse!(&fixes, from_geojson);
}

#[cfg(feature = "geojson")]
#[test]
fn geojson_walks_every_geometry_type_in_order() {
    let multi_point = r#"{"type":"MultiPoint","coordinates":[[1,2],[3,4]]}"#;
    assert_eq!(
        lat_lons(&from_geojson(multi_point).unwrap()),
        [(2.0, 1.0), (4.0, 3.0)]
    );
    let multi_line = r#"{"type":"MultiLineString","coordinates":[[[1,2],[3,4]],[[5,6]]]}"#;
    assert_eq!(
        lat_lons(&from_geojson(multi_line).unwrap()),
        [(2.0, 1.0), (4.0, 3.0), (6.0, 5.0)]
    );
    // Polygon rings are emitted vertex by vertex, closing vertex included,
    // outer ring then holes.
    let polygon = r#"{"type":"Polygon","coordinates":[
        [[0,0],[10,0],[10,10],[0,0]],[[1,1],[2,1],[1,1]]]}"#;
    assert_eq!(from_geojson(polygon).unwrap().len(), 7);
    let multi_polygon = r#"{"type":"MultiPolygon","coordinates":[
        [[[0,0],[1,0],[0,0]]],[[[5,5],[6,5],[5,5]],[[7,7],[8,7],[7,7]]]]}"#;
    let fixes = from_geojson(multi_polygon).unwrap();
    assert_eq!(fixes.len(), 9);
    assert_eq!((fixes[8].coord.lat(), fixes[8].coord.lon()), (7.0, 7.0));
    // A GeometryCollection recurses, nested collections included.
    let collection = r#"{"type":"GeometryCollection","geometries":[
        {"type":"Point","coordinates":[1,2]},
        {"type":"GeometryCollection","geometries":[
            {"type":"LineString","coordinates":[[3,4],[5,6]]}]}]}"#;
    let fixes = from_geojson(collection).unwrap();
    assert_eq!(lat_lons(&fixes), [(2.0, 1.0), (4.0, 3.0), (6.0, 5.0)]);
    assert_fragments_reparse!(&fixes, from_geojson);
}

#[cfg(feature = "geojson")]
#[test]
fn geojson_features_with_and_without_geometry() {
    // A bare Feature carrying a geometry.
    let feature = r#"{"type":"Feature","geometry":{"type":"Point","coordinates":[1,2]},
        "properties":{}}"#;
    assert_eq!(lat_lons(&from_geojson(feature).unwrap()), [(2.0, 1.0)]);
    // A null-geometry Feature is valid GeoJSON with no positions.
    let empty = r#"{"type":"Feature","geometry":null,"properties":null}"#;
    assert!(from_geojson(empty).unwrap().is_empty());
    // In a collection, null-geometry features are skipped.
    let fc = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","geometry":null,"properties":null},
        {"type":"Feature","geometry":{"type":"Point","coordinates":[3,4]},"properties":null}]}"#;
    assert_eq!(lat_lons(&from_geojson(fc).unwrap()), [(4.0, 3.0)]);
    // Without an altitude there is no height.
    assert_eq!(from_geojson(feature).unwrap()[0].coord.height(), None);
}

#[cfg(feature = "geojson")]
#[test]
fn geojson_rejections_are_parse_errors() {
    assert_parse_err!(from_geojson("not json"), "geojson: ");
    assert_parse_err!(
        from_geojson(r#"{"type":"Point","coordinates":[2.0,91.0]}"#),
        "geojson: invalid position at index 0: ",
    );
    // One bad vertex rejects the whole line, and the error names its index.
    assert_parse_err!(
        from_geojson(r#"{"type":"LineString","coordinates":[[0,0],[1,1],[181,0]]}"#),
        "geojson: invalid position at index 2: ",
    );
}

#[cfg(feature = "wkt")]
#[test]
fn wkt_reads_x_y_in_the_stated_axis_order() {
    let lon_lat = from_wkt("POINT(2 9)", AxisOrder::LonLat).unwrap();
    assert_eq!(lat_lons(&lon_lat), [(9.0, 2.0)]);
    assert_eq!(source(&lon_lat[0]).axis_order, Some(AxisOrder::LonLat));
    assert_eq!(source(&lon_lat[0]).raw, "POINT(2 9)");
    // The same text read lat-first (EPSG:4326 strict) transposes.
    let lat_lon = from_wkt("POINT(2 9)", AxisOrder::LatLon).unwrap();
    assert_eq!(lat_lons(&lat_lon), [(2.0, 9.0)]);
    assert_eq!(source(&lat_lon[0]).axis_order, Some(AxisOrder::LatLon));
    // A latitude-range check follows the stated order: 120 is a valid
    // longitude but not a valid latitude.
    assert!(from_wkt("POINT(120 10)", AxisOrder::LonLat).is_ok());
    assert_parse_err!(
        from_wkt("POINT(120 10)", AxisOrder::LatLon),
        "wkt: invalid position at index 0: ",
    );
    let line = from_wkt("LINESTRING(0 0, 10 20, 30 40)", AxisOrder::LonLat).unwrap();
    assert_eq!(line.len(), 3);
    assert_close(line[1].coord.lat(), 20.0, 1e-12);
    assert_close(line[1].coord.lon(), 10.0, 1e-12);
    for order in [AxisOrder::LonLat, AxisOrder::LatLon] {
        let fixes = from_wkt("LINESTRING(1 2, 3 4)", order).unwrap();
        assert_eq!(source(&fixes[1]).raw, "POINT(3 4)");
        assert_fragments_reparse!(&fixes, |raw| from_wkt(raw, order));
    }
}

#[cfg(feature = "wkt")]
#[test]
fn wkt_walks_every_geometry_type_in_order() {
    let xy = |input| from_wkt(input, AxisOrder::LonLat).unwrap();
    assert_eq!(
        lat_lons(&xy("MULTIPOINT((1 2),(3 4))")),
        [(2.0, 1.0), (4.0, 3.0)]
    );
    assert_eq!(
        lat_lons(&xy("MULTILINESTRING((1 2,3 4),(5 6,7 8))")),
        [(2.0, 1.0), (4.0, 3.0), (6.0, 5.0), (8.0, 7.0)]
    );
    // Outer ring then holes, closing vertices included.
    assert_eq!(xy("POLYGON((0 0,10 0,10 10,0 0),(1 1,2 1,1 1))").len(), 7);
    let fixes = xy("MULTIPOLYGON(((0 0,1 0,0 0)),((5 5,6 5,5 5),(7 7,8 7,7 7)))");
    assert_eq!(fixes.len(), 9);
    assert_eq!((fixes[8].coord.lat(), fixes[8].coord.lon()), (7.0, 7.0));
    let fixes = xy("GEOMETRYCOLLECTION(POINT(1 2),LINESTRING(3 4,5 6))");
    assert_eq!(lat_lons(&fixes), [(2.0, 1.0), (4.0, 3.0), (6.0, 5.0)]);
    assert_fragments_reparse!(&fixes, |raw| from_wkt(raw, AxisOrder::LonLat));
}

#[cfg(feature = "wkt")]
#[test]
fn wkt_empty_and_dimensioned_geometries() {
    let xy = |input| from_wkt(input, AxisOrder::LonLat).unwrap();
    // EMPTY geometries are valid WKT with no positions.
    assert!(xy("POINT EMPTY").is_empty());
    assert!(xy("LINESTRING EMPTY").is_empty());
    assert!(xy("GEOMETRYCOLLECTION EMPTY").is_empty());
    assert!(xy("MULTIPOINT EMPTY").is_empty());
    // Z is read as an ellipsoidal height, with a note that WKT does not say.
    let z = xy("POINT Z(1 2 3)");
    assert_eq!(z[0].coord.height(), Some(Height::Ellipsoidal(3.0)));
    assert_eq!(source(&z[0]).raw, "POINT Z(1 2 3)");
    let notes = &source(&z[0]).notes;
    assert!(
        notes.len() == 1 && notes[0].contains("vertical reference"),
        "{notes:?}"
    );
    // M is a measure, not a height: it is dropped, and so is the note.
    let m = xy("POINT M(1 2 3)");
    assert_eq!(lat_lons(&m), [(2.0, 1.0)]);
    assert_eq!(m[0].coord.height(), None);
    assert!(source(&m[0]).notes.is_empty());
    assert_eq!(source(&m[0]).raw, "POINT M(1 2 3)");
    let zm = xy("POINT ZM(1 2 3 4)");
    assert_eq!(zm[0].coord.height(), Some(Height::Ellipsoidal(3.0)));
    for fixes in [z, m, zm] {
        assert_fragments_reparse!(&fixes, |raw| from_wkt(raw, AxisOrder::LonLat));
    }
}

#[cfg(feature = "wkt")]
#[test]
fn wkt_rejections_are_parse_errors() {
    assert_parse_err!(from_wkt("CIRCLE(0 0 5)", AxisOrder::LonLat), "wkt: ");
    assert_parse_err!(
        from_wkt("POINT(181 0)", AxisOrder::LonLat),
        "wkt: invalid position at index 0: ",
    );
    assert_parse_err!(
        from_wkt("LINESTRING(0 0, 0 -91)", AxisOrder::LonLat),
        "wkt: invalid position at index 1: ",
    );
}

#[cfg(feature = "gpx")]
const GPX_HEAD: &str = r#"<?xml version="1.0"?>
    <gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">"#;

#[cfg(feature = "gpx")]
#[test]
fn gpx_waypoint_and_track() {
    let doc = format!(
        r#"{GPX_HEAD}
          <wpt lat="48.2" lon="16.3"><ele>183.0</ele></wpt>
          <trk><trkseg>
            <trkpt lat="40.0" lon="-75.0"/>
            <trkpt lat="41.0" lon="-74.0"/>
          </trkseg></trk>
        </gpx>"#
    );
    let fixes = from_gpx(&doc).unwrap();
    assert_eq!(fixes.len(), 3); // 1 waypoint + 2 track points
    assert_close(fixes[0].coord.lat(), 48.2, 1e-9);
    assert_close(fixes[0].coord.lon(), 16.3, 1e-9);
    // <ele> is mean-sea-level elevation.
    assert_eq!(fixes[0].coord.height(), Some(Height::Orthometric(183.0)));
    // GPX labels its axes with attributes, so no listing order is recorded.
    assert!(fixes.iter().all(|f| source(f).axis_order.is_none()));
    assert_eq!(
        source(&fixes[0]).raw,
        r#"<wpt lat="48.2" lon="16.3"><ele>183</ele></wpt>"#
    );
    assert_eq!(source(&fixes[2]).raw, r#"<trkpt lat="41" lon="-74"/>"#);
    assert_close(fixes[2].coord.lat(), 41.0, 1e-9);
    // A point without <time> or <hdop> carries neither.
    assert_eq!((fixes[1].timestamp, fixes[1].accuracy), (None, None));
}

#[cfg(feature = "gpx")]
#[test]
fn gpx_keeps_time_and_hdop() {
    use std::time::{Duration, SystemTime};

    let doc = format!(
        r#"{GPX_HEAD}<trk><trkseg>
          <trkpt lat="1" lon="2"><time>2024-03-01T12:00:05Z</time><hdop>1.5</hdop></trkpt>
          <trkpt lat="3" lon="4"><time>2024-03-01T14:00:05+02:00</time></trkpt>
        </trkseg></trk></gpx>"#
    );
    let fixes = from_gpx(&doc).unwrap();
    let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(1_709_294_405);
    assert_eq!(fixes[0].timestamp, Some(expected));
    // An offset is honoured: 14:00:05+02:00 is the same instant.
    assert_eq!(fixes[1].timestamp, Some(expected));
    // HDOP × 5 m nominal UERE, the NMEA GGA scaling.
    let accuracy = fixes[0].accuracy.unwrap();
    assert_close(accuracy.horizontal.unwrap().meters(), 7.5, 1e-12);
    assert_eq!(accuracy.vertical, None);
    assert_eq!(fixes[1].accuracy, None);
}

#[cfg(feature = "gpx")]
#[test]
fn gpx_emits_waypoints_then_tracks_then_routes() {
    let doc = format!(
        r#"{GPX_HEAD}
          <rte>
            <rtept lat="10.0" lon="20.0"/>
            <rtept lat="11.0" lon="21.0"><ele>5.5</ele></rtept>
          </rte>
          <trk><trkseg><trkpt lat="1.0" lon="2.0"/></trkseg>
               <trkseg><trkpt lat="3.0" lon="4.0"/></trkseg></trk>
          <wpt lat="-1.0" lon="-2.0"/>
        </gpx>"#
    );
    let fixes = from_gpx(&doc).unwrap();
    assert_eq!(
        lat_lons(&fixes),
        [
            (-1.0, -2.0),
            (1.0, 2.0),
            (3.0, 4.0),
            (10.0, 20.0),
            (11.0, 21.0)
        ]
    );
    assert_eq!(fixes[4].coord.height(), Some(Height::Orthometric(5.5)));
    assert_eq!(fixes[3].coord.height(), None);
    let tags: Vec<_> = fixes
        .iter()
        .map(|f| source(f).raw.split(' ').next().unwrap().to_string())
        .collect();
    assert_eq!(tags, ["<wpt", "<trkpt", "<trkpt", "<rtept", "<rtept"]);
    assert_eq!(
        source(&fixes[4]).raw,
        r#"<rtept lat="11" lon="21"><ele>5.5</ele></rtept>"#
    );
    // A document with no points has no fixes.
    assert!(from_gpx(&format!("{GPX_HEAD}</gpx>")).unwrap().is_empty());
}

#[cfg(feature = "gpx")]
#[test]
fn gpx_rejections_are_parse_errors() {
    assert_parse_err!(from_gpx("<gpx>broken"), "gpx: ");
    let negative_hdop = format!(
        r#"{GPX_HEAD}<wpt lat="1" lon="2"/><wpt lat="1" lon="2"><hdop>-1</hdop></wpt></gpx>"#
    );
    assert_parse_err!(
        from_gpx(&negative_hdop),
        "gpx: invalid position at index 1: ",
    );
    let infinite_hdop = format!(r#"{GPX_HEAD}<wpt lat="1" lon="2"><hdop>inf</hdop></wpt></gpx>"#);
    assert_parse_err!(
        from_gpx(&infinite_hdop),
        "gpx: invalid position at index 0: ",
    );
    // A zero HDOP is valid, and scales to a zero radius.
    let zero_hdop = format!(r#"{GPX_HEAD}<wpt lat="1" lon="2"><hdop>0</hdop></wpt></gpx>"#);
    let accuracy = from_gpx(&zero_hdop).unwrap()[0].accuracy.unwrap();
    assert_eq!(accuracy.horizontal, Some(crate::Length::ZERO));
    // The gpx reader range-checks attributes itself, before any index exists.
    let off_globe = format!(r#"{GPX_HEAD}<wpt lat="1" lon="2"/><wpt lat="1" lon="200"/></gpx>"#);
    assert_parse_err!(from_gpx(&off_globe), "gpx: ");
}

#[cfg(feature = "kml")]
#[test]
fn kml_placemark_point() {
    // KML coordinates are lon,lat,alt; altitude is above sea level.
    let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
        <kml xmlns="http://www.opengis.net/kml/2.2"><Document>
          <Placemark><Point><coordinates>16.3,48.2,183</coordinates></Point></Placemark>
          <Placemark><Point><coordinates>1.5,2</coordinates></Point></Placemark>
        </Document></kml>"#;
    let fixes = from_kml(doc).unwrap();
    assert_eq!(fixes.len(), 2);
    assert_close(fixes[0].coord.lat(), 48.2, 1e-9);
    assert_close(fixes[0].coord.lon(), 16.3, 1e-9);
    assert_eq!(fixes[0].coord.height(), Some(Height::Orthometric(183.0)));
    assert_eq!(fixes[1].coord.height(), None);
    assert_eq!(source(&fixes[0]).axis_order, Some(AxisOrder::LonLat));
    assert_eq!(
        source(&fixes[0]).raw,
        "<Point><coordinates>16.3,48.2,183</coordinates></Point>"
    );
    assert_eq!(
        source(&fixes[1]).raw,
        "<Point><coordinates>1.5,2</coordinates></Point>"
    );
    assert_fragments_reparse!(&fixes, from_kml);
}

#[cfg(feature = "kml")]
#[test]
fn kml_exercises_every_geometry_arm() {
    // Folders, bare geometries, and placemark-wrapped geometries — covering
    // every Kml/Geometry variant the walker handles.
    let doc = r#"<?xml version="1.0"?>
        <kml xmlns="http://www.opengis.net/kml/2.2"><Folder>
          <Point><coordinates>1,1</coordinates></Point>
          <LineString><coordinates>2,2 3,3</coordinates></LineString>
          <LinearRing><coordinates>4,4 5,5 6,6 4,4</coordinates></LinearRing>
          <Polygon><outerBoundaryIs><LinearRing>
            <coordinates>7,7 8,8 9,9 7,7</coordinates></LinearRing></outerBoundaryIs></Polygon>
          <MultiGeometry><Point><coordinates>10,10</coordinates></Point></MultiGeometry>
          <Placemark><LineString><coordinates>11,11 12,12</coordinates></LineString></Placemark>
          <Placemark><Polygon><outerBoundaryIs><LinearRing>
            <coordinates>13,13 14,14 15,15 13,13</coordinates>
            </LinearRing></outerBoundaryIs></Polygon></Placemark>
          <Placemark><MultiGeometry>
            <Point><coordinates>19,19</coordinates></Point></MultiGeometry></Placemark>
          <Placemark><LinearRing>
            <coordinates>20,20 21,21 22,22 20,20</coordinates></LinearRing></Placemark>
        </Folder></kml>"#;
    let fixes = from_kml(doc).unwrap();
    // 1 + 2 + 4 + 4 + 1 (bare) + 2 + 4 + 1 + 4 (placemark) = 23 positions;
    // every geometry/walk arm contributes, so deleting any one is caught.
    assert_eq!(fixes.len(), 23);
    assert_fragments_reparse!(&fixes, from_kml);
}

#[cfg(feature = "kml")]
#[test]
fn kml_polygon_inner_rings_and_rejections() {
    // Inner boundaries are walked after the outer one, for a bare polygon
    // and a placemark-wrapped one alike.
    let ring = "<LinearRing><coordinates>0,0 1,0 0,0</coordinates></LinearRing>";
    let polygon = format!(
        "<Polygon><outerBoundaryIs>{ring}</outerBoundaryIs>\
         <innerBoundaryIs>{ring}</innerBoundaryIs></Polygon>"
    );
    let doc = format!(
        r#"<kml xmlns="http://www.opengis.net/kml/2.2"><Folder>{polygon}
            <Placemark>{polygon}</Placemark></Folder></kml>"#
    );
    assert_eq!(from_kml(&doc).unwrap().len(), 12);
    // Mismatched tags and a non-numeric coordinate are parse errors. (A
    // truncated document such as `<kml><Point>` is not exercised: the
    // `kml` crate never returns on it, tracked in #135.)
    assert_parse_err!(from_kml("<kml></Point></kml>"), "kml: ");
    assert_parse_err!(
        from_kml("<kml><Point><coordinates>abc</coordinates></Point></kml>"),
        "kml: ",
    );
    assert_parse_err!(from_kml("not xml"), "kml: ");
    let off_globe = r#"<kml xmlns="http://www.opengis.net/kml/2.2">
        <Placemark><LineString><coordinates>0,0 0,95</coordinates></LineString></Placemark></kml>"#;
    assert_parse_err!(from_kml(off_globe), "kml: invalid position at index 1: ");
}
