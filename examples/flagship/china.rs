//! China datums (GCJ-02, BD-09) and CRS dispatch through `convert`.

use geocoordinates::china::{BaiduMercator, Bd09, Gcj02, Wgs84};
use geocoordinates::convert::convert;
use geocoordinates::{Approx, Coordinate, Crs, Height, Length};

use crate::support::{DemoResult, assert_close, assert_mercator_close, assert_within_meters};

pub(crate) fn china_datums_and_dispatch() -> DemoResult {
    let wgs = Wgs84::new(39.915, 116.404)?;
    let expected_gcj = Gcj02::new(39.916_404_281_501_64, 116.410_244_499_169_38)?;
    let expected_bd = Bd09::new(39.922_699_552_216_216, 116.416_627_243_787_33)?;

    let gcj = wgs.to_gcj02();
    assert_eq!(Wgs84::new(wgs.lat(), wgs.lon())?, wgs);
    assert_eq!(gcj.to_coordinate().crs(), Crs::Gcj02);
    assert_eq!(Coordinate::from(gcj), gcj.to_coordinate());
    assert_eq!(Gcj02::try_from_coordinate(gcj.to_coordinate())?, gcj);
    assert_close(gcj.lat(), expected_gcj.lat(), 1e-9);
    assert_close(gcj.lon(), expected_gcj.lon(), 1e-9);
    assert_eq!(Gcj02::from(wgs), gcj);
    let bd = wgs.try_to_bd09()?;
    assert_eq!(Bd09::new(bd.lat(), bd.lon())?, bd);
    assert_eq!(bd.to_coordinate().crs(), Crs::Bd09);
    assert_close(bd.lat(), expected_bd.lat(), 1e-9);
    assert_close(bd.lon(), expected_bd.lon(), 1e-9);
    let bd_from_gcj = Bd09::try_from(gcj)?;
    assert_eq!(bd_from_gcj, gcj.try_to_bd09()?);
    assert_eq!(Bd09::try_from(wgs)?, bd);

    let fast_wgs = gcj.to_wgs84_fast();
    let refined_wgs = gcj.to_wgs84_refined();
    assert!(fast_wgs.max_error() > refined_wgs.max_error());
    assert_within_meters(*fast_wgs.value(), wgs, fast_wgs.max_error().meters());
    assert_within_meters(*refined_wgs.value(), wgs, refined_wgs.max_error().meters());
    let fast_gcj = bd.try_to_gcj02_fast()?;
    let refined_gcj = bd.try_to_gcj02_refined()?;
    assert_within_meters(*fast_gcj.value(), gcj, fast_gcj.max_error().meters());
    assert_within_meters(*refined_gcj.value(), gcj, refined_gcj.max_error().meters());
    let refined_from_bd = bd.try_to_wgs84_refined()?;
    assert_within_meters(
        *refined_from_bd.value(),
        wgs,
        refined_from_bd.max_error().meters(),
    );
    // A crate building its own approximation wraps it the same way, with a
    // validated bound.
    let rounded = Approx::new(wgs, Length::from_meters(0.1))?;
    assert_eq!(rounded.max_error().meters(), 0.1);
    assert!(Approx::new(wgs, Length::from_meters(-0.1)).is_err());

    // BD-09's nudge applies worldwide, wrapped at the antimeridian.
    let east_edge = Gcj02::new(0.0, 180.0)?.try_to_bd09()?;
    assert!(east_edge.lon() < -179.99);
    // Near the antimeridian the nudge is not one-to-one, so the refined
    // inverse finds *a* preimage: its forward image lands on the input.
    let back = east_edge.try_to_gcj02_refined()?;
    assert_within_meters(
        back.value().try_to_bd09()?,
        east_edge,
        back.max_error().meters(),
    );

    let mercator = BaiduMercator::try_from_bd09(bd)?;
    assert_eq!(
        BaiduMercator::new(mercator.x_m(), mercator.y_m())?,
        mercator
    );
    let mercator_from = BaiduMercator::try_from(bd)?;
    assert_eq!(mercator_from, mercator);
    assert_within_meters(mercator.try_to_bd09()?, bd, 1.0);
    let tagged = mercator.try_to_coordinate()?;
    assert_eq!(tagged.crs(), Crs::Bd09);
    assert_mercator_close(BaiduMercator::try_from_coordinate(tagged)?, mercator, 0.02);
    assert_mercator_close(BaiduMercator::try_from(tagged)?, mercator, 0.02);

    let london = Wgs84::new(51.5074, -0.1278)?;
    assert!(london.is_outside_china());
    assert!(london.to_gcj02().is_outside_china());
    assert_eq!(london.to_gcj02(), Gcj02::new(london.lat(), london.lon())?);

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
            let converted = convert(source, to)?;
            let point = converted.value();
            assert_eq!(point.crs(), to);
            assert!(point.lat().is_finite() && point.lon().is_finite());
            assert!(point.height().is_some());
            // Classic-datum (Helmert) legs carry their parameters' published
            // accuracy, so any route through one is approximate too.
            let classic = |crs| matches!(crs, Crs::Nad27 | Crs::Tokyo | Crs::Pulkovo42);
            let is_approximate = (from == Crs::Gcj02 && to != Crs::Gcj02 && to != Crs::Bd09)
                || (from == Crs::Bd09 && to != Crs::Bd09)
                || (from != to && (classic(from) || classic(to)));
            if is_approximate {
                assert!(converted.max_error() > Length::ZERO);
            } else {
                assert_eq!(converted.max_error(), Length::ZERO);
            }
        }
    }

    Ok("China datums and runtime dispatch")
}
