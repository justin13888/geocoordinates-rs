//! The KML markup pre-scan, which must count exactly the elements the `kml`
//! parser opens and reject every document that leaves one open.

use super::*;

/// Whether the scan stays within the depth limit: `false` only for `TooDeep`,
/// so a closure error after the depth is settled still counts as within it.
fn within_depth(input: &str) -> bool {
    !matches!(
        check_kml_markup(input),
        Err(Error::Parse {
            kind: ParseErrorKind::TooDeep,
            ..
        })
    )
}

#[test]
fn xml_depth_scan_counts_only_open_elements() {
    let ok = |inner: &str| within_depth(&nested("<a>", inner, "</a>", 32));
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
    // `quick-xml` 0.37 balances only `<` and `>` inside a `<!DOCTYPE …>`,
    // ignoring quotes and `[`, so a quoted `>` counts: the declaration ends
    // after `'>'>` and the `<b>` that follows is a start tag the parser reads.
    assert!(!ok("<!DOCTYPE d [<!ENTITY e '>'><b>]>"));
    // The shortest closed forms. The `?` opening `<?` never ends it, so
    // `<?>` is unterminated rather than closed.
    assert!(ok("<??></a><b>"));
    assert!(ok("<!----></a><b>"));
    assert!(ok("<!DOCTYPE d></a><b>"));
    // `kml` ignores `</kml>`, so it closes no level, whatever its prefix:
    // under 30 levels, `<kml></kml>` leaves 31 open and two more start tags
    // are one too many, where any other closed element leaves room for both.
    let ok30 = |inner: &str| within_depth(&nested("<a>", inner, "</a>", 30));
    assert!(ok30("<kml></kml><b>"));
    assert!(!ok30("<kml></kml><b><c>"));
    assert!(!ok30("<k:kml></k:kml ><b><c>"));
    assert!(ok30("<kmlx></kmlx><b><c>"));
    assert!(ok30("<x:kmlx></x:kmlx><b><c>"));
    // End tags close levels, so siblings never add up.
    let siblings = nested("<a>", &"<b></b>".repeat(100), "</a>", 31);
    assert!(check_kml_markup(&siblings).is_ok());
}

/// The scan's verdict on `input`: `Ok`, or the parse error's detail.
fn verdict(input: &str) -> std::result::Result<(), String> {
    check_kml_markup(input).map_err(|e| match e {
        Error::Parse {
            kind: ParseErrorKind::Syntax,
            detail,
        } => detail,
        other => panic!("{input:?}: expected a syntax error, got {other:?}"),
    })
}

#[test]
fn xml_closure_scan_rejects_every_unclosed_document() {
    // Elements left open, innermost named: the `kml` parser would never
    // return on these (#135).
    assert_eq!(
        verdict("<kml><Point>"),
        Err("kml: element <Point> is not closed".into())
    );
    assert_eq!(
        verdict("<kml><Placemark>"),
        Err("kml: element <Placemark> is not closed".into())
    );
    assert_eq!(
        verdict(r#"<Data name="n"><value>1</value>"#),
        Err(r#"kml: element <Data> is not closed"#.into())
    );
    // A start tag's name ends at whitespace; the prefix is part of it.
    assert_eq!(
        verdict("<k:Point\n id='p'>"),
        Err("kml: element <k:Point> is not closed".into())
    );
    // An unterminated construct names where it starts, at any depth.
    for tail in [
        "<b",
        "<!-- <b>",
        "<![CDATA[ <b>",
        "<?pi",
        "<?>",
        "<?> <b> ?>",
        "</b",
        "<!DOCTYPE <",
    ] {
        let detail = Err("kml: markup at byte 6 is not terminated".to_string());
        assert_eq!(verdict(&format!("<a><a>{tail}")), detail, "{tail}");
        let detail = Err("kml: markup at byte 0 is not terminated".to_string());
        assert_eq!(verdict(tail), detail, "{tail}");
    }
    // A `<!` construct `quick-xml` errors on is rejected where it starts,
    // closed or not: only `<!--`, `<![CDATA[`, and `<!DOCTYPE` (any case)
    // open a construct it reads.
    for bang in [
        "<!x>",
        "<!x",
        "<!ELEMENT x>",
        "<!-x>",
        "<!-",
        "<![x>",
        "<![CDATA",
        "<!Dx>",
        "<!DOCTYP>",
        "<!>",
        "<!",
    ] {
        let detail = "kml: markup at byte 3 is not a comment, CDATA section, or DOCTYPE";
        assert_eq!(
            verdict(&format!("<a>{bang}</a>")),
            Err(detail.into()),
            "{bang}"
        );
    }
    // A DOCTYPE needs a name before its `>`.
    for nameless in ["<!DOCTYPE>", "<!doctype \n>"] {
        let detail = "kml: DOCTYPE at byte 0 has no name";
        assert_eq!(verdict(nameless), Err(detail.into()), "{nameless}");
    }
    assert_eq!(verdict("<!doctype d><a></a>"), Ok(()));
    // An end tag must close the innermost open element, by its exact name.
    assert_eq!(
        verdict("<kml></Point></kml>"),
        Err("kml: end tag </Point> does not close <kml>".into())
    );
    assert_eq!(
        verdict("<k:kml></kml>"),
        Err("kml: end tag </kml> does not close <k:kml>".into())
    );
    // Everything up to the `>` that ends the end tag is its name, and a `>`
    // inside quotes does not end it.
    assert_eq!(
        verdict(r#"<a></a x="></a>">"#),
        Err(r#"kml: end tag </a x="></a>"> does not close <a>"#.into())
    );
    assert_eq!(
        verdict("</kml>"),
        Err("kml: end tag </kml> closes nothing".into())
    );
    // Closed documents pass, whatever they hold between elements.
    for closed in [
        "",
        "not xml",
        "<kml/>",
        "<k:kml></k:kml >",
        "<kml>\n<Point id='p'><coordinates>1,2</coordinates></Point>\n</kml>",
        "<?xml version='1.0'?><!-- <a> --><kml><![CDATA[<a>]]></kml><kml></kml>",
        r#"<!DOCTYPE d [<!ENTITY e "x">]><kml><Point x="</kml>"/></kml>"#,
    ] {
        assert_eq!(verdict(closed), Ok(()), "{closed}");
    }
}
