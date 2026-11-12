//! Valid by construction through serde: for every type `AGENTS.md` lists, a
//! JSON record deserializes exactly when its constructor accepts the same
//! fields, and then to the same value. The generated fields cross every
//! range edge; JSON carries no NaN or infinity, so the floats are finite
//! (overflowing magnitudes included), and each constructor's own tests cover
//! the non-finite inputs.

use proptest::prelude::*;
use proptest::sample::select;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::angle::{Ddm, Dms, Hemisphere};
use crate::china::{BaiduMercator, Bd09, Gcj02, Wgs84};
use crate::fix::{Accuracy, AccuracyKind, Confidence};
use crate::geodesy::{Aer, DatumTransform, Ellipsoid, Enu, Helmert, Ned};
use crate::grids::{Geohash, Maidenhead, Mgrs, MgrsPrecision, PlusCode, Ups, Utm, UtmHemisphere};
use crate::{Coordinate, Crs, Height, Length, Result};

/// Fail the case unless `record` deserializes exactly when `built` is `Ok`,
/// and to the same value.
fn agrees<T>(record: Value, built: Result<T>) -> core::result::Result<(), TestCaseError>
where
    T: DeserializeOwned + PartialEq + core::fmt::Debug,
{
    let read = serde_json::from_value::<T>(record.clone());
    prop_assert_eq!(read.ok(), built.ok(), "record {}", record);
    Ok(())
}

/// Any finite `f64`, the only floats JSON carries.
fn finite() -> impl Strategy<Value = f64> {
    any::<f64>().prop_filter("JSON carries only finite numbers", |v| v.is_finite())
}

/// Mostly `lo..=hi` and the listed edges, sometimes any finite value.
fn around(lo: f64, hi: f64, edges: &'static [f64]) -> impl Strategy<Value = f64> {
    prop_oneof![4 => lo..=hi, 2 => select(edges), 1 => finite()]
}

/// A length in meters drawn as [`around`] draws a float.
fn length(lo: f64, hi: f64, edges: &'static [f64]) -> impl Strategy<Value = Length> {
    around(lo, hi, edges).prop_map(Length::from_meters)
}

fn lat() -> impl Strategy<Value = f64> {
    around(
        -100.0,
        100.0,
        &[-90.0, 90.0, -90.000_000_1, 90.000_000_1, 0.0, -0.0],
    )
}

fn lon() -> impl Strategy<Value = f64> {
    around(
        -200.0,
        200.0,
        &[-180.0, 180.0, -180.000_000_1, 180.000_000_1, 0.0],
    )
}

fn crs() -> impl Strategy<Value = Crs> {
    select(
        &[
            Crs::Wgs84,
            Crs::Gcj02,
            Crs::Bd09,
            Crs::Nad27,
            Crs::Tokyo,
            Crs::Pulkovo42,
        ][..],
    )
}

fn hemisphere() -> impl Strategy<Value = Hemisphere> {
    select(
        &[
            Hemisphere::North,
            Hemisphere::South,
            Hemisphere::East,
            Hemisphere::West,
        ][..],
    )
}

fn utm_hemisphere() -> impl Strategy<Value = UtmHemisphere> {
    select(&[UtmHemisphere::North, UtmHemisphere::South][..])
}

/// A valid WGS-84 position, for building codes an encoder emits.
fn position() -> impl Strategy<Value = Coordinate> + Clone {
    (-90.0..=90.0_f64, -180.0..=180.0_f64)
        .prop_map(|(lat, lon)| Coordinate::wgs84(lat, lon).unwrap())
}

/// Encoder output, its other-case spelling, and arbitrary text.
fn code(
    valid: impl Strategy<Value = String> + Clone,
    alphabet: &'static str,
) -> impl Strategy<Value = String> {
    prop_oneof![
        2 => valid.clone(),
        1 => valid.prop_map(|s| s.to_ascii_lowercase()),
        2 => proptest::string::string_regex(alphabet).unwrap(),
        1 => "[ -~]{0,20}",
    ]
}

/// A cell id an encoder emits, the same id with one bit flipped, or any `u64`.
#[cfg(any(feature = "h3", feature = "s2"))]
fn cell_id(valid: impl Strategy<Value = u64> + Clone) -> impl Strategy<Value = u64> {
    prop_oneof![
        1 => valid.clone(),
        2 => (valid, 0..64_u32).prop_map(|(id, bit)| id ^ (1 << bit)),
        1 => any::<u64>(),
    ]
}

proptest! {
    #[test]
    fn coordinate(
        lat in lat(),
        lon in lon(),
        crs in crs(),
        height in proptest::option::of(prop_oneof![
            finite().prop_map(Height::Ellipsoidal),
            finite().prop_map(Height::Orthometric),
        ]),
    ) {
        let built = Coordinate::new(lat, lon, crs).and_then(|c| match height {
            Some(h) => c.with_height(h),
            None => Ok(c),
        });
        agrees(json!({"lat": lat, "lon": lon, "height": height, "crs": crs}), built)?;
    }

    #[test]
    fn dms(
        degrees in prop_oneof![0..=200_u16, any::<u16>()],
        minutes in prop_oneof![0..=70_u8, any::<u8>()],
        seconds in around(-10.0, 70.0, &[0.0, -0.0, 59.999_999, 60.0]),
        hemisphere in hemisphere(),
    ) {
        let record = json!({
            "degrees": degrees, "minutes": minutes, "seconds": seconds, "hemisphere": hemisphere,
        });
        agrees(record, Dms::new(degrees, minutes, seconds, hemisphere))?;
    }

    #[test]
    fn ddm(
        degrees in prop_oneof![0..=200_u16, any::<u16>()],
        minutes in around(-10.0, 70.0, &[0.0, -0.0, 59.999_999, 60.0]),
        hemisphere in hemisphere(),
    ) {
        let record = json!({"degrees": degrees, "minutes": minutes, "hemisphere": hemisphere});
        agrees(record, Ddm::new(degrees, minutes, hemisphere))?;
    }

    #[test]
    fn china_newtypes(lat in lat(), lon in lon()) {
        let record = json!({"lat": lat, "lon": lon});
        agrees(record.clone(), Wgs84::new(lat, lon))?;
        agrees(record.clone(), Gcj02::new(lat, lon))?;
        agrees(record, Bd09::new(lat, lon))?;
    }

    #[test]
    fn baidu_mercator(
        x in around(-3e7, 3e7, &[0.0, -2.0e7, 2.0e7]),
        y in around(-3e7, 3e7, &[0.0, -1.3e7, 1.3e7]),
    ) {
        agrees(json!({"x": x, "y": y}), BaiduMercator::new(x, y))?;
    }

    #[test]
    fn ellipsoid(
        semi_major in length(-1e7, 1e7, &[0.0, -0.0, 6_378_137.0, f64::MIN_POSITIVE]),
        inverse_flattening in around(-10.0, 400.0, &[0.0, 1.0, 1.000_000_1, 298.257_223_563]),
    ) {
        let record = json!({"semi_major": semi_major, "inverse_flattening": inverse_flattening});
        agrees(record, Ellipsoid::new(semi_major, inverse_flattening))?;
    }

    #[test]
    fn enu_and_ned(
        a in around(-1e9, 1e9, &[0.0, 1.7e308, -1.7e308, f64::MAX]),
        b in around(-1e9, 1e9, &[0.0, 1.7e308, -1.7e308, f64::MAX]),
        c in around(-1e9, 1e9, &[0.0, 1.7e308, -1.7e308, f64::MAX]),
    ) {
        agrees(json!({"east_m": a, "north_m": b, "up_m": c}), Enu::new(a, b, c))?;
        agrees(json!({"north_m": a, "east_m": b, "down_m": c}), Ned::new(a, b, c))?;
    }

    #[test]
    fn aer(
        azimuth in around(-1000.0, 1000.0, &[0.0, 360.0, -360.0, 359.999_999]),
        elevation in around(-100.0, 100.0, &[-90.0, 90.0, 90.000_000_1, -90.000_000_1]),
        range in length(-10.0, 1e7, &[0.0, -0.0, -1e-9, 1.7e308]),
    ) {
        let record = json!({"azimuth_deg": azimuth, "elevation_deg": elevation, "range": range});
        agrees(record, Aer::new(azimuth, elevation, range))?;
    }

    #[test]
    fn utm(
        zone in prop_oneof![0..=62_u8, any::<u8>()],
        hemisphere in utm_hemisphere(),
        easting in around(-1e6, 2e6, &[0.0, 100_000.0, 500_000.0, 900_000.0, 1e6]),
        northing in around(-1e6, 1.2e7, &[0.0, -0.0, 9_999_999.0, 1e7]),
    ) {
        let record = json!({
            "zone": zone, "hemisphere": hemisphere, "easting": easting, "northing": northing,
        });
        agrees(record, Utm::new(zone, hemisphere, easting, northing))?;
    }

    #[test]
    fn ups(
        hemisphere in utm_hemisphere(),
        easting in around(-1e6, 5e6, &[0.0, 2e6, 4e6]),
        northing in around(-1e6, 5e6, &[0.0, 2e6, 4e6]),
    ) {
        let record = json!({"hemisphere": hemisphere, "easting": easting, "northing": northing});
        agrees(record, Ups::new(hemisphere, easting, northing))?;
    }

    #[test]
    fn datum_transform(
        from_crs in crs(),
        to_crs in crs(),
        params in proptest::array::uniform7(around(-1000.0, 1000.0, &[0.0])),
        accuracy in proptest::option::of(length(-10.0, 100.0, &[0.0, -0.0, -1e-9])),
    ) {
        let [tx_m, ty_m, tz_m, rx_arcsec, ry_arcsec, rz_arcsec, scale_ppm] = params;
        let helmert = Helmert { tx_m, ty_m, tz_m, rx_arcsec, ry_arcsec, rz_arcsec, scale_ppm };
        let record = json!({
            "from_crs": from_crs, "to_crs": to_crs, "helmert": helmert, "accuracy": accuracy,
        });
        agrees(record, DatumTransform::new(from_crs, to_crs, helmert, accuracy))?;
    }

    #[test]
    fn accuracy(
        kind in select(&[
            AccuracyKind::Bound, AccuracyKind::OneSigma, AccuracyKind::Cep50, AccuracyKind::Unspecified,
        ][..]),
        horizontal in proptest::option::of(length(-10.0, 100.0, &[0.0, -0.0, -1e-9])),
        vertical in proptest::option::of(length(-10.0, 100.0, &[0.0, -0.0, -1e-9])),
    ) {
        let record = json!({"kind": kind, "horizontal": horizontal, "vertical": vertical});
        agrees(record, Accuracy::new(kind, horizontal, vertical))?;
    }

    #[test]
    fn confidence(value in around(-1.0, 2.0, &[0.0, -0.0, 1.0, 1.000_000_1, -1e-9])) {
        agrees(json!(value), Confidence::new(value))?;
    }

    #[test]
    fn geohash(
        s in code(
            (position(), 1..=12_u8).prop_map(|(c, n)| Geohash::encode(c, n).unwrap().into()),
            "[0-9b-hjkmnp-zA-Z]{0,14}",
        ),
    ) {
        agrees(json!(s), Geohash::parse(&s))?;
    }

    #[test]
    fn plus_code(
        s in code(
            (position(), select(&[2_u8, 4, 6, 8, 10, 11, 12, 15][..]))
                .prop_map(|(c, n)| PlusCode::encode(c, n).unwrap().into()),
            "[02-9CFGHJMPQRVWX]{0,9}\\+?[2-9CFGHJMPQRVWX]{0,4}",
        ),
    ) {
        agrees(json!(s), PlusCode::parse(&s))?;
    }

    #[test]
    fn maidenhead(
        s in code(
            (position(), 1..=3_u8).prop_map(|(c, n)| Maidenhead::encode(c, n).unwrap().into()),
            "[A-Sa-s]{2}([0-9]{2}([a-yA-Y]{2})?)?",
        ),
    ) {
        agrees(json!(s), Maidenhead::parse(&s))?;
    }

    #[test]
    fn mgrs(
        s in code(
            (
                -80.0..=84.0_f64,
                -180.0..=180.0_f64,
                select(&[
                    MgrsPrecision::Km100, MgrsPrecision::Km10, MgrsPrecision::Km1,
                    MgrsPrecision::M100, MgrsPrecision::M10, MgrsPrecision::M1,
                ][..]),
            )
                .prop_map(|(lat, lon, p)| {
                    let c = Coordinate::wgs84(lat, lon).unwrap();
                    Mgrs::try_from_coordinate(c, p).unwrap().into()
                }),
            "[0-9]{1,2}[A-Za-z]{3} ?[0-9]{0,10}",
        ),
    ) {
        agrees(json!(s), Mgrs::parse(&s))?;
    }
}

#[cfg(feature = "index")]
proptest! {
    #[test]
    fn geo_bounds(south in lat(), west in lon(), north in lat(), east in lon(), crs in crs()) {
        use crate::index::GeoBounds;
        let record = json!({"south": south, "west": west, "north": north, "east": east, "crs": crs});
        agrees(record, GeoBounds::new(south, west, north, east, crs))?;
    }
}

#[cfg(feature = "exif")]
proptest! {
    #[test]
    fn north_bearing(
        degrees in around(-400.0, 400.0, &[0.0, -0.0, 360.0, 359.999_999, -1e-9]),
        reference in proptest::option::of(select(&[
            crate::exif::NorthReference::True, crate::exif::NorthReference::Magnetic,
        ][..])),
    ) {
        use crate::exif::NorthBearing;
        agrees(json!({"degrees": degrees, "reference": reference}), NorthBearing::new(degrees, reference))?;
    }
}

#[cfg(feature = "h3")]
proptest! {
    #[test]
    fn h3_cell(
        id in cell_id((position(), 0..=15_u8).prop_map(|(c, r)| {
            crate::dgg::H3Cell::encode(c, r).unwrap().into()
        })),
    ) {
        use crate::dgg::H3Cell;
        agrees(json!(id), H3Cell::new(id))?;
    }
}

#[cfg(feature = "s2")]
proptest! {
    #[test]
    fn s2_cell_id(
        id in cell_id((position(), 0..=30_u8).prop_map(|(c, l)| {
            crate::dgg::S2CellId::encode(c, l).unwrap().into()
        })),
    ) {
        use crate::dgg::S2CellId;
        agrees(json!(id), S2CellId::new(id))?;
    }
}
