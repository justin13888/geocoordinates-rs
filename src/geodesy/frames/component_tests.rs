//! Per-component validation of the local-frame offsets.

use super::*;

/// Each ENU component is checked on its own: one non-finite component is
/// rejected as an `"ENU"` error before it can reach the ECEF step, which
/// would otherwise report it under its own field.
#[test]
fn each_non_finite_enu_component_is_rejected_as_enu() {
    let origin = Coordinate::wgs84(40.0, -75.0).unwrap();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for enu in [
            Enu {
                east_m: bad,
                north_m: 0.0,
                up_m: 0.0,
            },
            Enu {
                east_m: 0.0,
                north_m: bad,
                up_m: 0.0,
            },
            Enu {
                east_m: 0.0,
                north_m: 0.0,
                up_m: bad,
            },
        ] {
            assert!(
                matches!(
                    enu.try_to_coordinate(origin),
                    Err(Error::InvalidValue { field: "ENU", .. })
                ),
                "{enu:?}"
            );
        }
    }
}
