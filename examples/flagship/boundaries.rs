//! Antimeridian, pole, and range boundaries, and the typed errors they raise.

use geocoordinates::angle::{Axis, Dd, Dms, Hemisphere as AngleHemisphere, wrap_longitude};
use geocoordinates::dgg::{H3Cell, S2CellId};
use geocoordinates::format::{FormatOptions, format};
use geocoordinates::geodesy::{
    DatumTransform, Ecef, Ellipsoid, Enu, destination, geodesic_distance, rhumb_destination,
};
use geocoordinates::grids::utm::Hemisphere as GridHemisphere;
use geocoordinates::grids::{Geohash, Maidenhead, Mgrs, PlusCode, Ups, Utm};
use geocoordinates::parse::sensors::from_nmea_sentence;
use geocoordinates::parse::text::parse as parse_text;
use geocoordinates::parse::{from_geo_uri, parse_coordinate};
use geocoordinates::{BaiduMercator, Coordinate, Crs, Error, Height, Length, Wgs84};

use crate::support::{DemoResult, NEW_YORK};

pub(crate) fn boundaries_and_typed_errors() -> DemoResult {
    Coordinate::wgs84(90.0, 180.0).validate()?;
    Coordinate::wgs84(-90.0, -180.0).validate()?;
    assert_eq!(wrap_longitude(180.0), -180.0);
    assert_eq!(wrap_longitude(-181.0), 179.0);

    let antimeridian_distance = geodesic_distance(
        &Coordinate::wgs84(0.0, 179.9),
        &Coordinate::wgs84(0.0, -179.9),
    )?;
    assert!(antimeridian_distance.meters() < 25_000.0);
    let reflected = rhumb_destination(
        &Coordinate::wgs84(80.0, 0.0),
        0.0,
        Length::from_meters(1_668_000.0),
    )?;
    assert!((80.0..90.0).contains(&reflected.lat));
    assert_eq!(reflected.lon, -180.0);

    for polar in [
        Coordinate::wgs84(90.0, 180.0),
        Coordinate::wgs84(-90.0, -180.0),
    ] {
        let plus = PlusCode::encode(polar, 10)?.decode();
        assert!(plus.lat.is_finite() && plus.lon.is_finite());
        let h3 = H3Cell::encode(polar, 9)?.decode()?;
        assert!(h3.lat.is_finite() && h3.lon.is_finite());
        let s2 = S2CellId::encode(polar, 20)?.decode()?;
        assert!(s2.lat.is_finite() && s2.lon.is_finite());
    }

    // `Error` is `PartialEq`, so a typed error can be asserted exactly.
    assert_eq!(
        Coordinate::wgs84(91.0, 0.0).validate(),
        Err(Error::OutOfRange {
            lat: 91.0,
            lon: 0.0
        })
    );
    assert!(matches!(
        Dd(f64::NAN).try_to_dms(Axis::Latitude),
        Err(Error::InvalidValue { .. })
    ));
    assert!(matches!(
        Dms {
            degrees: 40,
            minutes: 60,
            seconds: 0.0,
            hemisphere: AngleHemisphere::North,
        }
        .try_to_dd(),
        Err(Error::InvalidValue { .. })
    ));
    assert!(
        format(
            &NEW_YORK,
            &FormatOptions {
                precision: Some(16),
                ..FormatOptions::default()
            }
        )
        .is_err()
    );
    assert_eq!(
        BaiduMercator::try_from_coordinate(Coordinate::wgs84(39.915, 116.404)),
        Err(Error::CrsMismatch {
            expected: Crs::Bd09,
            found: Crs::Wgs84,
        })
    );
    assert!(matches!(
        Wgs84::try_from(Coordinate::gcj02(39.915, 116.404)),
        Err(Error::CrsMismatch { .. })
    ));
    assert!(Wgs84::new(f64::NAN, 116.404).try_to_gcj02().is_err());
    assert!(geodesic_distance(&Coordinate::wgs84(0.0, 0.0), &Coordinate::gcj02(0.0, 1.0)).is_err());
    assert!(destination(&Coordinate::gcj02(0.0, 0.0), 90.0, Length::from_meters(1.0)).is_err());
    assert!(
        Ecef::try_from_coordinate(
            Coordinate::wgs84(0.0, 0.0).with_height(Height::Orthometric(10.0)),
            Ellipsoid::WGS84
        )
        .is_err()
    );
    assert!(
        Enu::try_from_coordinate(
            Coordinate::gcj02(40.0, -75.0),
            Coordinate::wgs84(40.0, -75.0)
        )
        .is_err()
    );
    let nad27_transform = DatumTransform::to_wgs84(Crs::Nad27).expect("catalogued");
    assert!(
        nad27_transform
            .transform(Coordinate::wgs84(40.0, -100.0))
            .is_err()
    );
    assert!(matches!(
        PlusCode::try_from("not-a-plus-code"),
        Err(Error::InvalidGridRef(_))
    ));
    assert!(PlusCode::encode(NEW_YORK, 3).is_err());
    assert!(Geohash::encode(NEW_YORK, 23).is_err());
    assert!(Maidenhead::encode(NEW_YORK, 0).is_err());
    assert!(Mgrs::try_from_coordinate(NEW_YORK, 250).is_err());
    assert!(Mgrs::try_from("18TWK000000277577").is_err());
    assert!(H3Cell::encode(NEW_YORK, 16).is_err());
    assert!(H3Cell::encode(Coordinate::gcj02(40.0, -75.0), 9).is_err());
    assert_eq!(
        H3Cell(0).decode(),
        Err(Error::InvalidCellId {
            grid: "H3",
            value: 0,
        })
    );
    assert!(S2CellId::encode(NEW_YORK, 31).is_err());
    assert!(S2CellId::encode(Coordinate::bd09(40.0, -75.0), 20).is_err());
    assert!(S2CellId(0).decode().is_err());
    assert!(matches!(
        parse_coordinate("not a coordinate"),
        Err(Error::Parse(_))
    ));
    assert!(from_geo_uri("geo:1,2;u=-1").is_err());
    assert!(from_geo_uri("geo:1,2;u=1;crs=wgs84").is_err());
    assert!(parse_text("40 60 N 74 0 W").is_err());
    assert!(matches!(
        Utm::try_from_coordinate(Coordinate::wgs84(85.0, 0.0)),
        Err(Error::OutOfRange { .. })
    ));
    assert!(matches!(
        Ups::try_from_coordinate(Coordinate::wgs84(40.0, -75.0)),
        Err(Error::OutOfRange { .. })
    ));
    assert!(
        Utm {
            zone: 0,
            hemisphere: GridHemisphere::North,
            easting: 500_000.0,
            northing: 0.0,
        }
        .try_to_coordinate()
        .is_err()
    );
    assert!(matches!(
        from_nmea_sentence("$GPGLL,4916.45,N,12311.12,W,225444,A,*1E"),
        Err(Error::Parse(_))
    ));

    Ok("boundaries and typed errors")
}
