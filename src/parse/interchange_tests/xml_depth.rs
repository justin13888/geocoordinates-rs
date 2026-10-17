//! The KML depth guard's XML scan, which must count exactly the elements the
//! `kml` parser opens.

use super::*;

#[test]
fn xml_depth_scan_counts_only_open_elements() {
    let ok = |inner: &str| check_xml_depth(&nested("<a>", inner, "</a>", 32)).is_ok();
    // 32 open elements is the limit; one more start tag is too deep.
    assert!(ok(""));
    assert!(!ok("<b>"));
    // Constructs that open no element, each hiding a start tag that would
    // be counted if the scan misread it.
    assert!(ok("<b/>"));
    assert!(ok(r#"<b x="1>2"/>"#));
    assert!(ok("<b x='1>2'/>"));
    // A closed quote ends the attribute value, so the `>` after it ends the
    // start tag.
    assert!(!ok(r#"<b x="1>2">"#));
    assert!(ok("<!-- > <b> -->"));
    assert!(ok("<![CDATA[ > <b> ]]>"));
    assert!(ok("<?pi x?>"));
    assert!(ok("<!DOCTYPE d>"));
    // Each construct ends where `quick-xml` ends it, so an end tag hidden
    // inside one closes no level and the `<b>` after it is one too many.
    assert!(!ok("<?xml ></a>?><b>"));
    assert!(!ok("<!--></a>--><b>"));
    assert!(!ok("<!---></a>--><b>"));
    assert!(!ok(r#"<!DOCTYPE d [<!ENTITY e "x"></a>]><b>"#));
    assert!(!ok(r#"</a x="></a>"><b><c>"#));
    // `quick-xml` 0.37 balances only `<` and `>` inside a `<!DOCTYPE …>`,
    // ignoring quotes and `[`, so a quoted `>` counts: the declaration ends
    // after `'>'>` and the `<b>` that follows is a start tag the parser reads.
    assert!(!ok("<!DOCTYPE d [<!ENTITY e '>'><b>]>"));
    // The shortest closed forms: the `?` opening `<?>` also ends it.
    assert!(ok("<?></a><b>"));
    assert!(ok("<!----></a><b>"));
    assert!(ok("<!DOCTYPE d></a><b>"));
    // `kml` ignores `</kml>`, so it closes no level, whatever its prefix:
    // under 30 levels, `<kml></kml>` leaves 31 open and two more start tags
    // are one too many, where any other closed element leaves room for both.
    let ok30 = |inner: &str| check_xml_depth(&nested("<a>", inner, "</a>", 30)).is_ok();
    assert!(ok30("<kml></kml><b>"));
    assert!(!ok30("<kml></kml><b><c>"));
    assert!(!ok30("<k:kml></k:kml ><b><c>"));
    assert!(ok30("<kmlx></kmlx><b><c>"));
    assert!(ok30("<x:kmlx></x:kmlx><b><c>"));
    // End tags close levels, so siblings never add up.
    let siblings = nested("<a>", &"<b></b>".repeat(100), "</a>", 31);
    assert!(check_xml_depth(&siblings).is_ok());
    // An unterminated construct ends the scan; the parser rejects it.
    for tail in ["<b", "<!-- <b>", "<![CDATA[ <b>", "<?pi", "<!x", "</b"] {
        let truncated = format!("{}{tail}", "<a>".repeat(32));
        assert!(check_xml_depth(&truncated).is_ok(), "{tail}");
    }
}
