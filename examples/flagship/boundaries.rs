//! Antimeridian, pole, and range boundaries, and the typed errors they raise.

use geocoordinates::angle::{Axis, Ddm, Dms, Hemisphere as AngleHemisphere, wrap_longitude};
use geocoordinates::china::{BaiduMercator, Bd09, Gcj02, Wgs84};
use geocoordinates::dgg::{H3Cell, S2CellId};
use geocoordinates::format::{FormatOptions, HemisphereStyle, Representation, format};
use geocoordinates::geodesy::{
    DatumTransform, Ecef, Ellipsoid, Enu, Helmert, destination, geodesic_distance,
    rhumb_destination,
};
use geocoordinates::grids::{
    Geohash, Maidenhead, Mgrs, PlusCode, Ups, Utm, UtmHemisphere, utm_central_meridian_deg,
    utm_zone_for,
};
use geocoordinates::parse::sensors::from_nmea_sentence;
use geocoordinates::parse::{from_geo_uri, parse_coordinate, parse_text};
use geocoordinates::{Coordinate, Crs, Error, Height, Length, ParseErrorKind};

use crate::support::{DemoResult, new_york};

pub(crate) fn boundaries_and_typed_errors() -> DemoResult {
    // Valid by construction: the closed range edges are accepted...
    let north_pole_east = Coordinate::wgs84(90.0, 180.0)?;
    let south_pole_west = Coordinate::wgs84(-90.0, -180.0)?;
    assert_eq!(wrap_longitude(180.0), -180.0);
    assert_eq!(wrap_longitude(-181.0), 179.0);

    let antimeridian_distance = geodesic_distance(
        &Coordinate::wgs84(0.0, 179.9)?,
        &Coordinate::wgs84(0.0, -179.9)?,
    )?;
    assert!(antimeridian_distance.meters() < 25_000.0);
    let reflected = rhumb_destination(
        &Coordinate::wgs84(80.0, 0.0)?,
        0.0,
        Length::from_meters(1_668_000.0),
    )?;
    assert!((80.0..90.0).contains(&reflected.lat()));
    assert_eq!(reflected.lon(), -180.0);
    // Only a meridian crosses a pole: any other rhumb line spirals into it
    // without arriving, so a distance that would reach the pole is rejected.
    assert!(matches!(
        rhumb_destination(
            &Coordinate::wgs84(80.0, 0.0)?,
            45.0,
            Length::from_meters(2_000_000.0),
        ),
        Err(Error::InvalidValue {
            field: "distance",
            ..
        })
    ));

    for polar in [north_pole_east, south_pole_west] {
        let plus = PlusCode::encode(polar, 10)?.decode().into_inner();
        assert!(plus.lat().is_finite() && plus.lon().is_finite());
        let h3 = H3Cell::encode(polar, 9)?.decode().into_inner();
        assert!(h3.lat().is_finite() && h3.lon().is_finite());
        let s2 = S2CellId::encode(polar, 20)?.decode().into_inner();
        assert!(s2.lat().is_finite() && s2.lon().is_finite());
    }

    // ...and anything outside them never becomes a `Coordinate` at all, so no
    // downstream routine has to re-validate. `Error` is `PartialEq`, so the
    // typed rejection can be asserted exactly.
    assert_eq!(
        Coordinate::wgs84(91.0, 0.0),
        Err(Error::OutOfRange {
            lat: 91.0,
            lon: 0.0
        })
    );
    assert!(matches!(
        Coordinate::from_lon_lat(f64::NAN, 0.0, Crs::Wgs84),
        Err(Error::OutOfRange { .. })
    ));
    assert!(matches!(
        Coordinate::wgs84(0.0, 0.0)?.with_height(Height::Ellipsoidal(f64::INFINITY)),
        Err(Error::InvalidValue { .. })
    ));
    // Deserialization goes through the same check: JSON cannot smuggle in an
    // out-of-range coordinate.
    let valid_json = serde_json::to_string(&Coordinate::wgs84(41.0, 0.0)?)?;
    assert!(serde_json::from_str::<Coordinate>(&valid_json).is_ok());
    let out_of_range_json = valid_json.replace("41.0", "91.0");
    assert_ne!(out_of_range_json, valid_json);
    assert!(serde_json::from_str::<Coordinate>(&out_of_range_json).is_err());
    // Angle encodings are valid by construction too: every constructor and
    // deserialization rejects a component outside its range.
    assert!(matches!(
        Dms::from_degrees(f64::NAN, Axis::Latitude),
        Err(Error::InvalidValue {
            field: "decimal degrees",
            ..
        })
    ));
    assert!(matches!(
        Ddm::from_degrees(90.5, Axis::Latitude),
        Err(Error::InvalidValue {
            field: "decimal degrees",
            ..
        })
    ));
    assert!(matches!(
        Dms::new(40, 60, 0.0, AngleHemisphere::North),
        Err(Error::InvalidValue {
            field: "minutes",
            ..
        })
    ));
    assert!(matches!(
        Ddm::new(181, 0.0, AngleHemisphere::East),
        Err(Error::InvalidValue {
            field: "degrees",
            ..
        })
    ));
    let dms_json = serde_json::to_string(&Dms::new(40, 59, 0.0, AngleHemisphere::North)?)?;
    assert!(serde_json::from_str::<Dms>(&dms_json).is_ok());
    let bad_dms_json = dms_json.replace("59", "60");
    assert_ne!(bad_dms_json, dms_json);
    assert!(serde_json::from_str::<Dms>(&bad_dms_json).is_err());
    assert!(
        format(
            &new_york(),
            &FormatOptions {
                representation: Representation::DecimalDegrees {
                    decimals: Some(16),
                    hemisphere: HemisphereStyle::Signed,
                },
                ..FormatOptions::default()
            }
        )
        .is_err()
    );
    assert_eq!(
        BaiduMercator::try_from_coordinate(Coordinate::wgs84(39.915, 116.404)?),
        Err(Error::CrsMismatch {
            expected: Crs::Bd09,
            found: Crs::Wgs84,
        })
    );
    assert!(matches!(
        Wgs84::try_from(Coordinate::gcj02(39.915, 116.404)?),
        Err(Error::CrsMismatch { .. })
    ));
    assert!(matches!(
        Wgs84::new(f64::NAN, 116.404),
        Err(Error::OutOfRange { .. })
    ));
    assert!(matches!(
        Gcj02::new(90.0, 0.0)?.try_to_bd09(),
        Err(Error::OutsideDomain { .. })
    ));
    assert!(matches!(
        BaiduMercator::try_from_bd09(Bd09::new(80.0, 0.0)?),
        Err(Error::OutsideDomain { .. })
    ));
    assert!(
        geodesic_distance(&Coordinate::wgs84(0.0, 0.0)?, &Coordinate::gcj02(0.0, 1.0)?).is_err()
    );
    assert!(
        destination(
            &Coordinate::gcj02(0.0, 0.0)?,
            90.0,
            Length::from_meters(1.0)
        )
        .is_err()
    );
    assert!(
        Ecef::try_from_coordinate(
            Coordinate::wgs84(0.0, 0.0)?.with_height(Height::Orthometric(10.0))?
        )
        .is_err()
    );
    // GCJ-02 and BD-09 are obfuscated renderings, not datums: they have no
    // ellipsoid and so no ECEF position, and no transform can name them. A
    // coordinate carrying one is on the wrong datum; one named as an argument
    // is an invalid value for that argument.
    assert_eq!(Ellipsoid::for_crs(Crs::Gcj02), None);
    assert!(matches!(
        Ecef::try_from_coordinate(Coordinate::gcj02(39.9, 116.4)?),
        Err(Error::CrsMismatch { .. })
    ));
    assert!(matches!(
        Ecef::new(6_378_137.0, 0.0, 0.0).try_to_coordinate(Crs::Bd09),
        Err(Error::InvalidValue { field: "crs", .. })
    ));
    assert!(Ellipsoid::new(Length::from_meters(-1.0), 298.257_223_563).is_err());
    assert!(DatumTransform::new(Crs::Gcj02, Crs::Wgs84, Helmert::IDENTITY, None).is_err());
    assert!(
        Enu::try_from_coordinate(
            Coordinate::gcj02(40.0, -75.0)?,
            Coordinate::wgs84(40.0, -75.0)?
        )
        .is_err()
    );
    assert!(matches!(
        Enu::try_from_coordinate(
            Coordinate::new(40.0, -75.0, Crs::Nad27)?,
            Coordinate::wgs84(40.0, -75.0)?
        ),
        Err(Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Nad27
        })
    ));
    assert!(matches!(
        Enu::new(f64::NAN, 0.0, 0.0),
        Err(Error::InvalidValue { field: "east", .. })
    ));
    let nad27_transform = DatumTransform::catalog_to_wgs84(Crs::Nad27).expect("catalogued");
    assert!(
        nad27_transform
            .apply(Coordinate::wgs84(40.0, -100.0)?)
            .is_err()
    );
    assert!(matches!(
        PlusCode::try_from("not-a-plus-code"),
        Err(Error::InvalidGridRef(_))
    ));
    assert!(PlusCode::encode(new_york(), 3).is_err());
    assert!(Geohash::encode(new_york(), 23).is_err());
    assert!(Maidenhead::encode(new_york(), 0).is_err());
    // MGRS precision is the closed `MgrsPrecision` set, so an off-grid size
    // such as 250 m is unrepresentable; only the string side can be invalid,
    // and deserialization validates it as parsing does.
    assert!(Mgrs::try_from("18TWK000000277577").is_err());
    assert!(serde_json::from_str::<Mgrs>(r#""18TWK0000027757""#).is_ok());
    assert!(serde_json::from_str::<Mgrs>(r#""99TWK0000027757""#).is_err());
    assert!(H3Cell::encode(new_york(), 16).is_err());
    assert!(H3Cell::encode(Coordinate::gcj02(40.0, -75.0)?, 9).is_err());
    // Cell ids are valid by construction: an undefined raw id is rejected
    // when it is wrapped (or deserialized), so `decode` itself is infallible.
    assert_eq!(
        H3Cell::new(0),
        Err(Error::InvalidCellId {
            grid: "H3",
            value: 0,
        })
    );
    assert!(serde_json::from_str::<H3Cell>("0").is_err());
    assert!(S2CellId::encode(new_york(), 31).is_err());
    assert!(S2CellId::encode(Coordinate::bd09(40.0, -75.0)?, 20).is_err());
    assert!(S2CellId::new(0).is_err());
    assert!(serde_json::from_str::<S2CellId>("0").is_err());
    assert!(matches!(
        parse_coordinate("not a coordinate"),
        Err(Error::Parse {
            kind: ParseErrorKind::Syntax,
            ..
        })
    ));
    assert!(from_geo_uri("geo:1,2;u=-1").is_err());
    assert!(from_geo_uri("geo:1,2;u=1;crs=wgs84").is_err());
    assert!(parse_text("40 60 N 74 0 W").is_err());
    assert!(matches!(
        Utm::try_from_coordinate(Coordinate::wgs84(85.0, 0.0)?),
        Err(Error::OutsideDomain {
            operation: "UTM",
            ..
        })
    ));
    assert!(matches!(
        Ups::try_from_coordinate(Coordinate::wgs84(40.0, -75.0)?),
        Err(Error::OutsideDomain {
            operation: "UPS",
            ..
        })
    ));
    // Utm / Ups are valid by construction: an out-of-domain part is rejected
    // up front, so decoding is infallible.
    assert!(matches!(
        Utm::new(0, UtmHemisphere::North, 500_000.0, 0.0),
        Err(Error::InvalidValue { field: "zone", .. })
    ));
    // A northing past the pole would fold onto the far meridian.
    assert!(matches!(
        Utm::new(1, UtmHemisphere::North, 500_000.0, 9_999_000.0),
        Err(Error::InvalidValue {
            field: "northing",
            ..
        })
    ));
    assert!(matches!(
        Ups::new(UtmHemisphere::South, -1.0, 2_000_000.0),
        Err(Error::InvalidValue {
            field: "easting",
            ..
        })
    ));
    // Zone 1 west of the antimeridian decodes to a wrapped longitude.
    let across = Utm::new(1, UtmHemisphere::North, 150_000.0, 0.0)?.to_coordinate();
    assert!(across.lon() > 179.8);
    assert!(matches!(
        utm_zone_for(&Coordinate::wgs84(84.0, 9.0)?),
        Err(Error::OutsideDomain {
            operation: "UTM",
            ..
        })
    ));
    assert!(matches!(
        utm_central_meridian_deg(0),
        Err(Error::InvalidValue { field: "zone", .. })
    ));
    assert!(matches!(
        from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1E"),
        Err(Error::Parse {
            kind: ParseErrorKind::Checksum,
            ..
        })
    ));
    // A sentence reporting no fix is an error, never a position: RMC status V
    // (void) and GGA fix quality 0. Its kind lets a stream skip it.
    assert!(matches!(
        from_nmea_sentence("$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07"),
        Err(Error::Parse {
            kind: ParseErrorKind::NoFix,
            ..
        })
    ));
    assert!(matches!(
        from_nmea_sentence("$GPGGA,123519,,,,,0,00,,,M,,M,,"),
        Err(Error::Parse {
            kind: ParseErrorKind::NoFix,
            ..
        })
    ));

    Ok("boundaries and typed errors")
}
