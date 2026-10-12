//! Angle encodings, coordinate primitives, and serde coverage of value types.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use geocoordinates::angle::{
    Axis, Dd, Ddm, Dms, Hemisphere as AngleHemisphere, clamp_latitude, normalize_degrees,
    wrap_longitude,
};
use geocoordinates::dgg::{H3Cell, S2CellId};
use geocoordinates::fix::{AxisOrder, DatumAmbiguity};
use geocoordinates::format::{FormatOptions, HemisphereStyle, Representation, SymbolStyle};
use geocoordinates::geodesy::{Aer, DatumTransform, Ecef, Ellipsoid, Enu, Helmert, Ned};
use geocoordinates::grids::utm::Hemisphere as GridHemisphere;
use geocoordinates::grids::{Geohash, Maidenhead, Mgrs, PlusCode, Ups, Utm};
use geocoordinates::parse::text::TextParseOptions;
use geocoordinates::{
    Accuracy, Approx, BaiduMercator, Bd09, Confidence, Coordinate, Crs, Fix, Gcj02, Height, Length,
    LengthUnit, RawSource, Wgs84,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::support::{DemoResult, assert_close};

fn assert_serde<T>()
where
    T: Serialize + DeserializeOwned,
{
}

fn all_public_value_types_are_serde() {
    assert_serde::<AngleHemisphere>();
    assert_serde::<Axis>();
    assert_serde::<Dd>();
    assert_serde::<Dms>();
    assert_serde::<Ddm>();
    assert_serde::<Approx<Coordinate>>();
    assert_serde::<BaiduMercator>();
    assert_serde::<Bd09>();
    assert_serde::<Gcj02>();
    assert_serde::<Wgs84>();
    assert_serde::<Coordinate>();
    assert_serde::<Crs>();
    assert_serde::<Height>();
    assert_serde::<Accuracy>();
    assert_serde::<Confidence>();
    assert_serde::<Fix>();
    assert_serde::<RawSource>();
    assert_serde::<AxisOrder>();
    assert_serde::<DatumAmbiguity>();
    assert_serde::<SystemTime>();
    assert_serde::<Length>();
    assert_serde::<LengthUnit>();
    assert_serde::<Representation>();
    assert_serde::<SymbolStyle>();
    assert_serde::<HemisphereStyle>();
    assert_serde::<FormatOptions>();
    assert_serde::<TextParseOptions>();
    assert_serde::<Ellipsoid>();
    assert_serde::<Ecef>();
    assert_serde::<Enu>();
    assert_serde::<Ned>();
    assert_serde::<Aer>();
    assert_serde::<Helmert>();
    assert_serde::<DatumTransform>();
    assert_serde::<PlusCode>();
    assert_serde::<Geohash>();
    assert_serde::<Maidenhead>();
    assert_serde::<Mgrs>();
    assert_serde::<Utm>();
    assert_serde::<Ups>();
    assert_serde::<GridHemisphere>();
    assert_serde::<H3Cell>();
    assert_serde::<S2CellId>();
}

pub(crate) fn primitives_and_serde() -> DemoResult {
    // Construction validates, so a `Coordinate` in hand is always in range.
    let coordinate = Coordinate::wgs84(40.7128, -74.006)?.with_height(Height::Ellipsoidal(12.5))?;
    assert!(!coordinate.is_null_island());
    assert!(Coordinate::wgs84(0.0, 0.0)?.is_null_island());
    assert_eq!(coordinate.crs().to_string(), "WGS84");
    assert_eq!(coordinate.height(), Some(Height::Ellipsoidal(12.5)));
    assert_close(coordinate.lat(), 40.7128, 1e-12);
    assert_close(coordinate.lon(), -74.006, 1e-12);
    // Lon-first input (GeoJSON / WKT order) has its own named constructor.
    assert_eq!(
        Coordinate::from_lon_lat(-74.006, 40.7128, Crs::Wgs84)?,
        Coordinate::wgs84(40.7128, -74.006)?
    );

    let dd = Dd(-73.9857);
    let dms = dd.try_to_dms(Axis::Longitude)?;
    assert_eq!(dms.hemisphere, AngleHemisphere::West);
    assert_close(dms.try_to_dd()?.0, dd.0, 1e-12);
    let ddm = dms.try_to_ddm()?;
    assert_eq!(dd.try_to_ddm(Axis::Longitude)?, ddm);
    assert_close(ddm.try_to_dd()?.0, dd.0, 1e-12);
    assert_eq!(ddm.try_to_dms()?.hemisphere, AngleHemisphere::West);
    assert_eq!(
        Dd(40.0).try_to_dms(Axis::Latitude)?.hemisphere,
        AngleHemisphere::North
    );
    assert_eq!(AngleHemisphere::North.sign(), 1.0);
    assert_eq!(AngleHemisphere::East.sign(), 1.0);
    assert_eq!(AngleHemisphere::South.sign(), -1.0);
    assert_eq!(AngleHemisphere::West.sign(), -1.0);

    assert_eq!(wrap_longitude(181.0), -179.0);
    assert_eq!(clamp_latitude(95.0), 90.0);
    assert_eq!(normalize_degrees(-10.0), 350.0);

    for unit in [
        LengthUnit::Meter,
        LengthUnit::Kilometer,
        LengthUnit::Foot,
        LengthUnit::UsSurveyFoot,
        LengthUnit::NauticalMile,
    ] {
        let one = Length::from_unit(1.0, unit);
        assert_close(one.to_unit(unit), 1.0, 1e-12);
    }
    assert_eq!(Length::ZERO.meters(), 0.0);
    assert_eq!(
        Length::from_unit(1.0, LengthUnit::NauticalMile).meters(),
        1_852.0
    );
    let arithmetic =
        (Length::from_meters(10.0) + Length::from_meters(5.0) - Length::from_meters(3.0)) * 2.0;
    assert_eq!(arithmetic.meters(), 24.0);

    assert_eq!(Confidence::new(-1.0).value(), 0.0);
    assert_eq!(Confidence::new(2.0).value(), 1.0);
    let fix = Fix {
        coord: coordinate,
        accuracy: Some(Accuracy {
            horizontal_m: Some(3.0),
            vertical_m: Some(5.0),
        }),
        timestamp: Some(UNIX_EPOCH + Duration::from_secs(1_700_000_000)),
        source: Some(RawSource {
            raw: "40.7128, -74.006".to_owned(),
            confidence: Confidence::new(0.9),
            axis_order: Some(AxisOrder::LatLon),
            datum_ambiguity: Some(DatumAmbiguity::PossiblyGcj02),
            notes: vec!["flagship fixture".to_owned()],
        }),
    };
    assert_eq!(Fix::from_coord(coordinate).coord, coordinate);

    all_public_value_types_are_serde();
    let json = serde_json::to_string(&fix)?;
    let decoded: Fix = serde_json::from_str(&json)?;
    assert_eq!(decoded, fix);
    // `Confidence` is a bare number on the wire and deserializes through its clamp.
    assert_eq!(serde_json::to_string(&Confidence::new(0.5))?, "0.5");
    assert_eq!(serde_json::from_str::<Confidence>("2.0")?.value(), 1.0);
    let axis_json = serde_json::to_string(&Axis::Longitude)?;
    assert_eq!(serde_json::from_str::<Axis>(&axis_json)?, Axis::Longitude);

    Ok("primitives and serde")
}
