//! Angle encodings, coordinate primitives, and serde coverage of value types.

use std::hash::Hash;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use geocoordinates::angle::{
    Axis, Ddm, Dms, Hemisphere as AngleHemisphere, clamp_latitude, normalize_degrees,
    wrap_longitude,
};
use geocoordinates::china::{BaiduMercator, Bd09, Gcj02, Wgs84};
use geocoordinates::dgg::{H3Cell, S2CellId};
use geocoordinates::fix::{
    Accuracy, AccuracyKind, AxisOrder, Confidence, DatumAmbiguity, Fix, RawSource,
};
use geocoordinates::format::{
    DecimalSeparator, FormatOptions, HemisphereStyle, Representation, SymbolStyle,
};
use geocoordinates::geodesy::{Aer, DatumTransform, Ecef, Ellipsoid, Enu, Helmert, Ned};
use geocoordinates::grids::{
    Geohash, Maidenhead, Mgrs, MgrsPrecision, PlusCode, Ups, Utm, UtmHemisphere,
};
use geocoordinates::parse::{GridKind, ParseOptions};
use geocoordinates::{Approx, Coordinate, Crs, Height, Length, LengthUnit};
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
    assert_serde::<AccuracyKind>();
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
    assert_serde::<DecimalSeparator>();
    assert_serde::<ParseOptions>();
    assert_serde::<GridKind>();
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
    assert_serde::<MgrsPrecision>();
    assert_serde::<Utm>();
    assert_serde::<Ups>();
    assert_serde::<UtmHemisphere>();
    assert_serde::<H3Cell>();
    assert_serde::<S2CellId>();
}

fn assert_hash<T: Eq + Hash>() {}

/// Every value enum and options type whose equality is total also hashes, so
/// it can key a `HashMap` or fill a `HashSet`.
fn eq_value_types_are_hashable() {
    assert_hash::<AngleHemisphere>();
    assert_hash::<Axis>();
    assert_hash::<Crs>();
    assert_hash::<AxisOrder>();
    assert_hash::<DatumAmbiguity>();
    assert_hash::<AccuracyKind>();
    assert_hash::<LengthUnit>();
    assert_hash::<Representation>();
    assert_hash::<SymbolStyle>();
    assert_hash::<HemisphereStyle>();
    assert_hash::<DecimalSeparator>();
    assert_hash::<FormatOptions>();
    assert_hash::<ParseOptions>();
    assert_hash::<GridKind>();
    assert_hash::<UtmHemisphere>();
}

pub(crate) fn primitives_and_serde() -> DemoResult {
    // Construction validates, so a `Coordinate` in hand is always in range.
    let coordinate = Coordinate::wgs84(40.7128, -74.006)?.with_height(Height::Ellipsoidal(12.5))?;
    assert!(!coordinate.is_null_island());
    assert!(Coordinate::wgs84(0.0, 0.0)?.is_null_island());
    assert_eq!(coordinate.crs().to_string(), "WGS84");
    assert_eq!(coordinate.height(), Some(Height::Ellipsoidal(12.5)));
    // Either surface reads back as a `Length`.
    assert_eq!(
        coordinate.height().map(Height::meters),
        Some(Length::from_meters(12.5))
    );
    assert_close(coordinate.lat(), 40.7128, 1e-12);
    assert_close(coordinate.lon(), -74.006, 1e-12);
    // Lon-first input (GeoJSON / WKT order) has its own named constructor.
    assert_eq!(
        Coordinate::from_lon_lat(-74.006, 40.7128, Crs::Wgs84)?,
        Coordinate::wgs84(40.7128, -74.006)?
    );

    // Decimal degrees are a plain f64; the axis is stated at every conversion.
    let dd = -73.9857;
    let dms = Dms::from_degrees(dd, Axis::Longitude)?;
    assert_eq!(dms.hemisphere(), AngleHemisphere::West);
    assert_eq!((dms.degrees(), dms.minutes()), (73, 59));
    assert_close(dms.seconds(), 8.52, 1e-9);
    assert_close(dms.to_degrees(), dd, 1e-12);
    // Dms and Ddm are valid by construction, so converting between them is
    // infallible.
    let ddm = dms.to_ddm();
    assert_eq!(
        Ddm::from_degrees(dd, Axis::Longitude)?.degrees(),
        ddm.degrees()
    );
    assert_close(ddm.minutes(), 59.142, 1e-9);
    assert_close(ddm.to_degrees(), dd, 1e-12);
    assert_eq!(ddm.to_dms().hemisphere(), AngleHemisphere::West);
    assert_eq!(
        Dms::new(40, 42, 46.0, AngleHemisphere::North)?.minutes(),
        42
    );
    assert_eq!(Ddm::new(40, 42.5, AngleHemisphere::North)?.minutes(), 42.5);
    assert_eq!(
        Dms::from_degrees(40.0, Axis::Latitude)?.hemisphere(),
        AngleHemisphere::North
    );
    assert_eq!(AngleHemisphere::North.sign(), 1.0);
    assert_eq!(AngleHemisphere::East.sign(), 1.0);
    assert_eq!(AngleHemisphere::South.sign(), -1.0);
    assert_eq!(AngleHemisphere::West.sign(), -1.0);

    assert_eq!(wrap_longitude(181.0), -179.0);
    assert_eq!(clamp_latitude(95.0), 90.0);
    assert_eq!(normalize_degrees(-10.0), 350.0);
    // [0, 360) is half-open: a tiny negative never rounds up to 360.
    assert_eq!(normalize_degrees(-1e-20), 0.0);

    for unit in [
        LengthUnit::Meter,
        LengthUnit::Kilometer,
        LengthUnit::Foot,
        LengthUnit::UsSurveyFoot,
        LengthUnit::NauticalMile,
        LengthUnit::Mile,
        LengthUnit::Yard,
    ] {
        let one = Length::from_unit(1.0, unit);
        assert_close(one.to_unit(unit), 1.0, 1e-12);
        assert_eq!(one.meters(), unit.meters_per_unit());
    }
    assert_eq!(Length::ZERO.meters(), 0.0);
    assert_eq!(
        Length::from_unit(1.0, LengthUnit::NauticalMile).meters(),
        1_852.0
    );
    let arithmetic =
        (Length::from_meters(10.0) + Length::from_meters(5.0) - Length::from_meters(3.0)) * 2.0;
    assert_eq!(arithmetic.meters(), 24.0);

    // Confidence and accuracy are valid by construction.
    assert!(Confidence::new(-1.0).is_err() && Confidence::new(f64::NAN).is_err());
    assert!(Accuracy::new(AccuracyKind::Bound, Some(Length::from_meters(-1.0)), None).is_err());
    let mut source = RawSource::new("40.7128, -74.006", Confidence::new(0.9)?);
    source.axis_order = Some(AxisOrder::LatLon);
    source.datum_ambiguity = Some(DatumAmbiguity::PossiblyGcj02);
    source.geoid_separation = Some(Length::from_meters(-34.2));
    source.notes.push("flagship fixture".to_owned());
    let mut fix = Fix::from_coord(coordinate);
    fix.accuracy = Some(Accuracy::new(
        AccuracyKind::Cep50,
        Some(Length::from_meters(3.0)),
        Some(Length::from_unit(5.0, LengthUnit::Yard)),
    )?);
    fix.timestamp = Some(UNIX_EPOCH + Duration::from_secs(1_700_000_000));
    fix.source = Some(source);
    assert_eq!(Fix::from_coord(coordinate).coord, coordinate);

    all_public_value_types_are_serde();
    eq_value_types_are_hashable();
    let json = serde_json::to_string(&fix)?;
    let decoded: Fix = serde_json::from_str(&json)?;
    assert_eq!(decoded, fix);
    // `Confidence` is a bare number on the wire, validated on its way in.
    assert_eq!(serde_json::to_string(&Confidence::new(0.5)?)?, "0.5");
    assert!(serde_json::from_str::<Confidence>("2.0").is_err());
    let axis_json = serde_json::to_string(&Axis::Longitude)?;
    assert_eq!(serde_json::from_str::<Axis>(&axis_json)?, Axis::Longitude);

    Ok("primitives and serde")
}
