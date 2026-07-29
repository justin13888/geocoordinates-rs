//! Shared test-only helpers and reference vectors.
//!
//! Compiled only under `cfg(test)`, so nothing here is part of the public API.
//!
//! Assertions are expressed in **meters**, via the library's own
//! [`haversine_distance`](crate::geodesy::haversine_distance), so a tolerance
//! reads as a physical bound rather than an opaque float epsilon. The reference
//! vectors are transcribed from the permissively-licensed China-datum libraries
//! — eviltransform (BSD-2-Clause), coordtransform-rs (MIT/Apache-2.0), and
//! undrift_gps (MIT) — never from PRCoords (GPL).

// Shared test scaffolding: not every helper / field is exercised by the current
// (trimmed) test set, and more are used as later releases land. See ROADMAP.md.
#![allow(dead_code)]

use crate::coord::LatLon;
use crate::geodesy::haversine_distance;
use crate::{Bd09, Gcj02, Wgs84};

/// Assert that two positions are within `max_m` meters, measured by the
/// library's own [`haversine_distance`](crate::geodesy::haversine_distance).
///
/// Works on any [`LatLon`] — `Coordinate` and the datum newtypes alike.
#[track_caller]
pub(crate) fn assert_within_meters(a: &impl LatLon, b: &impl LatLon, max_m: f64) {
    let d = haversine_distance(a, b).unwrap().meters();
    assert!(
        d <= max_m,
        "expected within {max_m} m, got {d:.4} m \
         (a = {:.7},{:.7}  b = {:.7},{:.7})",
        a.lat(),
        a.lon(),
        b.lat(),
        b.lon(),
    );
}

/// Assert two scalar values (degrees or meters) agree within `eps` (absolute).
#[track_caller]
pub(crate) fn assert_close(actual: f64, expected: f64, eps: f64) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= eps,
        "expected {expected} ± {eps}, got {actual} (|Δ| = {diff})"
    );
}

/// A reference point with its representation in each China datum, in
/// `(lat, lon)` decimal-degree order. `bd09` is present only where the source
/// provides a BD-09 value for the *same* physical point.
pub(crate) struct DatumVector {
    /// Human-readable label / source location.
    pub name: &'static str,
    /// WGS-84 `(lat, lon)`.
    pub wgs84: (f64, f64),
    /// GCJ-02 `(lat, lon)` — the reference WGS-84 → GCJ-02 output.
    pub gcj02: (f64, f64),
    /// BD-09 `(lat, lon)` — the reference WGS-84 → BD-09 output, where known.
    pub bd09: Option<(f64, f64)>,
}

impl DatumVector {
    /// The WGS-84 point as a typed newtype.
    pub(crate) fn wgs84(&self) -> Wgs84 {
        Wgs84::new(self.wgs84.0, self.wgs84.1)
    }
    /// The GCJ-02 point as a typed newtype.
    pub(crate) fn gcj02(&self) -> Gcj02 {
        Gcj02::new(self.gcj02.0, self.gcj02.1)
    }
    /// The BD-09 point as a typed newtype, where the source provides one.
    pub(crate) fn bd09(&self) -> Option<Bd09> {
        self.bd09.map(|(lat, lon)| Bd09::new(lat, lon))
    }
}

/// Full-precision reference points (same physical location across datums).
///
/// - The first three are eviltransform's `TESTS` (WGS-84 → GCJ-02 pairs).
/// - The last is coordtransform-rs's test input `(116.404, 39.915)` treated as
///   WGS-84, with its `wgs84_to_gcj02` and `wgs84_to_bd09` outputs (a single
///   consistent triple).
pub(crate) const DATUM_VECTORS: &[DatumVector] = &[
    DatumVector {
        name: "Shanghai (eviltransform)",
        wgs84: (31.1774276, 121.5272106),
        gcj02: (31.175_303_983_645_97, 121.531_541_859_215),
        bd09: None,
    },
    DatumVector {
        name: "Shenzhen (eviltransform)",
        wgs84: (22.543847, 113.912316),
        gcj02: (22.540_796_131_694_766, 113.917_176_480_836_3),
        bd09: None,
    },
    DatumVector {
        name: "Beijing (eviltransform)",
        wgs84: (39.911954, 116.377817),
        gcj02: (39.913_345_455_360_69, 116.384_047_224_556_57),
        bd09: None,
    },
    DatumVector {
        name: "Beijing (coordtransform-rs)",
        wgs84: (39.915, 116.404),
        gcj02: (39.916_404_281_501_64, 116.410_244_499_169_38),
        bd09: Some((39.922_699_552_216_216, 116.416_627_243_787_33)),
    },
];

/// coordtransform-rs single-step closed-form outputs for input `(116.404,
/// 39.915)` interpreted in the named *source* datum, in `(lat, lon)` order.
/// These let us pin our closed-form transforms to exact reference values.
pub(crate) mod coordtransform {
    /// Input point shared by every coordtransform-rs test, as `(lat, lon)`.
    pub(crate) const INPUT: (f64, f64) = (39.915, 116.404);
    /// `gcj02_to_bd09(input)` → `(lat, lon)`.
    pub(crate) const GCJ_TO_BD: (f64, f64) = (39.921_336_993_510_21, 116.410_369_493_710_29);
    /// `bd09_to_gcj02(input)` → `(lat, lon)` (closed-form fast inverse).
    pub(crate) const BD_TO_GCJ: (f64, f64) = (39.908_656_739_576_31, 116.397_627_291_193_15);
    /// `gcj02_to_wgs84(input)` → `(lat, lon)` (the single-step *fast* inverse).
    pub(crate) const GCJ_TO_WGS_FAST: (f64, f64) = (39.913_595_718_498_36, 116.397_755_500_830_61);
}
