//! The KML reader: nesting bounds, truncated documents, placemarks, and every
//! geometry arm.

use super::*;

#[test]
fn kml_nesting_is_bounded_at_32_levels() {
    let point = "<Point><coordinates>1,2</coordinates></Point>";
    // <kml> + 29 folders + <Point> + <coordinates> is 32 element levels,
    // which the `kml` parser handles on a test thread's stack.
    let deepest = format!("<kml>{}</kml>", nested("<Folder>", point, "</Folder>", 29));
    let fixes = parse_kml(&deepest).unwrap();
    assert_eq!(fixes.len(), 1);
    assert_eq!((fixes[0].coord.lat(), fixes[0].coord.lon()), (2.0, 1.0));
    let too_deep = format!("<kml>{}</kml>", nested("<Folder>", point, "</Folder>", 30));
    assert_too_deep!(parse_kml(&too_deep), "kml: nesting deeper than 32 levels");
    // An end tag inside an `<?xml …?>` declaration, which `kml` skips even
    // inside elements, closes no level: 40 such folders are 40 levels.
    let hidden = nested("<Folder><?xml ></Folder>?>", point, "</Folder>", 40);
    assert_too_deep!(
        parse_kml(&format!("<kml>{hidden}</kml>")),
        "kml: nesting deeper than 32 levels"
    );
    let commented = format!("<kml><!-- <Folder> -->{point}</kml>");
    assert_eq!(parse_kml(&commented).unwrap().len(), 1);
    // End tags close levels: many shallow siblings are fine.
    let wide = format!("<kml><Folder>{}</Folder></kml>", point.repeat(200));
    assert_eq!(parse_kml(&wide).unwrap().len(), 200);
}

#[test]
fn kml_truncated_documents_are_rejected_instead_of_spinning() {
    // The `kml` parser loops forever on these (#135), so each runs on its own
    // thread: a regression fails the test instead of hanging the suite.
    let point = "<Point><coordinates>1,2</coordinates></Point>";
    let truncated = [
        "<kml><Point>".to_string(),
        "<kml><Placemark>".to_string(),
        "<LineString><coordinates>0,0 1,1</coordinates>".to_string(),
        r#"<kml><Data name="n"><displayName><!-- unterminated"#.to_string(),
        "<kml><SchemaData><SimpleData name='n'>".to_string(),
        format!("<kml><Document><Placemark>{point}"),
    ];
    // Closed, but holding a construct `quick-xml` errors on, which the
    // `Data` / `SchemaData` readers swallow before spinning on `Eof`.
    let bangs = "<!x>|<!ELEMENT x>|<!-x>|<![x>|<!Dx>|<!DOCTYPE >|<?>|<?>?>".split('|');
    let data =
        |b: &str| format!(r#"<kml><Data name="n"><displayName>{b}</displayName></Data></kml>"#);
    for input in truncated.into_iter().chain(bangs.map(data)) {
        let (tx, rx) = std::sync::mpsc::channel();
        let doc = input.clone();
        std::thread::spawn(move || tx.send(parse_kml(&doc)));
        let r = rx.recv_timeout(std::time::Duration::from_secs(5));
        let r = r.unwrap_or_else(|_| panic!("parse_kml({input:?}) did not return"));
        assert_parse_err!(r, "kml: ");
    }
    // The innermost unclosed element is named.
    let r = parse_kml(&format!("<kml><Document><Placemark>{point}"));
    assert_parse_err!(r, "kml: element <Placemark> is not closed");
    // The same document, closed, is read.
    let closed = format!("<kml><Document><Placemark>{point}</Placemark></Document></kml>");
    assert_eq!(parse_kml(&closed).unwrap().len(), 1);
}

#[test]
fn kml_100k_deep_nesting_errors_instead_of_overflowing() {
    let start = std::time::Instant::now();
    let closed = nested("<Folder>", "", "</Folder>", 100_000);
    assert_too_deep!(parse_kml(&closed), "kml: nesting");
    let geometry = nested("<MultiGeometry>", "", "</MultiGeometry>", 100_000);
    assert_too_deep!(parse_kml(&geometry), "kml: nesting");
    // Unclosed and too deep: the depth limit is reached first.
    assert_too_deep!(parse_kml(&"<Folder>".repeat(100_000)), "kml: nesting");
    // End tags hidden inside constructs the parser skips close no level: an
    // `<?xml …?>` declaration (ignored by `kml` even inside elements) ends
    // at `?>`, and a `<!DOCTYPE …>` at its balancing `>`.
    for hidden in [
        "<?xml ></Folder>?>",
        "<!DOCTYPE d [<!ENTITY e 'x'></Folder>]>",
    ] {
        let level = format!("<Folder>{hidden}");
        let input = nested(&level, "", "</Folder>", 100_000);
        assert_too_deep!(parse_kml(&input), "kml: nesting");
    }
    // `kml` ignores `</kml>`, so each repeat nests its reader one level
    // deeper even though the XML is flat.
    for repeats in [40, 100_000] {
        let flat = "<kml></kml>".repeat(repeats);
        assert_too_deep!(parse_kml(&flat), "kml: nesting");
    }
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn kml_placemark_point() {
    // KML coordinates are lon,lat,alt; altitude is above sea level.
    let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
        <kml xmlns="http://www.opengis.net/kml/2.2"><Document>
          <Placemark><Point><coordinates>16.3,48.2,183</coordinates></Point></Placemark>
          <Placemark><Point><coordinates>1.5,2</coordinates></Point></Placemark>
        </Document></kml>"#;
    let fixes = parse_kml(doc).unwrap();
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
    assert_fragments_reparse!(&fixes, parse_kml);
}

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
    let fixes = parse_kml(doc).unwrap();
    // 1 + 2 + 4 + 4 + 1 (bare) + 2 + 4 + 1 + 4 (placemark) = 23 positions;
    // every geometry/walk arm contributes, so deleting any one is caught.
    assert_eq!(fixes.len(), 23);
    assert_fragments_reparse!(&fixes, parse_kml);
}

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
    assert_eq!(parse_kml(&doc).unwrap().len(), 12);
    // Mismatched tags and a non-numeric coordinate are parse errors.
    assert_parse_err!(parse_kml("<kml></Point></kml>"), "kml: end tag </Point>");
    assert_parse_err!(
        parse_kml("<kml><Point><coordinates>abc</coordinates></Point></kml>"),
        "kml: ",
    );
    assert_parse_err!(parse_kml("not xml"), "kml: ");
    let off_globe = r#"<kml xmlns="http://www.opengis.net/kml/2.2">
        <Placemark><LineString><coordinates>0,0 0,95</coordinates></LineString></Placemark></kml>"#;
    let r = parse_kml(off_globe);
    assert_eq!(r, out_of_range(95.0, 0.0));
    // A non-finite altitude keeps its typed error, and names the index.
    let r = parse_kml("<LineString><coordinates>0,0,0 1,1,NaN</coordinates></LineString>");
    let ok = matches!(&r, Err(Error::InvalidValue { field: "height", detail })
        if detail.starts_with("kml: invalid position at index 1: "));
    assert!(ok, "{r:?}");
}
