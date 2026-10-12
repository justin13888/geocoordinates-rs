//! China datums (GCJ-02, BD-09) and CRS dispatch through `convert`.

use geocoordinates::china::out_of_china;
use geocoordinates::convert::{can_convert, convert};
use geocoordinates::{BaiduMercator, Bd09, Coordinate, Crs, Gcj02, Height, Wgs84};

use crate::support::{DemoResult, assert_close, assert_mercator_close, assert_within_meters};

pub(crate) fn china_datums_and_dispatch() -> DemoResult {
    const WGS: Wgs84 = Wgs84 {
        lat: 39.915,
        lon: 116.404,
    };
    const EXPECTED_GCJ: Gcj02 = Gcj02 {
        lat: 39.916_404_281_501_64,
        lon: 116.410_244_499_169_38,
    };
    const EXPECTED_BD: Bd09 = Bd09 {
        lat: 39.922_699_552_216_216,
        lon: 116.416_627_243_787_33,
    };

    let gcj = WGS.try_to_gcj02()?;
    assert_eq!(Wgs84::new(WGS.lat, WGS.lon), WGS);
    assert_eq!(Coordinate::gcj02(gcj.lat, gcj.lon)?.crs(), Crs::Gcj02);
    assert_close(gcj.lat, EXPECTED_GCJ.lat, 1e-9);
    assert_close(gcj.lon, EXPECTED_GCJ.lon, 1e-9);
    let gcj_from = Gcj02::try_from(WGS)?;
    assert_eq!(gcj_from, gcj);
    let bd = WGS.try_to_bd09()?;
    assert_eq!(Bd09::new(bd.lat, bd.lon), bd);
    assert_eq!(Coordinate::bd09(bd.lat, bd.lon)?.crs(), Crs::Bd09);
    assert_close(bd.lat, EXPECTED_BD.lat, 1e-9);
    assert_close(bd.lon, EXPECTED_BD.lon, 1e-9);
    let bd_from_gcj = Bd09::try_from(gcj)?;
    assert_eq!(bd_from_gcj, gcj.try_to_bd09()?);

    let fast_wgs = gcj.try_to_wgs84_fast()?;
    let refined_wgs = gcj.try_to_wgs84_refined()?;
    assert!(fast_wgs.max_error_m() > refined_wgs.max_error_m());
    assert_within_meters(&*fast_wgs, &WGS, fast_wgs.max_error_m());
    assert_within_meters(&*refined_wgs, &WGS, refined_wgs.max_error_m());
    let fast_gcj = bd.try_to_gcj02_fast()?;
    let refined_gcj = bd.try_to_gcj02_refined()?;
    assert_within_meters(&*fast_gcj, &gcj, fast_gcj.max_error_m());
    assert_within_meters(&*refined_gcj, &gcj, refined_gcj.max_error_m());
    let refined_from_bd = bd.try_to_wgs84_refined()?;
    assert_within_meters(&*refined_from_bd, &WGS, refined_from_bd.max_error_m());

    let mercator = BaiduMercator::try_from_bd09(bd)?;
    assert_eq!(BaiduMercator::new(mercator.x, mercator.y), mercator);
    let mercator_from = BaiduMercator::try_from(bd)?;
    assert_eq!(mercator_from, mercator);
    assert_within_meters(&mercator.try_to_bd09()?, &bd, 1.0);
    let tagged = mercator.try_to_coordinate()?;
    assert_eq!(tagged.crs(), Crs::Bd09);
    assert_mercator_close(BaiduMercator::try_from_coordinate(tagged)?, mercator, 0.02);
    assert_mercator_close(BaiduMercator::try_from(tagged)?, mercator, 0.02);

    let london = Wgs84::new(51.5074, -0.1278);
    assert!(out_of_china(london.lat, london.lon));
    assert_eq!(london.try_to_gcj02()?, Gcj02::new(london.lat, london.lon));

    let systems = [
        Crs::Wgs84,
        Crs::Gcj02,
        Crs::Bd09,
        Crs::Nad27,
        Crs::Tokyo,
        Crs::Pulkovo42,
    ];
    for from in systems {
        let source =
            Coordinate::new(39.915, 116.404, from)?.with_height(Height::Ellipsoidal(12.0))?;
        for to in systems {
            assert!(can_convert(from, to));
            let converted = convert(source, to)?;
            assert_eq!(converted.crs(), to);
            assert!(converted.lat().is_finite() && converted.lon().is_finite());
            assert!(converted.height().is_some());
            let is_approximate = (from == Crs::Gcj02 && to != Crs::Gcj02 && to != Crs::Bd09)
                || (from == Crs::Bd09 && to != Crs::Bd09);
            if is_approximate {
                assert!(converted.max_error_m() > 0.0);
            } else {
                assert_eq!(converted.max_error_m(), 0.0);
            }
        }
    }

    Ok("China datums and runtime dispatch")
}
