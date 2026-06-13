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
    Ok(out)
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
    Ok(out)
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
    Ok(out)
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
    Ok(out)
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
        // A FeatureCollection with a 3-vertex LineString yields three fixes.
        let fc = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","geometry":{"type":"LineString",
             "coordinates":[[0.0,0.0],[1.0,1.0],[2.0,3.0]]},"properties":null}]}"#;
        let line = from_geojson(fc).unwrap();
        assert_eq!(line.len(), 3);
        assert_close(line[2].coord.lat, 3.0, 1e-12);
        assert_close(line[2].coord.lon, 2.0, 1e-12);
        assert!(from_geojson("not json").is_err());
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
}
