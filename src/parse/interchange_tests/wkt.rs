//! The WKT reader: nesting bounds, the stated axis order, geometry walks, and
//! dimensioned geometries.

use super::*;

#[test]
fn wkt_nesting_is_bounded_at_128_levels() {
    // 127 collections around a point is 128 levels of parentheses: accepted,
    // and walked without recursion.
    let deepest = nested("GEOMETRYCOLLECTION(", "POINT(1 2)", ")", 127);
    let fixes = parse_wkt(&deepest, AxisOrder::LonLat).unwrap();
    assert_eq!(lat_lons(&fixes), [(2.0, 1.0)]);
    // One more level is rejected before the parser sees it.
    let too_deep = nested("GEOMETRYCOLLECTION(", "POINT(1 2)", ")", 128);
    assert_too_deep!(
        parse_wkt(&too_deep, AxisOrder::LonLat),
        "wkt: nesting deeper than 128 levels",
    );
    // Closing parentheses close levels: many shallow siblings are fine.
    let wide = format!("GEOMETRYCOLLECTION({})", vec!["POINT(0 0)"; 200].join(","));
    assert_eq!(parse_wkt(&wide, AxisOrder::LonLat).unwrap().len(), 200);
}

#[test]
fn wkt_100k_deep_nesting_errors_instead_of_overflowing() {
    let start = std::time::Instant::now();
    let unclosed = "GEOMETRYCOLLECTION(".repeat(100_000);
    assert_too_deep!(parse_wkt(&unclosed, AxisOrder::LonLat), "wkt: nesting");
    let closed = nested("GEOMETRYCOLLECTION(", "POINT(1 2)", ")", 100_000);
    assert_too_deep!(parse_wkt(&closed, AxisOrder::LonLat), "wkt: nesting");
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn wkt_reads_x_y_in_the_stated_axis_order() {
    let lon_lat = parse_wkt("POINT(2 9)", AxisOrder::LonLat).unwrap();
    assert_eq!(lat_lons(&lon_lat), [(9.0, 2.0)]);
    assert_eq!(source(&lon_lat[0]).axis_order, Some(AxisOrder::LonLat));
    assert_eq!(source(&lon_lat[0]).raw, "POINT(2 9)");
    // The same text read lat-first (EPSG:4326 strict) transposes.
    let lat_lon = parse_wkt("POINT(2 9)", AxisOrder::LatLon).unwrap();
    assert_eq!(lat_lons(&lat_lon), [(2.0, 9.0)]);
    assert_eq!(source(&lat_lon[0]).axis_order, Some(AxisOrder::LatLon));
    // A latitude-range check follows the stated order: 120 is a valid
    // longitude but not a valid latitude.
    assert!(parse_wkt("POINT(120 10)", AxisOrder::LonLat).is_ok());
    let r = parse_wkt("POINT(120 10)", AxisOrder::LatLon);
    assert_eq!(r, out_of_range(120.0, 10.0));
    let line = parse_wkt("LINESTRING(0 0, 10 20, 30 40)", AxisOrder::LonLat).unwrap();
    assert_eq!(line.len(), 3);
    assert_close(line[1].coord.lat(), 20.0, 1e-12);
    assert_close(line[1].coord.lon(), 10.0, 1e-12);
    for order in [AxisOrder::LonLat, AxisOrder::LatLon] {
        let fixes = parse_wkt("LINESTRING(1 2, 3 4)", order).unwrap();
        assert_eq!(source(&fixes[1]).raw, "POINT(3 4)");
        assert_fragments_reparse!(&fixes, |raw| parse_wkt(raw, order));
    }
}

#[test]
fn wkt_walks_every_geometry_type_in_order() {
    let xy = |input| parse_wkt(input, AxisOrder::LonLat).unwrap();
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
    assert_fragments_reparse!(&fixes, |raw| parse_wkt(raw, AxisOrder::LonLat));
}

#[test]
fn wkt_empty_and_dimensioned_geometries() {
    let xy = |input| parse_wkt(input, AxisOrder::LonLat).unwrap();
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
        assert_fragments_reparse!(&fixes, |raw| parse_wkt(raw, AxisOrder::LonLat));
    }
}

#[test]
fn wkt_rejections_are_parse_errors() {
    assert_parse_err!(parse_wkt("CIRCLE(0 0 5)", AxisOrder::LonLat), "wkt: ");
    let point = parse_wkt("POINT(181 0)", AxisOrder::LonLat);
    assert_eq!(point, out_of_range(0.0, 181.0));
    let line = parse_wkt("LINESTRING(0 0, 0 -91)", AxisOrder::LonLat);
    assert_eq!(line, out_of_range(-91.0, 0.0));
}
