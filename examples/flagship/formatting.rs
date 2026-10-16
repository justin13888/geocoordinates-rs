//! Coordinate formatting and the strict, geo-URI, and free-text parsers.

use geocoordinates::fix::{Accuracy, AccuracyKind, AxisOrder, Fix};
use geocoordinates::format::{
    DecimalSeparator, FormatOptions, HemisphereStyle, Representation, SymbolStyle, format,
    format_fix,
};
use geocoordinates::grids::MgrsPrecision;
use geocoordinates::parse::{
    GridKind, ParseOptions, from_geo_uri, parse_coordinate, parse_coordinate_with, parse_text,
    parse_text_with,
};
use geocoordinates::{Coordinate, Crs, Error, Height, Length};

use crate::support::{DemoResult, assert_close, assert_within_meters, new_york};

pub(crate) fn formatting_and_parsing() -> DemoResult {
    let new_york = new_york();
    assert_eq!(
        format(&new_york, &FormatOptions::default())?,
        "40.712800, -74.006000"
    );
    // `Coordinate` has no `Display`/`FromStr`: text carries no datum, so the
    // explicit `format` / `parse_coordinate` routes are the only text paths.
    let parsed = parse_coordinate("40.7128, -74.006")?.coord;
    assert_within_meters(&parsed, &new_york, 0.01);
    // A GCJ-02 coordinate renders to the same digits, but the datum does not
    // survive the trip: parsed text is always WGS-84 until the caller re-tags it.
    let mars = Coordinate::gcj02(39.915, 116.404)?;
    let mars_text = format(&mars, &FormatOptions::default())?;
    assert_eq!(mars_text, "39.915000, 116.404000");
    let reparsed_mars = parse_coordinate(&mars_text)?.coord;
    assert_eq!(reparsed_mars.crs(), Crs::Wgs84);
    assert_ne!(reparsed_mars.crs(), mars.crs());
    let retagged = Coordinate::new(reparsed_mars.lat(), reparsed_mars.lon(), Crs::Gcj02)?;
    assert_within_meters(&retagged, &mars, 0.2);

    // Each representation carries only the settings that apply to it, and
    // every grid with a string form renders through the same `format`.
    let representations = [
        (
            Representation::DecimalDegrees {
                decimals: Some(4),
                hemisphere: HemisphereStyle::Signed,
            },
            "40.7128, -74.0060",
        ),
        (
            Representation::Dms {
                second_decimals: Some(1),
                symbols: SymbolStyle::Unicode,
                hemisphere: HemisphereStyle::Cardinal,
            },
            "40°42′46.1″N 74°00′21.6″W",
        ),
        (
            Representation::Ddm {
                minute_decimals: Some(3),
                symbols: SymbolStyle::Unicode,
                hemisphere: HemisphereStyle::Cardinal,
            },
            "40°42.768′N 74°00.360′W",
        ),
        (Representation::PlusCode { length: 10 }, "87G7PX7V+4J"),
        (Representation::Geohash { length: 7 }, "dr5regw"),
        (Representation::Maidenhead { pairs: 3 }, "FN20xr"),
        (
            Representation::Mgrs {
                precision: MgrsPrecision::M10,
            },
            "18TWL83950735",
        ),
    ];
    for (representation, expected) in representations {
        let rendered = format(
            &new_york,
            &FormatOptions {
                representation,
                ..FormatOptions::default()
            },
        )?;
        assert_eq!(rendered, expected);
    }

    let symbols = [
        (SymbolStyle::Unicode, "10°30′00″N 20°15′00″E"),
        (SymbolStyle::Ascii, "10°30'00\"N 20°15'00\"E"),
        (SymbolStyle::Letters, "10d30m00sN 20d15m00sE"),
    ];
    for (symbols, expected) in symbols {
        let rendered = format(
            &Coordinate::wgs84(10.5, 20.25)?,
            &FormatOptions {
                representation: Representation::Dms {
                    second_decimals: Some(0),
                    symbols,
                    hemisphere: HemisphereStyle::Cardinal,
                },
                decimal_separator: DecimalSeparator::Point,
            },
        )?;
        assert_eq!(rendered, expected);
    }
    assert_eq!(
        format(
            &new_york,
            &FormatOptions {
                representation: Representation::DecimalDegrees {
                    decimals: None,
                    hemisphere: HemisphereStyle::Cardinal,
                },
                ..FormatOptions::default()
            },
        )?,
        "40.712800 N, 74.006000 W"
    );
    // The locale heuristic picks the separator; the options carry only it.
    assert_eq!(
        DecimalSeparator::for_locale("de-DE"),
        DecimalSeparator::Comma
    );
    assert_eq!(
        DecimalSeparator::for_locale("de-CH"),
        DecimalSeparator::Point
    );
    assert_eq!(
        format(
            &new_york,
            &FormatOptions {
                decimal_separator: DecimalSeparator::for_locale("de-DE"),
                ..FormatOptions::default()
            },
        )?,
        "40,712800 -74,006000"
    );
    // The default options leave the decimal count unset, so `format_fix`
    // derives it from the fix's accuracy.
    let mut low_accuracy = Fix::from_coord(new_york);
    low_accuracy.accuracy = Some(Accuracy::new(
        AccuracyKind::OneSigma,
        Some(Length::from_meters(1_000.0)),
        None,
    )?);
    assert_eq!(
        format_fix(&low_accuracy, &FormatOptions::default())?,
        "40.71, -74.01"
    );
    // A value that rounds to zero renders unsigned, so it re-parses to the
    // same rendering.
    let near_equator = Coordinate::wgs84(-1e-9, 10.0)?;
    let cardinal = FormatOptions {
        representation: Representation::DecimalDegrees {
            decimals: Some(2),
            hemisphere: HemisphereStyle::Cardinal,
        },
        ..FormatOptions::default()
    };
    let rendered = format(&near_equator, &cardinal)?;
    assert_eq!(rendered, "0.00 N, 10.00 E");
    assert_eq!(
        format(&parse_coordinate(&rendered)?.coord, &cardinal)?,
        rendered
    );

    let dms = parse_text("40°42′46″N 74°00′22″W")?;
    assert_close(dms.coord.lat(), 40.0 + 42.0 / 60.0 + 46.0 / 3_600.0, 1e-9);
    assert_close(dms.coord.lon(), -(74.0 + 22.0 / 3_600.0), 1e-9);
    assert_eq!(dms.source.as_ref().and_then(|s| s.axis_order), None);
    let ddm = parse_text("40 42.766 N 74 0.456 W")?;
    assert_close(ddm.coord.lat(), 40.0 + 42.766 / 60.0, 1e-9);
    assert_close(ddm.coord.lon(), -(74.0 + 0.456 / 60.0), 1e-9);

    let lon_lat = parse_text_with(
        "-74.006, 40.7128",
        &ParseOptions {
            default_axis_order: AxisOrder::LonLat,
            ..ParseOptions::default()
        },
    )?;
    assert_eq!(
        lon_lat.source.as_ref().and_then(|s| s.axis_order),
        Some(AxisOrder::LonLat)
    );
    assert_within_meters(&lon_lat.coord, &new_york, 0.01);
    let comma_options = ParseOptions {
        decimal_separator: DecimalSeparator::Comma,
        ..ParseOptions::default()
    };
    let decimal_comma = parse_text_with("40,7128 -74,006", &comma_options)?;
    assert_within_meters(&decimal_comma.coord, &new_york, 0.01);
    // With a decimal comma, a remaining `,` (or `;`) separates the pair.
    let listed = parse_text_with("40,7128, -74,006", &comma_options)?;
    // Free text cannot honour an expected grid, so it refuses the option.
    let expecting_grid = ParseOptions {
        expect_grid: Some(GridKind::Maidenhead),
        ..ParseOptions::default()
    };
    assert!(matches!(
        parse_text_with("40.7128, -74.006", &expecting_grid),
        Err(Error::InvalidValue {
            field: "expect_grid",
            ..
        })
    ));
    assert_within_meters(&listed.coord, &new_york, 0.01);
    // Input with no single reading is rejected, not guessed at: bare DDMM
    // without a hemisphere letter, a fraction before the last field, and
    // exponent notation.
    assert!(parse_text("1234.5, 10").is_err());
    assert!(parse_text("40.5 30, 10").is_err());
    assert!(parse_text("4e1 3e1").is_err());

    let uri = from_geo_uri("geo:48.2,16.3,183;crs=wgs84;u=40")?;
    assert_eq!(uri.coord.height(), Some(Height::Ellipsoidal(183.0)));
    assert_eq!(
        uri.accuracy.and_then(|accuracy| accuracy.horizontal()),
        Some(Length::from_meters(40.0))
    );
    let plus = parse_coordinate("8FVC2222+22")?;
    assert_eq!(plus.source.as_ref().and_then(|s| s.axis_order), None);
    assert!(
        (5.0..15.0).contains(
            &plus
                .accuracy
                .and_then(|accuracy| accuracy.horizontal())
                .expect("Plus Code reports its cell bound")
                .meters()
        )
    );

    // MGRS is self-delimiting, so it is always detected; a geohash or
    // Maidenhead locator is not (`42` and `bed` are valid geohashes), so it
    // decodes only when the options expect it.
    let mgrs = parse_coordinate("18TWK0000027757")?;
    assert_eq!(mgrs.source.as_ref().and_then(|s| s.axis_order), None);
    assert!(parse_coordinate("ezs42").is_err());
    assert!(parse_coordinate("42").is_err());
    let geohash = parse_coordinate_with(
        "ezs42",
        &ParseOptions {
            expect_grid: Some(GridKind::Geohash),
            ..ParseOptions::default()
        },
    )?;
    assert_close(geohash.coord.lat(), 42.6, 0.03);

    let rendered = format(&new_york, &FormatOptions::default())?;
    let reparsed = parse_coordinate(&rendered)?;
    assert_within_meters(&reparsed.coord, &new_york, 0.2);

    // Letter-style DMS re-parses through `parse_coordinate`; every other
    // rendering re-parses given the format options' own `parse_options`.
    let letters = format(
        &new_york,
        &FormatOptions {
            representation: Representation::Dms {
                second_decimals: None,
                symbols: SymbolStyle::Letters,
                hemisphere: HemisphereStyle::Cardinal,
            },
            ..FormatOptions::default()
        },
    )?;
    assert_within_meters(&parse_coordinate(&letters)?.coord, &new_york, 0.5);
    let german = FormatOptions {
        representation: Representation::Ddm {
            minute_decimals: None,
            symbols: SymbolStyle::Unicode,
            hemisphere: HemisphereStyle::Signed,
        },
        decimal_separator: DecimalSeparator::for_locale("de-DE"),
    };
    let rendered = format(&new_york, &german)?;
    assert!(parse_coordinate(&rendered).is_err());
    let reparsed = parse_coordinate_with(&rendered, &german.parse_options())?;
    assert_within_meters(&reparsed.coord, &new_york, 1.0);
    let maidenhead = FormatOptions {
        representation: Representation::Maidenhead { pairs: 3 },
        ..FormatOptions::default()
    };
    let locator = format(&new_york, &maidenhead)?;
    let cell = parse_coordinate_with(&locator, &maidenhead.parse_options())?;
    let bound = cell
        .accuracy
        .and_then(|accuracy| accuracy.horizontal())
        .expect("a grid cell reports its bound")
        .meters();
    assert_within_meters(&cell.coord, &new_york, bound);

    Ok("formatting and parsing")
}
