//! The KML markup pre-scan's `Point` check: the `kml` 0.13 parser panics on a
//! `Point` its geometry reader finds no position for, so every such document
//! must be rejected before it reaches the parser.

use super::*;

#[test]
fn kml_point_without_coordinates_is_rejected_instead_of_panicking() {
    // Each would panic inside `kml` (#205) without the pre-scan.
    let rejected = [
        "<Point></Point>",
        "<kml><Placemark><Point></Point></Placemark></kml>",
        "<kml:Point></kml:Point>",
        "<Point><altitudeMode>absolute</altitudeMode></Point>",
        "<Point><coordinates></coordinates></Point>",
        "<Point><coordinates> \n\t</coordinates></Point>",
        // Non-breaking spaces, literal and as a reference, split as
        // whitespace in the `kml` reader.
        "<Point><coordinates>\u{a0}</coordinates></Point>",
        "<Point><coordinates>&#160;</coordinates></Point>",
        "<Point><coordinates>&#xA0; &#32;&#x2003;</coordinates></Point>",
        // A named entity other than the five predefined ones unescapes to
        // whitespace where any crate enables `quick-xml`'s `escape-html`.
        "<Point><coordinates>&nbsp;</coordinates></Point>",
        "<Point><coordinates>&emsp; &ensp;&thinsp;</coordinates></Point>",
        "<Point><coordinates>&foo;</coordinates></Point>",
        "<Point><coordinates><![CDATA[ ]]></coordinates></Point>",
        // Only the last `<coordinates>` counts.
        "<Point><coordinates>1,2</coordinates><coordinates></coordinates></Point>",
        "<MultiGeometry><Point><coordinates>1,2</coordinates></Point><Point></Point></MultiGeometry>",
        // A nested `Point`'s end tag ends the outer one's reader before the
        // outer coordinates.
        "<Placemark><Point><Point></Point><coordinates>1,2</coordinates></Point></Placemark>",
        // Any other event after `<coordinates>` is no position (here `kml`
        // would return an error rather than panic).
        "<Point><coordinates><!-- c -->1,2</coordinates></Point>",
    ];
    for input in rejected {
        assert_parse_err!(from_kml(input), "kml: <Point> at byte ");
    }
    // A `;` outside a reference is not whitespace: the pre-scan passes it,
    // and the `kml` parser rejects it as a coordinate.
    // So do references that unescape to something other than whitespace,
    // and malformed ones, which `kml` keeps raw, `&` included, as does text
    // with a non-whitespace character after an unknown entity.
    let reaches_kml = [
        ";",
        "&lt;",
        "&gt;",
        "&amp;",
        "&apos;",
        "&quot;",
        "&nbsp;1",
        "&#0;",
        "&#+32;",
        "&#-32;",
        "&#xD800;",
        "&#;",
        "&#32",
        "&#32&#32;",
    ];
    for coordinates in reaches_kml {
        let r = from_kml(&format!(
            "<Point><coordinates>{coordinates}</coordinates></Point>"
        ));
        assert!(
            matches!(&r, Err(Error::Parse { detail, .. }) if !detail.starts_with("kml: <Point>")),
            "{coordinates}: {r:?}"
        );
        assert_parse_err!(r, "kml: ");
    }
    let r = from_kml("<kml><Placemark><Point></Point></Placemark></kml>");
    assert_parse_err!(r, "kml: <Point> at byte 16 has no coordinates");
    // The nested `Point` is named: the outer one's coordinates do not count
    // for the reader the inner end tag stops.
    let r = from_kml("<Point><coordinates>1,2</coordinates><Point></Point></Point>");
    assert_parse_err!(r, "kml: <Point> at byte 37 has no coordinates");
}

#[test]
fn kml_point_with_coordinates_is_read() {
    let one = |input: &str| {
        let fixes = from_kml(input).unwrap();
        assert_eq!(fixes.len(), 1, "{input}");
        (fixes[0].coord.lat(), fixes[0].coord.lon())
    };
    assert_eq!(
        one("<Point><coordinates>1,2</coordinates></Point>"),
        (2.0, 1.0)
    );
    assert_eq!(
        one("<kml:Point><kml:coordinates>1,2</kml:coordinates></kml:Point>"),
        (2.0, 1.0)
    );
    assert_eq!(
        one("<Point><coordinates><![CDATA[ 1,2 ]]></coordinates></Point>"),
        (2.0, 1.0)
    );
    // References are unescaped, so coordinates written only as references,
    // decimal or hex, hold a position.
    for coordinates in [
        "&#49;,2",
        "&#49;&#44;&#50;",
        "&#x31;&#x2C;&#x32;",
        " &#32;1,&#50; ",
    ] {
        assert_eq!(
            one(&format!(
                "<Point><coordinates>{coordinates}</coordinates></Point>"
            )),
            (2.0, 1.0),
            "{coordinates}"
        );
    }
    // A self-closing `<coordinates/>` is ignored, as `kml` ignores it.
    assert_eq!(
        one("<Point><coordinates>1,2</coordinates><coordinates/></Point>"),
        (2.0, 1.0)
    );
    // An empty `<coordinates>` followed by a full one: the last counts.
    assert_eq!(
        one("<Point><coordinates></coordinates><coordinates>1,2</coordinates></Point>"),
        (2.0, 1.0)
    );
    // The inner `Point`'s coordinates are the outer reader's too.
    assert_eq!(
        one("<Placemark><Point><Point><coordinates>1,2</coordinates></Point></Point></Placemark>"),
        (2.0, 1.0)
    );
    // A self-closing `<Point/>` builds no point, so it needs no coordinates.
    assert_eq!(from_kml("<Placemark><Point/></Placemark>").unwrap(), vec![]);
    // A `LineString` with no coordinates was never a panic, and stays empty.
    assert_eq!(from_kml("<LineString></LineString>").unwrap(), vec![]);
}
