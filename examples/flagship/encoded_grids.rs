//! Encoded grids (Geohash, Plus Code, Maidenhead) and global grids (H3, S2).

use geocoordinates::Coordinate;
use geocoordinates::dgg::{H3Cell, S2CellId};
use geocoordinates::grids::{Geohash, Maidenhead, PlusCode};

use crate::support::{DemoResult, assert_within_meters, new_york};

pub(crate) fn encoded_and_global_grids() -> DemoResult {
    let plus = PlusCode::encode(Coordinate::wgs84(47.000_062_5, 8.000_062_5)?, 10)?;
    assert_eq!(plus.as_str(), "8FVC2222+22");
    assert_eq!(PlusCode::try_from("8FVC2222+22")?, plus);
    assert_eq!("8FVC2222+22".parse::<PlusCode>()?, plus);
    // Display writes the canonical string, which parses back to an equal code.
    assert_eq!(plus.to_string().parse::<PlusCode>()?, plus);
    assert_eq!(PlusCode::try_from(String::from(plus.clone()))?, plus);
    let plus_area = plus.decode();
    assert_within_meters(
        *plus_area.value(),
        Coordinate::wgs84(47.000_062_5, 8.000_062_5)?,
        plus_area.max_error().meters(),
    );
    assert!(plus_area.value().lat().is_finite());
    let bound = plus_area.max_error().meters();
    let mapped = plus_area.map(|coordinate| (coordinate.lat(), coordinate.lon()));
    assert_eq!(mapped.max_error().meters(), bound);
    let inner = plus_area.into_inner();
    assert!(inner.lat().is_finite() && inner.lon().is_finite());

    let geohash = Geohash::encode(Coordinate::wgs84(42.6, -5.6)?, 5)?;
    assert_eq!(geohash.as_str(), "ezs42");
    assert_eq!(Geohash::try_from("EZS42")?, geohash);
    assert_eq!("ezs42".parse::<Geohash>()?, geohash);
    assert_eq!(geohash.to_string(), "ezs42");
    assert_eq!(AsRef::<str>::as_ref(&geohash), "ezs42");
    // Twelve characters is the longest geohash, encoded or parsed.
    assert_eq!(
        Geohash::encode(Coordinate::wgs84(42.6, -5.6)?, 12)?
            .as_str()
            .len(),
        12
    );
    assert!(Geohash::try_from("ezs42ezs42ezs").is_err());
    let geohash_area = geohash.decode();
    assert_within_meters(
        *geohash_area.value(),
        Coordinate::wgs84(42.6, -5.6)?,
        geohash_area.max_error().meters(),
    );

    let maidenhead = Maidenhead::encode(Coordinate::wgs84(40.5, -75.0)?, 2)?;
    assert_eq!(maidenhead.as_str(), "FN20");
    assert_eq!(Maidenhead::try_from("fn20")?, maidenhead);
    assert_eq!("FN20".parse::<Maidenhead>()?, maidenhead);
    assert_eq!(format!("{maidenhead}"), "FN20");
    // The encoders clamp the pole and the antimeridian: their output validates.
    let corner = Coordinate::wgs84(90.0, 179.999_999_999_999_97)?;
    let locator = Maidenhead::encode(corner, 3)?;
    assert_eq!(locator.as_str(), "RR99xx");
    assert_eq!(Maidenhead::try_from(locator.as_str())?, locator);
    let code = PlusCode::encode(corner, 15)?;
    assert_eq!(PlusCode::try_from(code.as_str())?, code);
    let maidenhead_area = maidenhead.decode();
    assert_within_meters(
        *maidenhead_area.value(),
        Coordinate::wgs84(40.5, -75.0)?,
        maidenhead_area.max_error().meters(),
    );

    let new_york = new_york();
    let h3 = H3Cell::encode(new_york, 9)?;
    assert_eq!(h3.as_u64(), 617_733_151_020_810_239);
    // A raw index is validated once, on the way in; decoding cannot fail.
    assert_eq!(H3Cell::new(617_733_151_020_810_239)?, h3);
    let h3_area = h3.decode();
    assert!((205.0..206.0).contains(&h3_area.max_error().meters()));
    assert_within_meters(*h3_area.value(), new_york, h3_area.max_error().meters());
    assert!(
        H3Cell::encode(new_york, 12)?.decode().max_error().meters()
            < H3Cell::encode(new_york, 5)?.decode().max_error().meters()
    );

    let s2 = S2CellId::encode(new_york, 20)?;
    assert_eq!(s2.as_u64(), 9_926_595_630_970_437_632);
    assert_eq!(S2CellId::new(9_926_595_630_970_437_632)?, s2);
    let s2_area = s2.decode();
    assert!((6.0..7.0).contains(&s2_area.max_error().meters()));
    assert_within_meters(*s2_area.value(), new_york, s2_area.max_error().meters());
    assert!(
        S2CellId::encode(new_york, 25)?
            .decode()
            .max_error()
            .meters()
            < S2CellId::encode(new_york, 5)?.decode().max_error().meters()
    );

    Ok("encoded and global grids")
}
