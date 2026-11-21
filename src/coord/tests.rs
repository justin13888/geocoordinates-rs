use super::*;

fn wgs(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

#[test]
fn new_accepts_in_range_incl_poles_and_antimeridian() {
    assert!(Coordinate::wgs84(40.0, -74.0).is_ok());
    assert!(Coordinate::wgs84(90.0, 0.0).is_ok());
    assert!(Coordinate::wgs84(-90.0, 0.0).is_ok());
    // Longitude ±180 is accepted (closed range; construction does not normalize).
    assert_eq!(wgs(0.0, 180.0).lon(), 180.0);
    assert_eq!(wgs(0.0, -180.0).lon(), -180.0);
    // A negative zero is stored as +0.0 on both axes.
    let zero = wgs(-0.0, -0.0);
    assert!(zero.lat().is_sign_positive() && zero.lon().is_sign_positive());
}

/// Every pair outside the closed ranges, or not finite, is the pair error
/// carrying the values given (bit for bit, so a NaN is the NaN passed).
#[test]
fn non_finite_or_out_of_range_pairs_are_out_of_range_with_values() {
    let (nan, inf) = (f64::NAN, f64::INFINITY);
    for (lat, lon) in [
        (91.0, 0.0),
        (-90.5, 0.0),
        (0.0, 181.0),
        (0.0, -180.5),
        (nan, 0.0),
        (0.0, nan),
        (inf, 0.0),
        (0.0, -inf),
        (-inf, nan),
    ] {
        for result in [
            Coordinate::wgs84(lat, lon),
            Coordinate::new(lat, lon, Crs::Nad27),
            Coordinate::from_lon_lat(lon, lat, Crs::Gcj02),
        ] {
            let Err(Error::OutOfRange { lat: l, lon: o }) = result else {
                panic!("({lat}, {lon}): {result:?}");
            };
            assert_eq!((l.to_bits(), o.to_bits()), (lat.to_bits(), lon.to_bits()));
        }
    }
}

#[test]
fn with_height_rejects_non_finite_and_keeps_finite() {
    for bad in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert!(matches!(
            wgs(0.0, 0.0).with_height(Height::Ellipsoidal(bad)),
            Err(Error::InvalidValue {
                field: "height",
                ..
            })
        ));
        assert!(wgs(0.0, 0.0).with_height(Height::Orthometric(bad)).is_err());
    }
    let c = wgs(1.0, 2.0)
        .with_height(Height::Orthometric(-3.5))
        .unwrap();
    assert_eq!(c.height(), Some(Height::Orthometric(-3.5)));
    assert_eq!((c.lat(), c.lon(), c.crs()), (1.0, 2.0, Crs::Wgs84));
}

#[test]
fn constructors_tag_their_datum_and_order_their_axes() {
    assert_eq!(Coordinate::gcj02(1.0, 2.0).unwrap().crs(), Crs::Gcj02);
    assert_eq!(Coordinate::bd09(1.0, 2.0).unwrap().crs(), Crs::Bd09);
    assert_eq!(wgs(1.0, 2.0).height(), None);
    let lon_first = Coordinate::from_lon_lat(120.0, 30.0, Crs::Tokyo).unwrap();
    assert_eq!(lon_first, Coordinate::new(30.0, 120.0, Crs::Tokyo).unwrap());
    // A lon-lat pair whose "latitude" is out of range is rejected, not swapped.
    let swapped = Coordinate::from_lon_lat(10.0, 120.0, Crs::Wgs84);
    assert_eq!(
        swapped,
        Err(Error::OutOfRange {
            lat: 120.0,
            lon: 10.0
        })
    );
}

/// C4: `Coordinate` has no `Display`/`FromStr`, so the only text round trip
/// is the explicit `format` → `parse_coordinate` pair, which states its
/// datum: the result is tagged WGS-84 rather than claiming to preserve one.
#[test]
fn text_round_trip_goes_through_explicit_routes() {
    let c = wgs(40.7128, -74.006);
    let rendered = crate::format::format(c, crate::format::FormatOptions::default());
    assert_eq!(rendered.as_deref(), Ok("40.712800, -74.006000"));
    let back = crate::parse::parse_coordinate(&rendered.unwrap()).unwrap();
    assert_eq!(back.coord, c);
}

/// C9: deserialization must apply the same range check as construction.
#[cfg(feature = "serde")]
#[test]
fn deserialize_rejects_out_of_range_latitude() {
    let json = r#"{"lat":91.0,"lon":0.0,"height":null,"crs":"Wgs84"}"#;
    let parsed: serde_json::Result<Coordinate> = serde_json::from_str(json);
    assert!(parsed.is_err(), "lat = 91 deserialized: {parsed:?}");
}

#[cfg(feature = "serde")]
#[test]
fn serde_shape_is_unchanged_and_validates_height() {
    let c = wgs(1.5, -2.5)
        .with_height(Height::Ellipsoidal(7.0))
        .unwrap();
    let json = serde_json::to_string(&c).unwrap();
    assert_eq!(
        json,
        r#"{"lat":1.5,"lon":-2.5,"height":{"Ellipsoidal":7.0},"crs":"Wgs84"}"#
    );
    assert_eq!(serde_json::from_str::<Coordinate>(&json).unwrap(), c);
    let no_height = r#"{"lat":1.5,"lon":-2.5,"crs":"Gcj02"}"#;
    assert_eq!(
        serde_json::from_str::<Coordinate>(no_height).unwrap(),
        Coordinate::gcj02(1.5, -2.5).unwrap()
    );
    let bad_lon = r#"{"lat":0.0,"lon":180.5,"height":null,"crs":"Wgs84"}"#;
    assert!(serde_json::from_str::<Coordinate>(bad_lon).is_err());
}

/// JSON cannot carry a non-finite number, so the height check is reached
/// through the conversion serde calls.
#[cfg(feature = "serde")]
#[test]
fn deserialize_conversion_rejects_non_finite_height() {
    let repr = CoordinateRepr {
        lat: 0.0,
        lon: 0.0,
        height: Some(Height::Orthometric(f64::NAN)),
        crs: Crs::Wgs84,
    };
    assert!(matches!(
        Coordinate::try_from(repr),
        Err(Error::InvalidValue {
            field: "height",
            ..
        })
    ));
}

#[test]
fn null_island_detection() {
    assert!(wgs(0.0, 0.0).is_null_island());
    assert!(wgs(0.0, -0.0).is_null_island());
    assert!(wgs(1e-7, 1e-7).is_null_island());
    // ~111 m away is a real position, not Null Island.
    assert!(!wgs(0.001, 0.0).is_null_island());
    assert!(!wgs(40.0, -74.0).is_null_island());
}
