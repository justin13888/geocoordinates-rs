//! Baidu Web Mercator — BD-09 lat/lon ↔ Baidu's projected meters.
//!
//! Baidu map tiles are addressed in a projected meter coordinate built **on top
//! of BD-09** (not WGS-84). The projection is a piecewise polynomial fit:
//! latitude / value bands ([`LLBAND`] / [`MCBAND`]) select one of six
//! coefficient rows in [`LL2MC`] (forward) or [`MC2LL`] (inverse).
//!
//! Both directions are deterministic, so they implement [`From`] and are modeled
//! as **exact** (like [`Utm`](crate::grids::Utm)). The two coefficient tables are
//! independent empirical fits, so a `from_bd09(p).to_bd09()` round-trip is stable
//! to sub-meter, not bit-identical.
//!
//! The geographic side is always [`Bd09`]; chain the [`china`](crate::china) datum
//! methods (BD-09 → GCJ-02 → WGS-84) to reach real GPS, which are honest about
//! being approximate inverses.

use super::Bd09;
use crate::coord::Coordinate;
use crate::error::{Error, Result};

/// Northing band edges (meters) selecting a [`MC2LL`] row, descending.
const MCBAND: [f64; 6] = [
    12_890_594.86,
    8_362_377.87,
    5_591_021.0,
    3_481_989.83,
    1_678_043.12,
    0.0,
];
/// Latitude band edges (degrees) selecting a [`LL2MC`] row, descending.
const LLBAND: [f64; 6] = [75.0, 60.0, 45.0, 30.0, 15.0, 0.0];

/// Inverse (meters → BD-09) coefficient rows, one per [`MCBAND`] band.
const MC2LL: [[f64; 10]; 6] = [
    [
        1.410_526_172_116_255e-8,
        0.000_008_983_055_096_488_72,
        -1.993_983_381_633_1,
        200.982_438_310_679_6,
        -187.240_370_381_554_7,
        91.608_751_666_984_3,
        -23.387_656_496_033_39,
        2.571_213_172_961_98,
        -0.038_010_033_086_53,
        17_337_981.2,
    ],
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
const LL2MC: [[f64; 10]; 6] = [
    [
        -0.001_570_210_244_4,
        111_320.702_061_693_9,
        1_704_480_524_535_203.0,
        -10_338_987_376_042_340.0,
        26_112_667_856_603_880.0,
        -35_149_669_176_653_700.0,
        26_595_700_718_403_920.0,
        -10_725_012_454_188_240.0,
        1_800_819_912_950_474.0,
        82.5,
    ],
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
/// tables work in both hemispheres) falls into.
fn lookup(
    bands: &[f64; 6],
    table: &'static [[f64; 10]; 6],
    value: f64,
) -> Option<&'static [f64; 10]> {
    let v = value.abs();
    (0..6).find(|&i| v >= bands[i]).map(|i| &table[i])
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
/// underlying geographic datum is **BD-09** — see [`BaiduMercator::to_bd09`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BaiduMercator {
    /// Easting (longitude axis), in meters.
    pub x: f64,
    /// Northing (latitude axis), in meters.
    pub y: f64,
}

impl BaiduMercator {
    /// Construct from easting/northing in meters.
    #[must_use]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Baidu Web Mercator → BD-09 lat/lon (exact inverse projection, `MC2LL`).
    #[must_use]
    pub fn to_bd09(self) -> Bd09 {
        let Some(row) = lookup(&MCBAND, &MC2LL, self.y) else {
            return Bd09::new(f64::NAN, f64::NAN);
        };
        let lon = row[0] + row[1] * self.x.abs();
        let lat = poly(&row[2..9], self.y.abs() / row[9]);
        Bd09::new(lat.copysign(self.y), lon.copysign(self.x))
    }

    /// BD-09 lat/lon → Baidu Web Mercator (exact forward projection, `LL2MC`).
    #[must_use]
    pub fn from_bd09(p: Bd09) -> Self {
        let Some(row) = lookup(&LLBAND, &LL2MC, p.lat) else {
            return Self::new(f64::NAN, f64::NAN);
        };
        let x = row[0] + row[1] * p.lon.abs();
        let y = poly(&row[2..9], p.lat.abs() / row[9]);
        Self::new(x.copysign(p.lon), y.copysign(p.lat))
    }

    /// Baidu Web Mercator → canonical [`Coordinate`], tagged [`Crs::Bd09`](crate::Crs)
    /// (exact). Height is left unset — the projection is 2-D.
    #[must_use]
    pub fn to_coordinate(self) -> Coordinate {
        self.to_bd09().into()
    }

    /// Canonical [`Coordinate`] → Baidu Web Mercator.
    ///
    /// # Errors
    /// Returns [`Error::CrsMismatch`] unless `coord.crs` is [`Crs::Bd09`](crate::Crs) — a
    /// non-BD-09 coordinate must be converted to BD-09 first, never silently
    /// reprojected.
    pub fn try_from_coordinate(coord: Coordinate) -> Result<Self> {
        Ok(Self::from_bd09(Bd09::try_from(coord)?))
    }
}

impl From<Bd09> for BaiduMercator {
    /// Exact forward projection.
    fn from(p: Bd09) -> Self {
        Self::from_bd09(p)
    }
}

impl From<BaiduMercator> for Bd09 {
    /// Exact inverse projection.
    fn from(m: BaiduMercator) -> Self {
        m.to_bd09()
    }
}

impl From<BaiduMercator> for Coordinate {
    /// Exact; tags the result [`Crs::Bd09`](crate::Crs).
    fn from(m: BaiduMercator) -> Self {
        m.to_coordinate()
    }
}

impl TryFrom<Coordinate> for BaiduMercator {
    type Error = Error;

    /// Fails with [`Error::CrsMismatch`] unless `coord.crs` is [`Crs::Bd09`](crate::Crs).
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

    #[test]
    fn mercator_to_bd09_matches_reference() {
        let bd = BaiduMercator::new(REF_XY.0, REF_XY.1).to_bd09();
        assert_close(bd.lat, REF_BD09_LAT, 1e-5);
        assert_close(bd.lon, REF_BD09_LON, 1e-5);
    }

    #[test]
    fn mercator_round_trip_sub_meter() {
        let m = BaiduMercator::new(REF_XY.0, REF_XY.1);
        let back = BaiduMercator::from_bd09(m.to_bd09());
        // Independent forward/inverse fits: stable to well under a meter.
        assert_close(back.x, m.x, 0.5);
        assert_close(back.y, m.y, 0.5);
    }

    #[test]
    fn coordinate_bridge_round_trips_and_checks_crs() {
        let m = BaiduMercator::new(REF_XY.0, REF_XY.1);
        let coord = m.to_coordinate();
        assert_eq!(coord.crs, Crs::Bd09);
        // A non-BD-09 coordinate must be rejected, not silently reprojected.
        assert!(BaiduMercator::try_from(Coordinate::wgs84(REF_BD09_LAT, REF_BD09_LON)).is_err());
    }
}
