//! Structured interchange formats: GeoJSON, WKT, GPX, KML.
//!
//! **Axis-order trap:** GeoJSON and WKT positions are lon-lat (X,Y), and KML
//! `coordinates` are lon,lat,alt; GPX uses explicit `lat`/`lon` attributes.
//! These parsers normalize every position to the library's lat/lon model and
//! record the source ordering on each [`Fix`]'s [`RawSource`] — they never
//! silently transpose.
//!
//! Every position becomes one [`Fix`] (so a `LineString` of *n* vertices yields
//! *n* fixes), tagged with confidence `1.0` because the structured input is
//! unambiguous. Each format is behind its own cargo feature.

use crate::coord::{Coordinate, Height};
use crate::error::{Error, Result};
use crate::fix::{AxisOrder, Confidence, Fix, RawSource};

/// Build a [`Fix`] from a single position, recording the source axis order.
fn position_fix(lon: f64, lat: f64, alt: Option<f64>, axis: AxisOrder, raw: &str) -> Fix {
    let mut coord = Coordinate::wgs84(lat, lon);
    if let Some(a) = alt {
        coord = coord.with_height(Height::Ellipsoidal(a));
    }
    Fix {
        coord,
        accuracy: None,
        timestamp: None,
        source: Some(RawSource {
            raw: raw.to_string(),
            confidence: Confidence::new(1.0),
            axis_order: Some(axis),
            datum_ambiguity: None,
            notes: Vec::new(),
        }),
    }
}

fn validate_positions(mut positions: Vec<Fix>, input: &str, format: &str) -> Result<Vec<Fix>> {
    for fix in &mut positions {
        fix.coord
            .validate()
            .map_err(|e| Error::Parse(format!("{format}: invalid position: {e}")))?;
        if let Some(source) = &mut fix.source {
            source.raw = input.to_string();
        }
    }
    Ok(positions)
}

/// Parse one or more positions from a GeoJSON value (lon-lat order).
///
/// Walks the whole structure — features, geometry collections, and every
/// vertex of lines and polygons — emitting one [`Fix`] per position.
///
/// # Errors
/// Returns [`crate::Error::Parse`] on malformed GeoJSON.
#[cfg(feature = "geojson")]
pub fn from_geojson(input: &str) -> Result<Vec<Fix>> {
    use geojson::{GeoJson, Geometry, GeometryValue, Position};

    fn position(p: &Position, out: &mut Vec<Fix>) {
        if p.len() >= 2 {
            let alt = if p.len() >= 3 { Some(p[2]) } else { None };
            out.push(position_fix(p[0], p[1], alt, AxisOrder::LonLat, "geojson"));
        }
    }
    fn geometry(g: &Geometry, out: &mut Vec<Fix>) {
        match &g.value {
            GeometryValue::Point { coordinates } => position(coordinates, out),
            GeometryValue::MultiPoint { coordinates }
            | GeometryValue::LineString { coordinates } => {
                coordinates.iter().for_each(|p| position(p, out));
            }
            GeometryValue::MultiLineString { coordinates }
            | GeometryValue::Polygon { coordinates } => {
                coordinates.iter().flatten().for_each(|p| position(p, out));
            }
            GeometryValue::MultiPolygon { coordinates } => {
                coordinates
                    .iter()
                    .flatten()
                    .flatten()
                    .for_each(|p| position(p, out));
            }
            GeometryValue::GeometryCollection { geometries } => {
                geometries.iter().for_each(|g| geometry(g, out));
            }
        }
    }

    let parsed: GeoJson = input
        .parse()
        .map_err(|e| Error::Parse(format!("geojson: {e}")))?;
    let mut out = Vec::new();
    match parsed {
        GeoJson::Geometry(g) => geometry(&g, &mut out),
        GeoJson::Feature(f) => {
            if let Some(g) = f.geometry {
                geometry(&g, &mut out);
            }
        }
        GeoJson::FeatureCollection(fc) => fc.features.iter().for_each(|f| {
            if let Some(g) = &f.geometry {
                geometry(g, &mut out);
            }
        }),
    }
    validate_positions(out, input, "geojson")
}

/// Parse a coordinate/geometry from a WKT string (X Y order).
///
/// # Errors
/// Returns [`crate::Error::Parse`] on malformed WKT.
#[cfg(feature = "wkt")]
pub fn from_wkt(input: &str) -> Result<Vec<Fix>> {
    use core::str::FromStr;

    use wkt::Wkt;
    use wkt::types::Coord;

    fn coord(c: &Coord<f64>, out: &mut Vec<Fix>) {
        out.push(position_fix(c.x, c.y, c.z, AxisOrder::LonLat, "wkt"));
    }
    fn geometry(g: &Wkt<f64>, out: &mut Vec<Fix>) {
        match g {
            Wkt::Point(p) => {
                if let Some(c) = p.coord() {
                    coord(c, out);
                }
            }
            Wkt::LineString(ls) => ls.coords().iter().for_each(|c| coord(c, out)),
            Wkt::Polygon(poly) => poly
                .rings()
                .iter()
                .for_each(|ls| ls.coords().iter().for_each(|c| coord(c, out))),
            Wkt::MultiPoint(mp) => mp.points().iter().for_each(|p| {
                if let Some(c) = p.coord() {
                    coord(c, out);
                }
            }),
            Wkt::MultiLineString(mls) => mls
                .line_strings()
                .iter()
                .for_each(|ls| ls.coords().iter().for_each(|c| coord(c, out))),
            Wkt::MultiPolygon(mp) => mp.polygons().iter().for_each(|poly| {
                poly.rings()
                    .iter()
                    .for_each(|ls| ls.coords().iter().for_each(|c| coord(c, out)));
            }),
            Wkt::GeometryCollection(gc) => gc.geometries().iter().for_each(|g| geometry(g, out)),
        }
    }

    let parsed = Wkt::<f64>::from_str(input).map_err(|e| Error::Parse(format!("wkt: {e}")))?;
    let mut out = Vec::new();
    geometry(&parsed, &mut out);
    validate_positions(out, input, "wkt")
}

/// Parse track/route/waypoint positions from a GPX document.
///
/// # Errors
/// Returns [`crate::Error::Parse`] on malformed GPX.
#[cfg(feature = "gpx")]
pub fn from_gpx(input: &str) -> Result<Vec<Fix>> {
    fn push(wp: &gpx::Waypoint, out: &mut Vec<Fix>) {
        let p = wp.point(); // geo-types Point: x = lon, y = lat
        out.push(position_fix(
            p.x(),
            p.y(),
            wp.elevation,
            AxisOrder::LatLon,
            "gpx",
        ));
    }

    let data = gpx::read(input.as_bytes()).map_err(|e| Error::Parse(format!("gpx: {e}")))?;
    let mut out = Vec::new();
    for wp in &data.waypoints {
        push(wp, &mut out);
    }
    for track in &data.tracks {
        for segment in &track.segments {
            for wp in &segment.points {
                push(wp, &mut out);
            }
        }
    }
    for route in &data.routes {
        for wp in &route.points {
            push(wp, &mut out);
        }
    }
    validate_positions(out, input, "gpx")
}

/// Parse placemark positions from a KML document.
///
/// # Errors
/// Returns [`crate::Error::Parse`] on malformed KML.
#[cfg(feature = "kml")]
pub fn from_kml(input: &str) -> Result<Vec<Fix>> {
    use core::str::FromStr;

    use kml::Kml;
    use kml::types::{Coord, Geometry};

    fn coord(c: &Coord<f64>, out: &mut Vec<Fix>) {
        out.push(position_fix(c.x, c.y, c.z, AxisOrder::LonLat, "kml"));
    }
    fn geometry(g: &Geometry<f64>, out: &mut Vec<Fix>) {
        match g {
            Geometry::Point(p) => coord(&p.coord, out),
            Geometry::LineString(ls) => ls.coords.iter().for_each(|c| coord(c, out)),
            Geometry::LinearRing(lr) => lr.coords.iter().for_each(|c| coord(c, out)),
            Geometry::Polygon(poly) => {
                poly.outer.coords.iter().for_each(|c| coord(c, out));
                poly.inner
                    .iter()
                    .for_each(|r| r.coords.iter().for_each(|c| coord(c, out)));
            }
            Geometry::MultiGeometry(mg) => mg.geometries.iter().for_each(|g| geometry(g, out)),
            _ => {} // Element / future variants carry no coordinates
        }
    }
    fn walk(k: &Kml<f64>, out: &mut Vec<Fix>) {
        match k {
            Kml::Point(p) => coord(&p.coord, out),
            Kml::LineString(ls) => ls.coords.iter().for_each(|c| coord(c, out)),
            Kml::LinearRing(lr) => lr.coords.iter().for_each(|c| coord(c, out)),
            Kml::Polygon(poly) => {
                poly.outer.coords.iter().for_each(|c| coord(c, out));
                poly.inner
                    .iter()
                    .for_each(|r| r.coords.iter().for_each(|c| coord(c, out)));
            }
            Kml::MultiGeometry(mg) => mg.geometries.iter().for_each(|g| geometry(g, out)),
            Kml::Placemark(pm) => {
                if let Some(g) = &pm.geometry {
                    geometry(g, out);
                }
            }
            Kml::Document { elements, .. } => elements.iter().for_each(|e| walk(e, out)),
            Kml::Folder(f) => f.elements.iter().for_each(|e| walk(e, out)),
            Kml::KmlDocument(doc) => doc.elements.iter().for_each(|e| walk(e, out)),
            _ => {}
        }
    }

    let parsed = Kml::<f64>::from_str(input).map_err(|e| Error::Parse(format!("kml: {e}")))?;
    let mut out = Vec::new();
    walk(&parsed, &mut out);
    validate_positions(out, input, "kml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coord::Height;
    use crate::test_support::assert_close;

    #[cfg(feature = "geojson")]
    #[test]
    fn geojson_point_and_linestring() {
        // A single Point with altitude.
        let pt = from_geojson(r#"{"type":"Point","coordinates":[2.0,9.0,100.0]}"#).unwrap();
        assert_eq!(pt.len(), 1);
        assert_close(pt[0].coord.lat, 9.0, 1e-12); // lat is the SECOND element
        assert_close(pt[0].coord.lon, 2.0, 1e-12);
        assert_eq!(pt[0].coord.height, Some(Height::Ellipsoidal(100.0)));
        let src = pt[0].source.as_ref().unwrap();
        assert_eq!(src.axis_order, Some(AxisOrder::LonLat));
        assert_close(src.confidence.value(), 1.0, 1e-12);
        assert_eq!(src.raw, r#"{"type":"Point","coordinates":[2.0,9.0,100.0]}"#);
        // A FeatureCollection with a 3-vertex LineString yields three fixes.
        let fc = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","geometry":{"type":"LineString",
             "coordinates":[[0.0,0.0],[1.0,1.0],[2.0,3.0]]},"properties":null}]}"#;
        let line = from_geojson(fc).unwrap();
        assert_eq!(line.len(), 3);
        assert_close(line[2].coord.lat, 3.0, 1e-12);
        assert_close(line[2].coord.lon, 2.0, 1e-12);
        assert!(from_geojson("not json").is_err());
        assert!(from_geojson(r#"{"type":"Point","coordinates":[2.0,91.0]}"#).is_err());
    }

    #[cfg(feature = "wkt")]
    #[test]
    fn wkt_point_and_linestring_are_x_y() {
        let pt = from_wkt("POINT(2 9)").unwrap();
        assert_eq!(pt.len(), 1);
        assert_close(pt[0].coord.lat, 9.0, 1e-12); // y = latitude
        assert_close(pt[0].coord.lon, 2.0, 1e-12); // x = longitude
        assert_eq!(
            pt[0].source.as_ref().unwrap().axis_order,
            Some(AxisOrder::LonLat)
        );
        let line = from_wkt("LINESTRING(0 0, 10 20, 30 40)").unwrap();
        assert_eq!(line.len(), 3);
        assert_close(line[1].coord.lat, 20.0, 1e-12);
        assert_close(line[1].coord.lon, 10.0, 1e-12);
        assert!(from_wkt("CIRCLE(0 0 5)").is_err());
        assert!(from_wkt("POINT(181 0)").is_err());
    }

    #[cfg(feature = "gpx")]
    #[test]
    fn gpx_waypoint_and_track() {
        let doc = r#"<?xml version="1.0"?>
            <gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
              <wpt lat="48.2" lon="16.3"><ele>183.0</ele></wpt>
              <trk><trkseg>
                <trkpt lat="40.0" lon="-75.0"/>
                <trkpt lat="41.0" lon="-74.0"/>
              </trkseg></trk>
            </gpx>"#;
        let fixes = from_gpx(doc).unwrap();
        assert_eq!(fixes.len(), 3); // 1 waypoint + 2 track points
        assert_close(fixes[0].coord.lat, 48.2, 1e-9);
        assert_close(fixes[0].coord.lon, 16.3, 1e-9);
        assert_eq!(fixes[0].coord.height, Some(Height::Ellipsoidal(183.0)));
        assert_eq!(
            fixes[0].source.as_ref().unwrap().axis_order,
            Some(AxisOrder::LatLon)
        );
        assert_close(fixes[2].coord.lat, 41.0, 1e-9);
        assert!(from_gpx("<gpx>broken").is_err());
    }

    #[cfg(feature = "kml")]
    #[test]
    fn kml_placemark_point() {
        // KML coordinates are lon,lat,alt.
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
            <kml xmlns="http://www.opengis.net/kml/2.2"><Document>
              <Placemark><Point><coordinates>16.3,48.2,183</coordinates></Point></Placemark>
            </Document></kml>"#;
        let fixes = from_kml(doc).unwrap();
        assert_eq!(fixes.len(), 1);
        assert_close(fixes[0].coord.lat, 48.2, 1e-9);
        assert_close(fixes[0].coord.lon, 16.3, 1e-9);
        assert_eq!(fixes[0].coord.height, Some(Height::Ellipsoidal(183.0)));
        assert_eq!(
            fixes[0].source.as_ref().unwrap().axis_order,
            Some(AxisOrder::LonLat)
        );
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
    }

    /// The `(lat, lon)` of every fix, in emission order.
    fn lat_lons(fixes: &[Fix]) -> Vec<(f64, f64)> {
        fixes.iter().map(|f| (f.coord.lat, f.coord.lon)).collect()
    }

    /// A rejection is `Parse` whose message names the format first.
    #[track_caller]
    fn assert_parse_err(r: Result<Vec<Fix>>, prefix: &str) {
        assert!(
            matches!(&r, Err(Error::Parse(m)) if m.starts_with(prefix)),
            "{r:?}"
        );
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
        assert_eq!((fixes[8].coord.lat, fixes[8].coord.lon), (7.0, 7.0));
        // A GeometryCollection recurses, nested collections included.
        let collection = r#"{"type":"GeometryCollection","geometries":[
            {"type":"Point","coordinates":[1,2]},
            {"type":"GeometryCollection","geometries":[
                {"type":"LineString","coordinates":[[3,4],[5,6]]}]}]}"#;
        let fixes = from_geojson(collection).unwrap();
        assert_eq!(lat_lons(&fixes), [(2.0, 1.0), (4.0, 3.0), (6.0, 5.0)]);
        // Every fix records the whole input verbatim, not the fragment.
        assert!(
            fixes
                .iter()
                .all(|f| f.source.as_ref().unwrap().raw == collection)
        );
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
        assert_eq!(from_geojson(feature).unwrap()[0].coord.height, None);
    }

    #[cfg(feature = "geojson")]
    #[test]
    fn geojson_rejections_are_parse_errors() {
        assert_parse_err(from_geojson("not json"), "geojson: ");
        assert_parse_err(
            from_geojson(r#"{"type":"Point","coordinates":[2.0,91.0]}"#),
            "geojson: invalid position",
        );
        // One bad vertex rejects the whole line.
        assert_parse_err(
            from_geojson(r#"{"type":"LineString","coordinates":[[0,0],[181,0]]}"#),
            "geojson: invalid position",
        );
    }

    #[cfg(feature = "wkt")]
    #[test]
    fn wkt_walks_every_geometry_type_in_order() {
        assert_eq!(
            lat_lons(&from_wkt("MULTIPOINT((1 2),(3 4))").unwrap()),
            [(2.0, 1.0), (4.0, 3.0)]
        );
        assert_eq!(
            lat_lons(&from_wkt("MULTILINESTRING((1 2,3 4),(5 6,7 8))").unwrap()),
            [(2.0, 1.0), (4.0, 3.0), (6.0, 5.0), (8.0, 7.0)]
        );
        // Outer ring then holes, closing vertices included.
        let polygon = "POLYGON((0 0,10 0,10 10,0 0),(1 1,2 1,1 1))";
        assert_eq!(from_wkt(polygon).unwrap().len(), 7);
        let multi_polygon = "MULTIPOLYGON(((0 0,1 0,0 0)),((5 5,6 5,5 5),(7 7,8 7,7 7)))";
        let fixes = from_wkt(multi_polygon).unwrap();
        assert_eq!(fixes.len(), 9);
        assert_eq!((fixes[8].coord.lat, fixes[8].coord.lon), (7.0, 7.0));
        let collection = "GEOMETRYCOLLECTION(POINT(1 2),LINESTRING(3 4,5 6))";
        assert_eq!(
            lat_lons(&from_wkt(collection).unwrap()),
            [(2.0, 1.0), (4.0, 3.0), (6.0, 5.0)]
        );
    }

    #[cfg(feature = "wkt")]
    #[test]
    fn wkt_empty_and_dimensioned_geometries() {
        // EMPTY geometries are valid WKT with no positions.
        assert!(from_wkt("POINT EMPTY").unwrap().is_empty());
        assert!(from_wkt("LINESTRING EMPTY").unwrap().is_empty());
        assert!(from_wkt("GEOMETRYCOLLECTION EMPTY").unwrap().is_empty());
        assert!(from_wkt("MULTIPOINT EMPTY").unwrap().is_empty());
        // Z is read as an ellipsoidal height.
        let z = from_wkt("POINT Z(1 2 3)").unwrap();
        assert_eq!(z[0].coord.height, Some(Height::Ellipsoidal(3.0)));
        // M is a measure, not a height: it is dropped.
        let m = from_wkt("POINT M(1 2 3)").unwrap();
        assert_eq!((m[0].coord.lat, m[0].coord.lon), (2.0, 1.0));
        assert_eq!(m[0].coord.height, None);
        let zm = from_wkt("POINT ZM(1 2 3 4)").unwrap();
        assert_eq!(zm[0].coord.height, Some(Height::Ellipsoidal(3.0)));
    }

    #[cfg(feature = "wkt")]
    #[test]
    fn wkt_rejections_are_parse_errors() {
        assert_parse_err(from_wkt("CIRCLE(0 0 5)"), "wkt: ");
        assert_parse_err(from_wkt("POINT(181 0)"), "wkt: invalid position");
        assert_parse_err(from_wkt("POINT(0 -91)"), "wkt: invalid position");
    }

    #[cfg(feature = "gpx")]
    #[test]
    fn gpx_emits_waypoints_then_tracks_then_routes() {
        let doc = r#"<?xml version="1.0"?>
            <gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
              <rte>
                <rtept lat="10.0" lon="20.0"/>
                <rtept lat="11.0" lon="21.0"><ele>5.5</ele></rtept>
              </rte>
              <trk><trkseg><trkpt lat="1.0" lon="2.0"/></trkseg>
                   <trkseg><trkpt lat="3.0" lon="4.0"/></trkseg></trk>
              <wpt lat="-1.0" lon="-2.0"/>
            </gpx>"#;
        let fixes = from_gpx(doc).unwrap();
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
        assert_eq!(fixes[4].coord.height, Some(Height::Ellipsoidal(5.5)));
        assert_eq!(fixes[3].coord.height, None);
        assert!(fixes.iter().all(|f| f.source.as_ref().unwrap().raw == doc));
    }

    #[cfg(feature = "gpx")]
    #[test]
    fn gpx_rejections_are_parse_errors() {
        assert_parse_err(from_gpx("<gpx>broken"), "gpx: ");
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
        assert_parse_err(from_kml("<kml></Point></kml>"), "kml: ");
        assert_parse_err(
            from_kml("<kml><Point><coordinates>abc</coordinates></Point></kml>"),
            "kml: ",
        );
        assert_parse_err(from_kml("not xml"), "kml: ");
        let off_globe = r#"<kml xmlns="http://www.opengis.net/kml/2.2">
            <Placemark><Point><coordinates>0,95</coordinates></Point></Placemark></kml>"#;
        assert_parse_err(from_kml(off_globe), "kml: invalid position");
    }
}
