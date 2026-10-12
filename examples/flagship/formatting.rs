//! Coordinate formatting and the strict, geo-URI, and free-text parsers.

use geocoordinates::fix::AxisOrder;
use geocoordinates::format::{
    FormatOptions, HemisphereStyle, Representation, SymbolStyle, format, format_fix,
};
use geocoordinates::parse::text::{TextParseOptions, parse as parse_text, parse_with};
use geocoordinates::parse::{from_geo_uri, parse_coordinate, parse_coordinate_with};
use geocoordinates::{Accuracy, Coordinate, Fix, Height};

use crate::support::{DemoResult, NEW_YORK, assert_close, assert_within_meters};

pub(crate) fn formatting_and_parsing() -> DemoResult {
    assert_eq!(
        format(&NEW_YORK, &FormatOptions::default())?,
        "40.712800, -74.006000"
    );
    assert_eq!(NEW_YORK.to_string(), "40.712800, -74.006000");
    let from_str: Coordinate = "40.7128, -74.006".parse()?;
    assert_within_meters(&from_str, &NEW_YORK, 0.01);

    for representation in [
        Representation::DecimalDegrees,
        Representation::Dms,
        Representation::Ddm,
        Representation::PlusCode,
    ] {
        let rendered = format(
            &NEW_YORK,
            &FormatOptions {
                representation,
                ..FormatOptions::default()
            },
        )?;
        assert!(!rendered.is_empty());
    }

    let symbols = [
        (SymbolStyle::Unicode, "10°30′00″N 20°15′00″E"),
        (SymbolStyle::Ascii, "10°30'00\"N 20°15'00\"E"),
        (SymbolStyle::Letters, "10d30m00sN 20d15m00sE"),
    ];
    for (symbol_style, expected) in symbols {
        let rendered = format(
            &Coordinate::wgs84(10.5, 20.25),
            &FormatOptions {
                representation: Representation::Dms,
                precision: Some(0),
                symbol_style,
                hemisphere_style: HemisphereStyle::Cardinal,
                locale: None,
            },
        )?;
        assert_eq!(rendered, expected);
    }
    assert_eq!(
        format(
            &NEW_YORK,
            &FormatOptions {
                hemisphere_style: HemisphereStyle::Cardinal,
                ..FormatOptions::default()
            },
        )?,
        "40.712800 N, 74.006000 W"
    );
    assert_eq!(
        format(
            &NEW_YORK,
            &FormatOptions {
                locale: Some("de-DE".to_owned()),
                ..FormatOptions::default()
            },
        )?,
        "40,712800 -74,006000"
    );
    let low_accuracy = Fix {
        coord: NEW_YORK,
        accuracy: Some(Accuracy {
            horizontal_m: Some(1_000.0),
            vertical_m: None,
        }),
        timestamp: None,
        source: None,
    };
    assert_eq!(
        format_fix(
            &low_accuracy,
            &FormatOptions {
                precision: None,
                ..FormatOptions::default()
            }
        )?,
        "40.71, -74.01"
    );

    let dms = parse_text("40°42′46″N 74°00′22″W")?;
    assert_close(dms.coord.lat, 40.0 + 42.0 / 60.0 + 46.0 / 3_600.0, 1e-9);
    assert_close(dms.coord.lon, -(74.0 + 22.0 / 3_600.0), 1e-9);
    assert_eq!(dms.source.as_ref().and_then(|s| s.axis_order), None);
    let ddm = parse_text("40 42.766 N 74 0.456 W")?;
    assert_close(ddm.coord.lat, 40.0 + 42.766 / 60.0, 1e-9);
    assert_close(ddm.coord.lon, -(74.0 + 0.456 / 60.0), 1e-9);

    let lon_lat = parse_with(
        "-74.006, 40.7128",
        &TextParseOptions {
            default_axis_order: AxisOrder::LonLat,
            decimal_comma: false,
        },
    )?;
    assert_eq!(
        lon_lat.source.as_ref().and_then(|s| s.axis_order),
        Some(AxisOrder::LonLat)
    );
    assert_within_meters(&lon_lat.coord, &NEW_YORK, 0.01);
    let decimal_comma = parse_with(
        "40,7128 -74,006",
        &TextParseOptions {
            default_axis_order: AxisOrder::LatLon,
            decimal_comma: true,
        },
    )?;
    assert_within_meters(&decimal_comma.coord, &NEW_YORK, 0.01);

    let uri = from_geo_uri("geo:48.2,16.3,183;crs=wgs84;u=40")?;
    assert_eq!(uri.coord.height, Some(Height::Ellipsoidal(183.0)));
    assert_eq!(
        uri.accuracy.and_then(|accuracy| accuracy.horizontal_m),
        Some(40.0)
    );
    let plus = parse_coordinate("8FVC2222+22")?;
    assert_eq!(plus.source.as_ref().and_then(|s| s.axis_order), None);
    assert!(
        (5.0..15.0).contains(
            &plus
                .accuracy
                .and_then(|accuracy| accuracy.horizontal_m)
                .expect("Plus Code reports its cell bound")
        )
    );

    let rendered = format(&NEW_YORK, &FormatOptions::default())?;
    let reparsed = parse_coordinate(&rendered)?;
    assert_within_meters(&reparsed.coord, &NEW_YORK, 0.2);

    // Letter-style DMS re-parses through `parse_coordinate`; a decimal-comma
    // locale re-parses once `decimal_comma` follows the format options.
    let letters = format(
        &NEW_YORK,
        &FormatOptions {
            representation: Representation::Dms,
            symbol_style: SymbolStyle::Letters,
            hemisphere_style: HemisphereStyle::Cardinal,
            ..FormatOptions::default()
        },
    )?;
    assert_within_meters(&parse_coordinate(&letters)?.coord, &NEW_YORK, 0.5);
    let german = FormatOptions {
        representation: Representation::Ddm,
        locale: Some("de-DE".to_owned()),
        ..FormatOptions::default()
    };
    assert!(german.uses_decimal_comma());
    let rendered = format(&NEW_YORK, &german)?;
    assert!(parse_coordinate(&rendered).is_err());
    let reparsed = parse_coordinate_with(
        &rendered,
        &TextParseOptions {
            decimal_comma: german.uses_decimal_comma(),
            ..TextParseOptions::default()
        },
    )?;
    assert_within_meters(&reparsed.coord, &NEW_YORK, 1.0);

    Ok("formatting and parsing")
}
