//! Structured interchange formats: GeoJSON, WKT, GPX, KML.
//!
//! **Axis order:** GeoJSON positions and KML `coordinates` are lon-lat (X,Y),
//! GPX names its axes with `lat`/`lon` attributes, and WKT defines no order at
//! all, so [`from_wkt`] takes the caller's [`AxisOrder`]. These parsers
//! normalize every position to the library's lat/lon model and record on each
//! [`Fix`]'s [`RawSource::axis_order`] the order the source listed the axes in
//! (`None` for GPX, whose axes are labelled) — they never silently transpose.
//!
//! **Altitude:** GeoJSON (RFC 7946) altitude is height above the WGS-84
//! ellipsoid, so it becomes [`Height::Ellipsoidal`]. KML 2.2 altitude is height
//! above sea level and GPX `<ele>` is mean-sea-level elevation in practice, so
//! both become [`Height::Orthometric`]. WKT does not say what Z measures; it is
//! read as [`Height::Ellipsoidal`] and the fix carries a note saying so.
//!
//! Every position becomes one [`Fix`] (so a `LineString` of *n* vertices yields
//! *n* fixes), tagged with confidence `1.0` because the structured input is
//! unambiguous. Its [`RawSource::raw`] holds that position alone, re-rendered
//! in the source format's syntax — never the whole document. A document with no
//! positions (a null-geometry GeoJSON feature, `POINT EMPTY`, an empty GPX file)
//! yields `Ok(vec![])`. Each format is behind its own cargo feature.
//!
//! **Errors:** malformed input is an [`Error::Parse`] of kind `Syntax`, or
//! `TooDeep` past the nesting limits below. One invalid position rejects the
//! whole document: [`Error::OutOfRange`] with its values (GPX's reader rejects
//! one itself, as `Syntax`), or, for a non-finite height, a bad GPX `<hdop>`,
//! or a GeoJSON position of one number, an error naming its zero-based index.
//!
//! **Untrusted input:** every parser returns or errors on any input; none
//! recurses without bound. GeoJSON nesting is capped at 128 levels by
//! `serde_json`; WKT parentheses nested deeper than 128 levels and KML
//! elements nested deeper than 32 are rejected before they are parsed, as is
//! KML with an unclosed element or construct (the `kml` parser never returns
//! on a truncated document); and the walks over the parsed geometry are
//! iterative.

use super::{hdop_accuracy, syntax};
use crate::coord::{Coordinate, Crs, Height};
use crate::error::{Error, Result};
use crate::fix::{AxisOrder, Confidence, Fix, RawSource};

#[cfg(feature = "kml")]
mod xml_scan;
#[cfg(feature = "kml")]
pub(super) use xml_scan::check_kml_markup;

/// One position read from a document, not yet validated.
struct Position {
    lon: f64,
    lat: f64,
    height: Option<Height>,
    /// The order the source listed the axes in; `None` when they are labelled.
    axis_order: Option<AxisOrder>,
    /// The position alone, in the source format's syntax.
    raw: String,
    timestamp: Option<std::time::SystemTime>,
    /// GPX horizontal dilution of precision, scaled into an accuracy radius.
    hdop: Option<f64>,
    notes: Vec<String>,
}

impl Position {
    fn new(
        lon: f64,
        lat: f64,
        height: Option<Height>,
        axis_order: Option<AxisOrder>,
        raw: String,
    ) -> Self {
        Self {
            lon,
            lat,
            height,
            axis_order,
            raw,
            timestamp: None,
            hdop: None,
            notes: Vec::new(),
        }
    }

    /// Validate the position and build its [`Fix`].
    fn into_fix(self) -> Result<Fix> {
        let mut coord = Coordinate::from_lon_lat(self.lon, self.lat, Crs::Wgs84)?;
        if let Some(height) = self.height {
            coord = coord.with_height(height)?;
        }
        let mut fix = Fix::from_coord(coord);
        fix.accuracy = self.hdop.map(hdop_accuracy).transpose()?;
        fix.timestamp = self.timestamp;
        let mut source = RawSource::new(self.raw, Confidence::CERTAIN);
        source.axis_order = self.axis_order;
        source.notes = self.notes;
        fix.source = Some(source);
        Ok(fix)
    }
}

/// Validate every position in document order, failing on the first invalid
/// one with its own error, its `detail` (if any) naming the format and index.
fn into_fixes(format: &str, positions: Vec<Position>) -> Result<Vec<Fix>> {
    positions
        .into_iter()
        .enumerate()
        .map(|(index, position)| {
            let at = |detail| format!("{format}: invalid position at index {index}: {detail}");
            position.into_fix().map_err(|e| match e {
                Error::InvalidValue { field, detail } => Error::InvalidValue {
                    field,
                    detail: at(detail),
                },
                Error::Parse { kind, detail } => Error::parse(kind, at(detail)),
                other => other,
            })
        })
        .collect()
}

/// The deepest parenthesis nesting a WKT string may have: 128, the recursion
/// limit `serde_json` applies to GeoJSON. The `wkt` parser recurses once per
/// level with no limit of its own, so deeper text is rejected before it
/// reaches it rather than allowed to overflow the stack.
#[cfg(feature = "wkt")]
const MAX_WKT_DEPTH: usize = 128;

/// The error for a document nested deeper than `limit` levels.
#[cfg(any(feature = "geojson", feature = "wkt", feature = "kml"))]
fn too_deep(format: &str, limit: usize) -> Error {
    Error::parse(
        crate::error::ParseErrorKind::TooDeep,
        format!("{format}: nesting deeper than {limit} levels"),
    )
}

/// Reject WKT whose parentheses nest deeper than [`MAX_WKT_DEPTH`].
/// WKT has no string literals or comments, so every `(` opens a level.
#[cfg(feature = "wkt")]
fn check_wkt_depth(input: &str) -> Result<()> {
    let mut depth = 0usize;
    for b in input.bytes() {
        match b {
            b'(' => {
                depth += 1;
                if depth > MAX_WKT_DEPTH {
                    return Err(too_deep("wkt", MAX_WKT_DEPTH));
                }
            }
            b')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

/// Parse every position in a GeoJSON document (lon-lat order, RFC 7946).
///
/// Walks the whole structure — features, geometry collections, and every
/// vertex of lines and polygons — emitting one [`Fix`] per position, in
/// document order. A feature with a `null` geometry contributes no positions,
/// so a document of only such features yields `Ok(vec![])`. Altitude is
/// [`Height::Ellipsoidal`]; each fix's raw source is its position as a GeoJSON
/// `Point` geometry, which this function parses back to the same fix.
///
/// # Errors
/// [`Error::Parse`] on malformed GeoJSON (`TooDeep` past 128 levels), or the
/// first invalid position's error, as the [module docs](self) describe.
///
/// ```
/// use geocoordinates::{Error, Height, ParseErrorKind};
/// use geocoordinates::fix::AxisOrder;
/// use geocoordinates::parse::interchange::from_geojson;
///
/// let fixes = from_geojson(r#"{"type":"Point","coordinates":[-74.006,40.7128,10.0]}"#)?;
/// let fix = &fixes[0];
/// assert_eq!((fix.coord.lat(), fix.coord.lon()), (40.7128, -74.006));
/// assert_eq!(fix.coord.height(), Some(Height::Ellipsoidal(10.0)));
/// let axis_order = fix.source.as_ref().and_then(|s| s.axis_order);
/// assert_eq!(axis_order, Some(AxisOrder::LonLat));
///
/// // Latitude 200 is out of range, and the error carries it.
/// let line = r#"{"type":"LineString","coordinates":[[0,0],[0,200]]}"#;
/// assert_eq!(from_geojson(line), Err(Error::OutOfRange { lat: 200.0, lon: 0.0 }));
///
/// // A position needs two numbers: the error names the position's index.
/// let short = r#"{"type":"LineString","coordinates":[[0,0],[5]]}"#;
/// let Err(Error::Parse { kind, detail }) = from_geojson(short) else {
///     panic!("expected a parse error");
/// };
/// assert_eq!(kind, ParseErrorKind::Syntax);
/// assert!(detail.contains("index 1"));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[cfg(feature = "geojson")]
#[cfg_attr(docsrs, doc(cfg(feature = "geojson")))]
pub fn from_geojson(input: &str) -> Result<Vec<Fix>> {
    use geojson::{GeoJson, Geometry, GeometryValue, Position as GeoJsonPosition};

    /// Push one position, rejecting one with fewer than two numbers (the
    /// `geojson` crate length-checks only a `Point`'s) at its document index.
    fn position(p: &GeoJsonPosition, out: &mut Vec<Position>) -> Result<()> {
        if p.len() < 2 {
            return Err(syntax(format!(
                "geojson: invalid position at index {}: \
                 a position needs at least 2 numbers, got {}",
                out.len(),
                p.len()
            )));
        }
        let alt = (p.len() >= 3).then(|| Height::Ellipsoidal(p[2]));
        let raw = Geometry::new_point(p.clone()).to_string();
        out.push(Position::new(p[0], p[1], alt, Some(AxisOrder::LonLat), raw));
        Ok(())
    }
    /// Walk `root` in document order with an explicit stack, not recursion.
    fn geometry(root: &Geometry, out: &mut Vec<Position>) -> Result<()> {
        let mut stack = vec![root];
        while let Some(g) = stack.pop() {
            match &g.value {
                GeometryValue::Point { coordinates } => position(coordinates, out)?,
                GeometryValue::MultiPoint { coordinates }
                | GeometryValue::LineString { coordinates } => {
                    coordinates.iter().try_for_each(|p| position(p, out))?;
                }
                GeometryValue::MultiLineString { coordinates }
                | GeometryValue::Polygon { coordinates } => {
                    coordinates
                        .iter()
                        .flatten()
                        .try_for_each(|p| position(p, out))?;
                }
                GeometryValue::MultiPolygon { coordinates } => {
                    coordinates
                        .iter()
                        .flatten()
                        .flatten()
                        .try_for_each(|p| position(p, out))?;
                }
                GeometryValue::GeometryCollection { geometries } => {
                    stack.extend(geometries.iter().rev());
                }
            }
        }
        Ok(())
    }

    // `serde_json` stops at 128 levels and says so only in its message.
    let parsed: GeoJson = input.parse().map_err(|e: geojson::Error| match &e {
        geojson::Error::MalformedGeoJson(json)
            if json.to_string().starts_with("recursion limit exceeded") =>
        {
            too_deep("geojson", 128)
        }
        _ => syntax(format!("geojson: {e}")),
    })?;
    let mut out = Vec::new();
    let walked = match &parsed {
        GeoJson::Geometry(g) => geometry(g, &mut out),
        GeoJson::Feature(f) => f.geometry.iter().try_for_each(|g| geometry(g, &mut out)),
        GeoJson::FeatureCollection(fc) => fc
            .features
            .iter()
            .filter_map(|f| f.geometry.as_ref())
            .try_for_each(|g| geometry(g, &mut out)),
    };
    // A short position stops the walk; an out-of-range position before it
    // still comes first in document order, so validate those first.
    let fixes = into_fixes("geojson", out)?;
    walked.map(|()| fixes)
}

/// Parse every position in a WKT string, reading its X and Y as `axis_order`.
///
/// WKT carries no CRS and so no axis order: OGC CRS84 and most GIS software
/// write longitude first ([`AxisOrder::LonLat`]), while EPSG:4326 taken
/// strictly is latitude first ([`AxisOrder::LatLon`]). The caller states which
/// one the text uses; there is no default. Each fix records `axis_order` as the
/// order its source listed the axes in.
///
/// One [`Fix`] per position, in document order; `EMPTY` geometries yield
/// `Ok(vec![])`. Z is read as [`Height::Ellipsoidal`], with a note on the fix
/// that WKT leaves the vertical reference unspecified; M is a measure, not a
/// height, and is dropped. Each fix's raw source is its position as a WKT
/// `POINT`, which this function parses back to the same fix.
///
/// # Errors
/// [`Error::Parse`] on malformed WKT (`TooDeep` past 128 levels of
/// parentheses), or the first invalid position's error, as the
/// [module docs](self) describe.
///
/// ```
/// use geocoordinates::Error;
/// use geocoordinates::fix::AxisOrder;
/// use geocoordinates::parse::interchange::from_wkt;
///
/// let line = "LINESTRING (-122.4194 37.7749, 2.3522 48.8566)";
/// let fixes = from_wkt(line, AxisOrder::LonLat)?;
/// assert_eq!(fixes.len(), 2);
/// assert_eq!((fixes[1].coord.lat(), fixes[1].coord.lon()), (48.8566, 2.3522));
///
/// // Read lat-first, the same text puts -122.4194 on the latitude axis.
/// let transposed = Error::OutOfRange { lat: -122.4194, lon: 37.7749 };
/// assert_eq!(from_wkt(line, AxisOrder::LatLon), Err(transposed));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[cfg(feature = "wkt")]
#[cfg_attr(docsrs, doc(cfg(feature = "wkt")))]
pub fn from_wkt(input: &str, axis_order: AxisOrder) -> Result<Vec<Fix>> {
    use core::str::FromStr;

    use wkt::Wkt;
    use wkt::types::{Coord, Point};

    fn coord(c: &Coord<f64>, axis_order: AxisOrder, out: &mut Vec<Position>) {
        let (lon, lat) = match axis_order {
            AxisOrder::LonLat => (c.x, c.y),
            AxisOrder::LatLon => (c.y, c.x),
        };
        let raw = Point::from_coord(*c).to_string();
        let mut position = Position::new(
            lon,
            lat,
            c.z.map(Height::Ellipsoidal),
            Some(axis_order),
            raw,
        );
        if c.z.is_some() {
            position.notes.push(
                "WKT does not specify the vertical reference; Z is read as ellipsoidal height"
                    .to_string(),
            );
        }
        out.push(position);
    }
    fn coords(cs: &[Coord<f64>], axis_order: AxisOrder, out: &mut Vec<Position>) {
        cs.iter().for_each(|c| coord(c, axis_order, out));
    }
    /// Walk `root` in document order with an explicit stack, not recursion.
    fn geometry(root: &Wkt<f64>, axis_order: AxisOrder, out: &mut Vec<Position>) {
        let mut stack = vec![root];
        while let Some(g) = stack.pop() {
            match g {
                Wkt::Point(p) => {
                    if let Some(c) = p.coord() {
                        coord(c, axis_order, out);
                    }
                }
                Wkt::LineString(ls) => coords(ls.coords(), axis_order, out),
                Wkt::Polygon(poly) => poly
                    .rings()
                    .iter()
                    .for_each(|ls| coords(ls.coords(), axis_order, out)),
                Wkt::MultiPoint(mp) => mp.points().iter().for_each(|p| {
                    if let Some(c) = p.coord() {
                        coord(c, axis_order, out);
                    }
                }),
                Wkt::MultiLineString(mls) => mls
                    .line_strings()
                    .iter()
                    .for_each(|ls| coords(ls.coords(), axis_order, out)),
                Wkt::MultiPolygon(mp) => mp.polygons().iter().for_each(|poly| {
                    poly.rings()
                        .iter()
                        .for_each(|ls| coords(ls.coords(), axis_order, out));
                }),
                Wkt::GeometryCollection(gc) => stack.extend(gc.geometries().iter().rev()),
            }
        }
    }

    check_wkt_depth(input)?;
    let parsed = Wkt::<f64>::from_str(input).map_err(|e| syntax(format!("wkt: {e}")))?;
    let mut out = Vec::new();
    geometry(&parsed, axis_order, &mut out);
    into_fixes("wkt", out)
}

/// Parse every waypoint, then track point, then route point in a GPX document.
///
/// GPX names its axes with `lat`/`lon` attributes, so each fix's
/// [`RawSource::axis_order`] is `None`. `<ele>` is mean-sea-level elevation in
/// practice and becomes [`Height::Orthometric`]; `<time>` becomes
/// [`Fix::timestamp`]; `<hdop>` becomes a horizontal accuracy of `HDOP × 5 m`
/// (a nominal GPS user-equivalent range error, the scaling the NMEA parser
/// applies to `GGA`), tagged [`AccuracyKind::OneSigma`](crate::fix::AccuracyKind::OneSigma). A document
/// with no points yields `Ok(vec![])`. Each fix's raw source is its point
/// element (`<wpt>`, `<trkpt>`, or `<rtept>`) with its `<ele>`.
///
/// # Errors
/// [`Error::Parse`] of kind `Syntax` on malformed GPX or a `lat` / `lon` out
/// of range (the `gpx` reader checks both itself), and for a bad `<hdop>`,
/// naming the first such point's index (see the [module docs](self)).
///
/// ```
/// use geocoordinates::{Error, Height, ParseErrorKind};
/// use geocoordinates::parse::interchange::from_gpx;
///
/// let gpx = r#"<?xml version="1.0"?>
/// <gpx version="1.1" creator="example" xmlns="http://www.topografix.com/GPX/1/1">
///   <wpt lat="40.7128" lon="-74.006"><ele>10</ele><hdop>2</hdop></wpt>
/// </gpx>"#;
/// let fix = &from_gpx(gpx)?[0];
/// assert_eq!((fix.coord.lat(), fix.coord.lon()), (40.7128, -74.006));
/// assert_eq!(fix.coord.height(), Some(Height::Orthometric(10.0)));
/// let horizontal = fix.accuracy.and_then(|a| a.horizontal()).map(|h| h.meters());
/// assert_eq!(horizontal, Some(10.0)); // HDOP 2 × 5 m
///
/// let truncated = from_gpx("<gpx>");
/// assert!(matches!(truncated, Err(Error::Parse { kind: ParseErrorKind::Syntax, .. })));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[cfg(feature = "gpx")]
#[cfg_attr(docsrs, doc(cfg(feature = "gpx")))]
pub fn from_gpx(input: &str) -> Result<Vec<Fix>> {
    fn push(tag: &str, wp: &gpx::Waypoint, out: &mut Vec<Position>) {
        let p = wp.point(); // geo-types Point: x = lon, y = lat
        let (lon, lat) = (p.x(), p.y());
        let raw = match wp.elevation {
            Some(ele) => format!(r#"<{tag} lat="{lat}" lon="{lon}"><ele>{ele}</ele></{tag}>"#),
            None => format!(r#"<{tag} lat="{lat}" lon="{lon}"/>"#),
        };
        let mut position =
            Position::new(lon, lat, wp.elevation.map(Height::Orthometric), None, raw);
        position.timestamp = wp
            .time
            .map(|t| std::time::SystemTime::from(time::OffsetDateTime::from(t)));
        position.hdop = wp.hdop;
        out.push(position);
    }

    let data = gpx::read(input.as_bytes()).map_err(|e| syntax(format!("gpx: {e}")))?;
    let mut out = Vec::new();
    for wp in &data.waypoints {
        push("wpt", wp, &mut out);
    }
    for track in &data.tracks {
        for segment in &track.segments {
            for wp in &segment.points {
                push("trkpt", wp, &mut out);
            }
        }
    }
    for route in &data.routes {
        for wp in &route.points {
            push("rtept", wp, &mut out);
        }
    }
    into_fixes("gpx", out)
}

/// Parse every placemark and geometry position in a KML document (lon,lat,alt).
///
/// Each fix records [`AxisOrder::LonLat`], the order KML `coordinates` list
/// their axes in. KML 2.2 altitude is height above sea level, so it becomes
/// [`Height::Orthometric`]; the `altitudeMode` that says whether to honour it
/// is not read. A document with no positions yields `Ok(vec![])`. Each fix's
/// raw source is its position as a KML `Point`, which this function parses
/// back to the same fix.
///
/// # Errors
/// [`Error::Parse`] on malformed KML: `TooDeep` past 32 levels of elements,
/// `Syntax` for anything else, including an element left unclosed or an end
/// tag that does not close the innermost open element. Otherwise the first
/// invalid position's error, as the [module docs](self) describe.
///
/// ```
/// use geocoordinates::{Error, Height, ParseErrorKind};
/// use geocoordinates::parse::interchange::from_kml;
///
/// let kml = r#"<Placemark><name>NYC</name>
///   <Point><coordinates>-74.006,40.7128,10</coordinates></Point>
/// </Placemark>"#;
/// let fix = &from_kml(kml)?[0];
/// assert_eq!((fix.coord.lat(), fix.coord.lon()), (40.7128, -74.006));
/// assert_eq!(fix.coord.height(), Some(Height::Orthometric(10.0)));
///
/// let off_the_map = "<Point><coordinates>-74.006,95</coordinates></Point>";
/// assert_eq!(from_kml(off_the_map), Err(Error::OutOfRange { lat: 95.0, lon: -74.006 }));
///
/// // A truncated document is rejected rather than read to its end.
/// let Err(Error::Parse { kind, detail }) = from_kml("<kml><Point>") else {
///     panic!("expected a parse error");
/// };
/// assert_eq!(kind, ParseErrorKind::Syntax);
/// assert_eq!(detail, "kml: element <Point> is not closed");
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[cfg(feature = "kml")]
#[cfg_attr(docsrs, doc(cfg(feature = "kml")))]
pub fn from_kml(input: &str) -> Result<Vec<Fix>> {
    use core::str::FromStr;

    use kml::Kml;
    use kml::types::{Coord, Geometry};

    fn coord(c: &Coord<f64>, out: &mut Vec<Position>) {
        let raw = match c.z {
            Some(z) => format!(
                "<Point><coordinates>{},{},{z}</coordinates></Point>",
                c.x, c.y
            ),
            None => format!("<Point><coordinates>{},{}</coordinates></Point>", c.x, c.y),
        };
        out.push(Position::new(
            c.x,
            c.y,
            c.z.map(Height::Orthometric),
            Some(AxisOrder::LonLat),
            raw,
        ));
    }
    /// A node still to visit: a document element or a geometry within one.
    enum Node<'a> {
        Kml(&'a Kml<f64>),
        Geometry(&'a Geometry<f64>),
    }
    fn polygon(poly: &kml::types::Polygon<f64>, out: &mut Vec<Position>) {
        poly.outer.coords.iter().for_each(|c| coord(c, out));
        poly.inner
            .iter()
            .for_each(|r| r.coords.iter().for_each(|c| coord(c, out)));
    }
    /// Walk `root` in document order with an explicit stack, not recursion.
    fn walk(root: &Kml<f64>, out: &mut Vec<Position>) {
        fn kmls(elements: &[Kml<f64>]) -> impl DoubleEndedIterator<Item = Node<'_>> {
            elements.iter().map(Node::Kml)
        }
        fn geometries(gs: &[Geometry<f64>]) -> impl DoubleEndedIterator<Item = Node<'_>> {
            gs.iter().map(Node::Geometry)
        }
        let mut stack = vec![Node::Kml(root)];
        while let Some(node) = stack.pop() {
            match node {
                Node::Geometry(g) => match g {
                    Geometry::Point(p) => coord(&p.coord, out),
                    Geometry::LineString(ls) => ls.coords.iter().for_each(|c| coord(c, out)),
                    Geometry::LinearRing(lr) => lr.coords.iter().for_each(|c| coord(c, out)),
                    Geometry::Polygon(poly) => polygon(poly, out),
                    Geometry::MultiGeometry(mg) => stack.extend(geometries(&mg.geometries).rev()),
                    _ => {} // Element / future variants carry no coordinates
                },
                Node::Kml(k) => match k {
                    Kml::Point(p) => coord(&p.coord, out),
                    Kml::LineString(ls) => ls.coords.iter().for_each(|c| coord(c, out)),
                    Kml::LinearRing(lr) => lr.coords.iter().for_each(|c| coord(c, out)),
                    Kml::Polygon(poly) => polygon(poly, out),
                    Kml::MultiGeometry(mg) => stack.extend(geometries(&mg.geometries).rev()),
                    Kml::Placemark(pm) => stack.extend(pm.geometry.as_ref().map(Node::Geometry)),
                    Kml::Document { elements, .. } => stack.extend(kmls(elements).rev()),
                    Kml::Folder(f) => stack.extend(kmls(&f.elements).rev()),
                    Kml::KmlDocument(doc) => stack.extend(kmls(&doc.elements).rev()),
                    _ => {}
                },
            }
        }
    }

    check_kml_markup(input)?;
    let parsed = Kml::<f64>::from_str(input).map_err(|e| syntax(format!("kml: {e}")))?;
    let mut out = Vec::new();
    walk(&parsed, &mut out);
    into_fixes("kml", out)
}
