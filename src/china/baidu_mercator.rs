//! Baidu Web Mercator — BD-09 lat/lon ↔ Baidu's projected meters.
//!
//! Baidu map tiles are addressed in a projected meter coordinate built **on top
//! of BD-09** (not WGS-84). The projection is a piecewise polynomial fit:
//! latitude / value bands ([`LLBAND`] / [`MCBAND`]) select one of five
//! coefficient rows in [`LL2MC`] (forward) or [`MC2LL`] (inverse).
//!
//! Each direction reproduces Baidu's own published formula deterministically,
//! so it returns a bare type rather than [`Approx`](crate::Approx), and remains
//! fallible because its projection domain is validated: like Baidu's own
//! `convertLL2MC`, it is defined only within ±74° of latitude. The two
//! directions are **not** exact inverses of each other: the coefficient tables
//! are independent empirical fits. A `try_from_bd09(p)?.try_to_bd09()?` round
//! trip drifts by up to about 0.25 m below 60° latitude (all of China), and by
//! up to about 7 m in the 60°–74° band.
//!
//! ```
//! use geocoordinates::{BaiduMercator, Bd09};
//!
//! let m = BaiduMercator::try_from_bd09(Bd09::new(39.9, 116.4)?)?;
//! let back = m.try_to_bd09()?;
//! assert!((back.lat() - 39.9).abs() < 1e-5);
//! assert!(BaiduMercator::try_from_bd09(Bd09::new(80.0, 0.0)?).is_err());
//! # Ok::<(), geocoordinates::Error>(())
//! ```
//!
//! The geographic side is always [`Bd09`]; chain the [`china`](crate::china) datum
//! methods (BD-09 → GCJ-02 → WGS-84) to reach real GPS, which are honest about
//! being approximate inverses.

use super::Bd09;
use crate::coord::Coordinate;
use crate::error::{Error, Result};

/// Largest latitude magnitude (degrees) Baidu's projection accepts: its own
/// `convertLL2MC` clamps latitude to ±74°.
const MAX_LAT_DEG: f64 = 74.0;
/// Largest northing magnitude (meters): the forward image of [`MAX_LAT_DEG`].
const MAX_NORTHING_M: f64 = 12_474_104.174_141_884;

/// Lower northing edges (meters) of the first four [`MC2LL`] rows, descending;
/// the last row covers everything below the last edge. Baidu publishes a
/// further band from 12 890 594.86 m (75°), which lies beyond
/// [`MAX_NORTHING_M`] and so is omitted with its row.
const MCBAND: [f64; 4] = [8_362_377.87, 5_591_021.0, 3_481_989.83, 1_678_043.12];
/// Lower latitude edges (degrees) of the first four [`LL2MC`] rows,
/// descending; the last row covers everything below 15°. The published 75°
/// band lies beyond [`MAX_LAT_DEG`] and is omitted with its row.
const LLBAND: [f64; 4] = [60.0, 45.0, 30.0, 15.0];

/// Inverse (meters → BD-09) coefficient rows, one per [`MCBAND`] band.
const MC2LL: [[f64; 10]; 5] = [
    [
        -7.435_856_389_565_537e-9,
        0.000_008_983_055_097_726_239,
        -0.786_252_018_862_89,
        96.326_875_997_598_46,
        -1.852_047_575_298_26,
        -59.369_359_054_858_77,
        47.400_335_492_967_37,
        -16.507_419_310_638_87,
        2.287_866_746_993_75,
        10_260_144.86,
    ],
    [
        -3.030_883_460_898_826e-8,
        0.000_008_983_055_099_835_78,
        0.300_713_162_876_16,
        59.742_936_184_422_77,
        7.357_984_074_871,
        -25.383_710_026_647_45,
        13.453_805_211_109_08,
        -3.298_837_672_355_84,
        0.327_109_053_634_75,
        6_856_817.37,
    ],
    [
        -1.981_981_304_930_552e-8,
        0.000_008_983_055_099_779_535,
        0.032_781_828_525_91,
        40.316_785_277_057_44,
        0.656_592_986_772_77,
        -4.442_555_344_774_92,
        0.853_419_118_052_63,
        0.129_233_479_982_04,
        -0.046_257_360_075_61,
        4_482_777.06,
    ],
    [
        3.091_913_710_684_37e-9,
        0.000_008_983_055_096_812_155,
        0.000_069_957_240_62,
        23.109_343_041_449_01,
        -0.000_236_634_905_11,
        -0.632_181_781_024_2,
        -0.006_634_944_672_73,
        0.034_300_823_979_53,
        -0.004_660_438_763_32,
        2_555_164.4,
    ],
    [
        2.890_871_144_776_878e-9,
        0.000_008_983_055_095_805_407,
        -3.068_298e-8,
        7.471_370_254_680_32,
        -0.000_003_539_379_94,
        -0.021_451_448_610_37,
        -0.000_012_344_265_96,
        0.000_103_229_527_73,
        -0.000_003_238_903_64,
        826_088.5,
    ],
];

/// Forward (BD-09 → meters) coefficient rows, one per [`LLBAND`] band.
const LL2MC: [[f64; 10]; 5] = [
    [
        0.000_827_782_451_617_252_6,
        111_320.702_046_357_8,
        647_795_574.667_160_7,
        -4_082_003_173.641_316,
        10_774_905_663.511_42,
        -15_171_875_531.515_59,
        12_053_065_338.621_67,
        -5_124_939_663.577_472,
        913_311_935.951_203_2,
        67.5,
    ],
    [
        0.003_373_987_667_65,
        111_320.702_020_216_2,
        4_481_351.045_890_365,
        -23_393_751.199_316_62,
        79_682_215.471_864_55,
        -115_964_993.279_725_3,
        97_236_711.156_021_45,
        -43_661_946.337_528_21,
        8_477_230.501_135_234,
        52.5,
    ],
    [
        0.002_206_364_962_08,
        111_320.702_020_912_8,
        51_751.861_128_411_31,
        3_796_837.749_470_245,
        992_013.739_779_101_3,
        -1_221_952.217_112_87,
        1_340_652.697_009_075,
        -620_943.699_098_431_2,
        144_416.929_380_624_1,
        37.5,
    ],
    [
        -0.000_344_196_350_436_839_2,
        111_320.702_057_685_6,
        278.235_398_077_275_2,
        2_485_758.690_035_394,
        6_070.750_963_243_378,
        54_821.183_453_521_18,
        9_540.606_633_304_236,
        -2_710.553_267_466_45,
        1_405.483_844_121_726,
        22.5,
    ],
    [
        -0.000_321_813_587_861_313_2,
        111_320.702_070_161_5,
        0.003_693_834_312_89,
        823_725.640_279_571_8,
        0.461_049_869_090_93,
        2_351.343_141_331_292,
        1.580_607_842_981_99,
        8.777_385_890_782_84,
        0.372_388_842_524_24,
        7.45,
    ],
];

/// Select the coefficient row whose band `value` (compared by magnitude, so the
/// tables work in both hemispheres) falls into: the first row whose lower edge
/// it reaches, or the last row below every edge. Total, so it never fails.
fn lookup(bands: &[f64; 4], table: &'static [[f64; 10]; 5], value: f64) -> &'static [f64; 10] {
    let v = value.abs();
    &table[bands.iter().take_while(|&&edge| v < edge).count()]
}

/// Evaluate the degree-6 polynomial `factors[0] + factors[1]·x + … + factors[6]·x⁶`.
fn poly(factors: &[f64], x: f64) -> f64 {
    factors
        .iter()
        .enumerate()
        .map(|(i, f)| f * x.powi(i as i32))
        .sum()
}

/// A Baidu Web Mercator position, in projected meters.
///
/// `x` is the easting (longitude axis), `y` the northing (latitude axis). The
/// underlying geographic datum is **BD-09** — see
/// [`BaiduMercator::try_to_bd09`].
///
/// Both components are finite by construction ([`BaiduMercator::new`]);
/// whether they lie inside the projection's domain is checked by
/// [`BaiduMercator::try_to_bd09`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "BaiduMercatorRecord"))]
pub struct BaiduMercator {
    x: f64,
    y: f64,
}

/// Unvalidated serde shape of [`BaiduMercator`]; deserialization goes through
/// [`BaiduMercator::new`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct BaiduMercatorRecord {
    x: f64,
    y: f64,
}

#[cfg(feature = "serde")]
impl TryFrom<BaiduMercatorRecord> for BaiduMercator {
    type Error = Error;

    fn try_from(r: BaiduMercatorRecord) -> Result<Self> {
        Self::new(r.x, r.y)
    }
}

/// Reject a non-finite projected component, naming it.
fn finite(field: &'static str, value: f64) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Error::InvalidValue {
            field,
            detail: "must be finite".into(),
        })
    }
}

impl BaiduMercator {
    /// Construct from easting/northing in meters.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidValue`] naming `"x"` or `"y"` when that
    /// component is not finite.
    ///
    /// ```
    /// use geocoordinates::BaiduMercator;
    ///
    /// let m = BaiduMercator::new(12_949_156.8, 4_817_450.0)?;
    /// assert_eq!(m.y(), 4_817_450.0);
    /// assert!(BaiduMercator::new(f64::NAN, 0.0).is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(x: f64, y: f64) -> Result<Self> {
        finite("x", x)?;
        finite("y", y)?;
        Ok(Self { x, y })
    }

    /// Easting (longitude axis), in meters.
    #[must_use]
    pub fn x(&self) -> f64 {
        self.x
    }

    /// Northing (latitude axis), in meters.
    #[must_use]
    pub fn y(&self) -> f64 {
        self.y
    }

    /// Baidu Web Mercator → BD-09 lat/lon by Baidu's `MC2LL` polynomial fit.
    ///
    /// Not an exact inverse of [`BaiduMercator::try_from_bd09`]: the two are
    /// independent fits, and a round trip drifts by up to about 0.25 m below
    /// 60° latitude and up to about 7 m in the 60°–74° band.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidValue`] naming `"y"` when the northing is
    /// beyond the forward image of ±74° (about ±12 474 104 m), or `"x"` when
    /// the easting maps past ±180° of longitude.
    pub fn try_to_bd09(self) -> Result<Bd09> {
        if self.y.abs() > MAX_NORTHING_M {
            return Err(Error::InvalidValue {
                field: "y",
                detail: format!("northing must be within ±{MAX_NORTHING_M} m (latitude ±74°)"),
            });
        }
        let row = lookup(&MCBAND, &MC2LL, self.y);
        let lon = row[0] + row[1] * self.x.abs();
        if lon > 180.0 {
            return Err(Error::InvalidValue {
                field: "x",
                detail: "easting maps past ±180° of longitude".into(),
            });
        }
        let lat = poly(&row[2..9], self.y.abs() / row[9]);
        Ok(Bd09::from_valid(lat.copysign(self.y), lon.copysign(self.x)))
    }

    /// BD-09 lat/lon → Baidu Web Mercator by Baidu's `LL2MC` polynomial fit.
    ///
    /// Not an exact inverse of [`BaiduMercator::try_to_bd09`]: the two are
    /// independent fits, and a round trip drifts by up to about 0.25 m below
    /// 60° latitude and up to about 7 m in the 60°–74° band.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OutsideDomain`] (operation `"Baidu Mercator"`) when
    /// the latitude is beyond ±74°, where Baidu's projection is not defined.
    pub fn try_from_bd09(p: Bd09) -> Result<Self> {
        if p.lat().abs() > MAX_LAT_DEG {
            return Err(Error::OutsideDomain {
                operation: "Baidu Mercator",
                lat: p.lat(),
                lon: p.lon(),
            });
        }
        let row = lookup(&LLBAND, &LL2MC, p.lat());
        let x = row[0] + row[1] * p.lon().abs();
        let y = poly(&row[2..9], p.lat().abs() / row[9]);
        Ok(Self {
            x: x.copysign(p.lon()),
            y: y.copysign(p.lat()),
        })
    }

    /// Baidu Web Mercator → canonical [`Coordinate`], tagged [`Crs::Bd09`](crate::Crs),
    /// via [`BaiduMercator::try_to_bd09`]. Height is left unset — the
    /// projection is 2-D.
    ///
    /// # Errors
    ///
    /// As [`BaiduMercator::try_to_bd09`].
    pub fn try_to_coordinate(self) -> Result<Coordinate> {
        Ok(self.try_to_bd09()?.to_coordinate())
    }

    /// Canonical [`Coordinate`] → Baidu Web Mercator.
    ///
    /// # Errors
    /// Returns [`Error::CrsMismatch`] unless `coord.crs()` is [`Crs::Bd09`](crate::Crs) — a
    /// non-BD-09 coordinate must be converted to BD-09 first, never silently
    /// reprojected — and otherwise as [`BaiduMercator::try_from_bd09`].
    pub fn try_from_coordinate(coord: Coordinate) -> Result<Self> {
        Self::try_from_bd09(Bd09::try_from_coordinate(coord)?)
    }
}

impl TryFrom<Bd09> for BaiduMercator {
    type Error = Error;

    /// Forward projection. Thin sugar over [`BaiduMercator::try_from_bd09`].
    fn try_from(p: Bd09) -> Result<Self> {
        Self::try_from_bd09(p)
    }
}

impl TryFrom<BaiduMercator> for Bd09 {
    type Error = Error;

    /// Inverse projection. Thin sugar over [`BaiduMercator::try_to_bd09`].
    fn try_from(m: BaiduMercator) -> Result<Self> {
        m.try_to_bd09()
    }
}

impl TryFrom<Coordinate> for BaiduMercator {
    type Error = Error;

    /// Fails with [`Error::CrsMismatch`] unless `coord.crs()` is [`Crs::Bd09`](crate::Crs).
    fn try_from(coord: Coordinate) -> Result<Self> {
        Self::try_from_coordinate(coord)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Crs;
    use crate::test_support::assert_close;

    // Reference point from undrift_gps `baidu_mercator.rs` (MIT).
    const REF_XY: (f64, f64) = (12_949_156.80, 4_817_450.00);
    const REF_BD09_LAT: f64 = 39.856_357;
    const REF_BD09_LON: f64 = 116.322_989;

    fn merc(x: f64, y: f64) -> BaiduMercator {
        BaiduMercator::new(x, y).unwrap()
    }

    fn bd(lat: f64, lon: f64) -> Bd09 {
        Bd09::new(lat, lon).unwrap()
    }

    #[test]
    fn mercator_to_bd09_matches_reference() {
        let b = merc(REF_XY.0, REF_XY.1).try_to_bd09().unwrap();
        assert_close(b.lat(), REF_BD09_LAT, 1e-5);
        assert_close(b.lon(), REF_BD09_LON, 1e-5);
        // The southern / western mirror is the same fit with the signs flipped.
        let mirrored = merc(-REF_XY.0, -REF_XY.1).try_to_bd09().unwrap();
        assert_eq!((mirrored.lat(), mirrored.lon()), (-b.lat(), -b.lon()));
    }

    #[test]
    fn mercator_round_trip_sub_meter() {
        let m = merc(REF_XY.0, REF_XY.1);
        let back = BaiduMercator::try_from_bd09(m.try_to_bd09().unwrap()).unwrap();
        // Independent forward/inverse fits: stable to well under a meter.
        assert_close(back.x(), m.x(), 0.5);
        assert_close(back.y(), m.y(), 0.5);
        assert_eq!(
            BaiduMercator::try_from(m.try_to_bd09().unwrap()).unwrap(),
            back
        );
        assert_eq!(Bd09::try_from(m).unwrap(), m.try_to_bd09().unwrap());
    }

    #[test]
    fn bd09_round_trip_drift_matches_the_documented_bands() {
        // Worst ground drift of BD-09 → Mercator → BD-09 over a latitude band,
        // sampled every 0.1° of latitude and 1° of longitude (70°E–140°E).
        let worst_m = |from: u32, to: u32| {
            (from..to)
                .flat_map(|i| (70..140).map(move |lon| (f64::from(i) * 0.1, f64::from(lon))))
                .map(|(lat, lon)| {
                    let m = BaiduMercator::try_from_bd09(bd(lat, lon)).unwrap();
                    let b = m.try_to_bd09().unwrap();
                    let d_north = (b.lat() - lat).to_radians();
                    let d_east = (b.lon() - lon).to_radians() * lat.to_radians().cos();
                    d_north.hypot(d_east) * 6_378_137.0
                })
                .fold(0.0_f64, f64::max)
        };
        let low = worst_m(1, 600);
        assert!(low < 0.3, "below 60°: {low} m");
        let high = worst_m(600, 741);
        assert!((1.0..8.0).contains(&high), "60°–74°: {high} m");
    }

    #[test]
    fn coordinate_bridge_round_trips_and_checks_crs() {
        let m = merc(REF_XY.0, REF_XY.1);
        let coord = m.try_to_coordinate().unwrap();
        assert_eq!(coord.crs(), Crs::Bd09);
        assert_eq!(
            BaiduMercator::try_from_coordinate(coord).unwrap(),
            BaiduMercator::try_from(coord).unwrap()
        );
        // A non-BD-09 coordinate must be rejected, not silently reprojected.
        let wgs = Coordinate::wgs84(REF_BD09_LAT, REF_BD09_LON).unwrap();
        assert!(matches!(
            BaiduMercator::try_from(wgs),
            Err(Error::CrsMismatch { .. })
        ));
    }

    /// C17: Baidu clamps its projection to ±74°, so a latitude beyond it is
    /// outside the domain rather than silently stretched.
    #[test]
    fn latitudes_beyond_74_degrees_are_outside_the_domain() {
        for lat in [74.000_001, 74.5, 80.0, -75.0, 90.0] {
            assert!(
                matches!(
                    BaiduMercator::try_from_bd09(bd(lat, 10.0)),
                    Err(Error::OutsideDomain {
                        operation: "Baidu Mercator",
                        ..
                    })
                ),
                "lat {lat}"
            );
        }
        // 74° itself is inside, and lands exactly on the northing limit.
        let edge = BaiduMercator::try_from_bd09(bd(-74.0, 10.0)).unwrap();
        assert_close(edge.y(), -MAX_NORTHING_M, 1e-6);
        assert!(edge.try_to_bd09().is_ok());
    }

    /// The inverse rejects a northing beyond the 74° limit and an easting that
    /// maps past the antimeridian, naming the component.
    #[test]
    fn inverse_rejects_values_outside_the_projection() {
        let invalid_field = |m: BaiduMercator| match m.try_to_bd09() {
            Err(Error::InvalidValue { field, .. }) => field,
            other => panic!("expected InvalidValue, got {other:?}"),
        };
        assert_eq!(invalid_field(merc(0.0, MAX_NORTHING_M + 1.0)), "y");
        assert_eq!(invalid_field(merc(0.0, -MAX_NORTHING_M - 1.0)), "y");
        assert_eq!(invalid_field(merc(20_100_000.0, 0.0)), "x");
        assert_eq!(invalid_field(merc(-20_100_000.0, 0.0)), "x");
        // The limits themselves are inside.
        assert!(merc(0.0, MAX_NORTHING_M).try_to_bd09().is_ok());
        let east = merc(20_037_000.0, 0.0).try_to_bd09().unwrap();
        assert!(east.lon() > 179.99 && east.lon() <= 180.0, "{}", east.lon());
    }

    #[test]
    fn construction_rejects_non_finite_components() {
        let field = |r: Result<BaiduMercator>| match r {
            Err(Error::InvalidValue { field, .. }) => field,
            other => panic!("expected InvalidValue, got {other:?}"),
        };
        assert_eq!(field(BaiduMercator::new(f64::NAN, 0.0)), "x");
        assert_eq!(field(BaiduMercator::new(0.0, f64::INFINITY)), "y");
    }

    /// Each band edge selects the row above it, and anything below the last
    /// edge the last row.
    #[test]
    fn lookup_selects_the_band_by_its_lower_edge() {
        assert!(std::ptr::eq(lookup(&LLBAND, &LL2MC, 60.0), &LL2MC[0]));
        assert!(std::ptr::eq(lookup(&LLBAND, &LL2MC, 59.999), &LL2MC[1]));
        assert!(std::ptr::eq(lookup(&LLBAND, &LL2MC, -15.0), &LL2MC[3]));
        assert!(std::ptr::eq(lookup(&LLBAND, &LL2MC, 14.999), &LL2MC[4]));
        assert!(std::ptr::eq(
            lookup(&MCBAND, &MC2LL, 8_362_377.87),
            &MC2LL[0]
        ));
        assert!(std::ptr::eq(lookup(&MCBAND, &MC2LL, 0.0), &MC2LL[4]));
    }

    /// The northing limit is the forward image of 74°, to the last digit.
    #[test]
    fn northing_limit_is_the_image_of_74_degrees() {
        let m = BaiduMercator::try_from_bd09(bd(MAX_LAT_DEG, 0.0)).unwrap();
        assert_close(m.y(), MAX_NORTHING_M, 1e-6);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trips_through_the_record() {
        let m = merc(REF_XY.0, REF_XY.1);
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, r#"{"x":12949156.8,"y":4817450.0}"#);
        assert_eq!(serde_json::from_str::<BaiduMercator>(&json).unwrap(), m);
        // JSON cannot carry a non-finite number, so validation is exercised
        // directly on the record conversion.
        let bad = BaiduMercatorRecord {
            x: f64::NAN,
            y: 0.0,
        };
        assert!(BaiduMercator::try_from(bad).is_err());
    }
}
