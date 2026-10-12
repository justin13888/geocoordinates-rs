//! Encoded grids (Geohash, Plus Code, Maidenhead) and global grids (H3, S2).

use geocoordinates::Coordinate;
use geocoordinates::dgg::{H3Cell, S2CellId};
use geocoordinates::grids::{Geohash, Maidenhead, PlusCode};

use crate::support::{DemoResult, NEW_YORK, assert_within_meters};

pub(crate) fn encoded_and_global_grids() -> DemoResult {
    let plus = PlusCode::encode(Coordinate::wgs84(47.000_062_5, 8.000_062_5), 10)?;
    assert_eq!(plus.as_str(), "8FVC2222+22");
    assert_eq!(PlusCode::try_from("8FVC2222+22")?, plus);
    assert_eq!("8FVC2222+22".parse::<PlusCode>()?, plus);
    let plus_area = plus.decode();
    assert_within_meters(
        plus_area.value(),
        &Coordinate::wgs84(47.000_062_5, 8.000_062_5),
        plus_area.max_error_m(),
    );
    assert!(plus_area.lat.is_finite());
    let bound = plus_area.max_error_m();
    let mapped = plus_area.map(|coordinate| (coordinate.lat, coordinate.lon));
    assert_eq!(mapped.max_error_m(), bound);
    let inner = plus_area.into_inner();
    assert!(inner.lat.is_finite() && inner.lon.is_finite());

    let geohash = Geohash::encode(Coordinate::wgs84(42.6, -5.6), 5)?;
    assert_eq!(geohash.as_str(), "ezs42");
    assert_eq!(Geohash::try_from("EZS42")?, geohash);
    assert_eq!("ezs42".parse::<Geohash>()?, geohash);
    let geohash_area = geohash.decode();
    assert_within_meters(
        geohash_area.value(),
        &Coordinate::wgs84(42.6, -5.6),
        geohash_area.max_error_m(),
    );

    let maidenhead = Maidenhead::encode(Coordinate::wgs84(40.5, -75.0), 2)?;
    assert_eq!(maidenhead.as_str(), "FN20");
    assert_eq!(Maidenhead::try_from("fn20")?, maidenhead);
    assert_eq!("FN20".parse::<Maidenhead>()?, maidenhead);
    let maidenhead_area = maidenhead.decode();
    assert_within_meters(
        maidenhead_area.value(),
        &Coordinate::wgs84(40.5, -75.0),
        maidenhead_area.max_error_m(),
    );

    let h3 = H3Cell::encode(NEW_YORK, 9)?;
    assert_eq!(h3.0, 617_733_151_020_810_239);
    let h3_area = h3.decode()?;
    assert!((205.0..206.0).contains(&h3_area.max_error_m()));
    assert_within_meters(h3_area.value(), &NEW_YORK, h3_area.max_error_m());
    assert!(
        H3Cell::encode(NEW_YORK, 12)?.decode()?.max_error_m()
            < H3Cell::encode(NEW_YORK, 5)?.decode()?.max_error_m()
    );

    let s2 = S2CellId::encode(NEW_YORK, 20)?;
    assert_eq!(s2.0, 9_926_595_630_970_437_632);
    let s2_area = s2.decode()?;
    assert!((6.0..7.0).contains(&s2_area.max_error_m()));
    assert_within_meters(s2_area.value(), &NEW_YORK, s2_area.max_error_m());
    assert!(
        S2CellId::encode(NEW_YORK, 25)?.decode()?.max_error_m()
            < S2CellId::encode(NEW_YORK, 5)?.decode()?.max_error_m()
    );

    Ok("encoded and global grids")
}
