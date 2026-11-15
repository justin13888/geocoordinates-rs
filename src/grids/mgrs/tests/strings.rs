//! The string conversions in `strings.rs`.

use super::*;

#[test]
fn as_ref_is_the_display_string_at_every_precision() {
    let precisions = [
        MgrsPrecision::Km100,
        MgrsPrecision::Km10,
        MgrsPrecision::Km1,
        MgrsPrecision::M100,
        MgrsPrecision::M10,
        MgrsPrecision::M1,
    ];
    for precision in precisions {
        let m = Mgrs::try_from_coordinate(c(40.0, -75.0), precision).unwrap();
        let s: &str = m.as_ref();
        assert_eq!(s, m.to_string(), "{precision:?}");
        assert_eq!(s, m.as_str(), "{precision:?}");
    }
}
