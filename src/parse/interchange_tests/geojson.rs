//! The GeoJSON reader: geometry walks, per-position raw fragments, and
//! indexed rejections.

use super::*;

#[test]
fn geojson_point_and_linestring() {
    // A single Point with altitude: lon-lat order, ellipsoidal height.
    let pt = parse_geojson(r#"{"type":"Point","coordinates":[2.0,9.0,100.0]}"#).unwrap();
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
    let line = parse_geojson(fc).unwrap();
    assert_eq!(line.len(), 3);
    assert_close(line[2].coord.lat(), 3.0, 1e-12);
    assert_close(line[2].coord.lon(), 2.0, 1e-12);
}

#[test]
fn geojson_raw_is_each_positions_own_point() {
    let line = r#"{"type":"LineString","coordinates":[[1.5,2.25,7.0],[3,4]]}"#;
    let fixes = parse_geojson(line).unwrap();
    let raws: Vec<_> = fixes.iter().map(|f| source(f).raw.clone()).collect();
    assert_eq!(
        raws,
        [
            r#"{"type":"Point","coordinates":[1.5,2.25,7.0]}"#,
            r#"{"type":"Point","coordinates":[3.0,4.0]}"#,
        ]
    );
    assert_fragments_reparse!(&fixes, parse_geojson);
}

#[test]
fn geojson_walks_every_geometry_type_in_order() {
    let multi_point = r#"{"type":"MultiPoint","coordinates":[[1,2],[3,4]]}"#;
    assert_eq!(
        lat_lons(&parse_geojson(multi_point).unwrap()),
        [(2.0, 1.0), (4.0, 3.0)]
    );
    let multi_line = r#"{"type":"MultiLineString","coordinates":[[[1,2],[3,4]],[[5,6]]]}"#;
    assert_eq!(
        lat_lons(&parse_geojson(multi_line).unwrap()),
        [(2.0, 1.0), (4.0, 3.0), (6.0, 5.0)]
    );
    // Polygon rings are emitted vertex by vertex, closing vertex included,
    // outer ring then holes.
    let polygon = r#"{"type":"Polygon","coordinates":[
        [[0,0],[10,0],[10,10],[0,0]],[[1,1],[2,1],[1,1]]]}"#;
    assert_eq!(parse_geojson(polygon).unwrap().len(), 7);
    let multi_polygon = r#"{"type":"MultiPolygon","coordinates":[
        [[[0,0],[1,0],[0,0]]],[[[5,5],[6,5],[5,5]],[[7,7],[8,7],[7,7]]]]}"#;
    let fixes = parse_geojson(multi_polygon).unwrap();
    assert_eq!(fixes.len(), 9);
    assert_eq!((fixes[8].coord.lat(), fixes[8].coord.lon()), (7.0, 7.0));
    // A GeometryCollection recurses, nested collections included.
    let collection = r#"{"type":"GeometryCollection","geometries":[
        {"type":"Point","coordinates":[1,2]},
        {"type":"GeometryCollection","geometries":[
            {"type":"LineString","coordinates":[[3,4],[5,6]]}]}]}"#;
    let fixes = parse_geojson(collection).unwrap();
    assert_eq!(lat_lons(&fixes), [(2.0, 1.0), (4.0, 3.0), (6.0, 5.0)]);
    assert_fragments_reparse!(&fixes, parse_geojson);
}

#[test]
fn geojson_features_with_and_without_geometry() {
    // A bare Feature carrying a geometry.
    let feature = r#"{"type":"Feature","geometry":{"type":"Point","coordinates":[1,2]},
        "properties":{}}"#;
    assert_eq!(lat_lons(&parse_geojson(feature).unwrap()), [(2.0, 1.0)]);
    // A null-geometry Feature is valid GeoJSON with no positions.
    let empty = r#"{"type":"Feature","geometry":null,"properties":null}"#;
    assert!(parse_geojson(empty).unwrap().is_empty());
    // In a collection, null-geometry features are skipped.
    let fc = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","geometry":null,"properties":null},
        {"type":"Feature","geometry":{"type":"Point","coordinates":[3,4]},"properties":null}]}"#;
    assert_eq!(lat_lons(&parse_geojson(fc).unwrap()), [(4.0, 3.0)]);
    // Without an altitude there is no height.
    assert_eq!(parse_geojson(feature).unwrap()[0].coord.height(), None);
}

#[test]
fn geojson_rejections_are_parse_errors() {
    assert_parse_err!(parse_geojson("not json"), "geojson: ");
    // An out-of-range position keeps its typed error and offending values.
    let point = parse_geojson(r#"{"type":"Point","coordinates":[2.0,91.0]}"#);
    assert_eq!(point, out_of_range(91.0, 2.0));
    // One bad vertex rejects the whole line.
    let line = parse_geojson(r#"{"type":"LineString","coordinates":[[0,0],[1,1],[181,0]]}"#);
    assert_eq!(line, out_of_range(0.0, 181.0));
    // `serde_json`'s 128-level recursion limit is a `TooDeep` rejection.
    let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
    let doc = format!(r#"{{"type":"MultiPoint","coordinates":{deep}}}"#);
    assert_too_deep!(
        parse_geojson(&doc),
        "geojson: nesting deeper than 128 levels"
    );
}

#[test]
fn geojson_short_positions_are_rejected_at_their_document_index() {
    // `[5]` is position 1: it is rejected, not dropped, so nothing after it
    // shifts down an index.
    assert_parse_err!(
        parse_geojson(r#"{"type":"LineString","coordinates":[[0,0],[5],[0,200]]}"#),
        "geojson: invalid position at index 1: a position needs at least 2 numbers, got 1",
    );
    // An empty position in a polygon ring, inside a collection, counts every
    // position walked before it.
    let doc = r#"{"type":"GeometryCollection","geometries":[
        {"type":"MultiPoint","coordinates":[[0,0],[1,1]]},
        {"type":"Polygon","coordinates":[[[0,0],[1,0],[]]]}]}"#;
    assert_parse_err!(
        parse_geojson(doc),
        "geojson: invalid position at index 4: a position needs at least 2 numbers, got 0",
    );
    // A feature collection walks every feature's geometry, skipping null ones.
    let doc = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":null,"geometry":{"type":"Point","coordinates":[0,0]}},
        {"type":"Feature","properties":null,"geometry":null},
        {"type":"Feature","properties":null,"geometry":{"type":"MultiPolygon","coordinates":[[[[0,0],[7]]]]}}]}"#;
    assert_parse_err!(parse_geojson(doc), "geojson: invalid position at index 2: ",);
    let feature = r#"{"type":"Feature","properties":null,
        "geometry":{"type":"MultiLineString","coordinates":[[[0,0]],[[1]]]}}"#;
    assert_parse_err!(
        parse_geojson(feature),
        "geojson: invalid position at index 1: ",
    );
    // An out-of-range position before the short one is still the first error.
    let r = parse_geojson(r#"{"type":"LineString","coordinates":[[0,0],[0,200],[5]]}"#);
    assert_eq!(r, out_of_range(200.0, 0.0));
}
