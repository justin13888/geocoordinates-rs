//! Axis-order resolution for the free-text parser: deciding which of two
//! parsed components is latitude, and how confident that decision is.

use super::super::AxisOrder;
use crate::angle::Axis;
use crate::error::{Error, Result};
use crate::fix::Confidence;

/// Confidence when both hemisphere letters are present (axis order is certain).
const CONFIDENCE_EXPLICIT: Confidence = Confidence::CERTAIN;
/// Confidence when one hemisphere letter, or the `|v| > 90` range rule,
/// determines the axis order.
const CONFIDENCE_RESOLVED: Confidence = Confidence::from_constant(0.9);
/// Confidence when both values are in latitude range and the order was assumed
/// from `default_axis_order`.
const CONFIDENCE_ASSUMED: Confidence = Confidence::from_constant(0.7);

/// One parsed component: an unsigned magnitude in degrees, its sign, and the
/// axis a hemisphere letter pinned it to (if any).
pub(super) struct Component {
    pub(super) magnitude: f64,
    pub(super) negative: bool,
    pub(super) axis: Option<Axis>,
}

impl Component {
    fn signed(&self) -> f64 {
        if self.negative {
            -self.magnitude
        } else {
            self.magnitude
        }
    }
}

/// The resolved lat/lon plus how the axis order was decided.
pub(super) struct Resolved {
    pub(super) lat: f64,
    pub(super) lon: f64,
    pub(super) axis_order: Option<AxisOrder>,
    pub(super) confidence: Confidence,
    pub(super) notes: Vec<String>,
}

/// Resolve which component is latitude vs longitude, recording how it was
/// decided and a confidence.
pub(super) fn resolve(a: Component, b: Component, default_order: AxisOrder) -> Result<Resolved> {
    let a_val = a.signed();
    let b_val = b.signed();

    // Both axes pinned by hemisphere letters.
    if let (Some(ax), Some(bx)) = (a.axis, b.axis) {
        return match (ax, bx) {
            (Axis::Latitude, Axis::Longitude) => Ok(explicit(a_val, b_val)),
            (Axis::Longitude, Axis::Latitude) => Ok(explicit(b_val, a_val)),
            _ => Err(Error::Parse(
                "two hemisphere letters name the same axis".to_string(),
            )),
        };
    }

    // Exactly one axis pinned by a hemisphere letter; the other is its complement.
    match (a.axis, b.axis) {
        (Some(Axis::Latitude), None) | (None, Some(Axis::Longitude)) => {
            return Ok(one_letter(a_val, b_val));
        }
        (Some(Axis::Longitude), None) | (None, Some(Axis::Latitude)) => {
            return Ok(one_letter(b_val, a_val));
        }
        _ => {}
    }

    // No hemisphere letters: range rule, then the configured default.
    let a_lon_only = a_val.abs() > 90.0;
    let b_lon_only = b_val.abs() > 90.0;
    match (a_lon_only, b_lon_only) {
        (true, true) => Err(Error::Parse(
            "both components exceed the latitude range".to_string(),
        )),
        (true, false) => Ok(range_forced(b_val, a_val, AxisOrder::LonLat)),
        (false, true) => Ok(range_forced(a_val, b_val, AxisOrder::LatLon)),
        (false, false) => Ok(match default_order {
            AxisOrder::LatLon => assumed(a_val, b_val, AxisOrder::LatLon),
            AxisOrder::LonLat => assumed(b_val, a_val, AxisOrder::LonLat),
        }),
    }
}

fn explicit(lat: f64, lon: f64) -> Resolved {
    Resolved {
        lat,
        lon,
        axis_order: None,
        confidence: CONFIDENCE_EXPLICIT,
        notes: Vec::new(),
    }
}

fn one_letter(lat: f64, lon: f64) -> Resolved {
    Resolved {
        lat,
        lon,
        axis_order: None, // a hemisphere letter fixes the axis; nothing was assumed
        confidence: CONFIDENCE_RESOLVED,
        notes: Vec::new(),
    }
}

fn range_forced(lat: f64, lon: f64, order: AxisOrder) -> Resolved {
    Resolved {
        lat,
        lon,
        axis_order: Some(order),
        confidence: CONFIDENCE_RESOLVED,
        notes: vec!["axis order forced by the |value| > 90 rule".to_string()],
    }
}

fn assumed(lat: f64, lon: f64, order: AxisOrder) -> Resolved {
    Resolved {
        lat,
        lon,
        axis_order: Some(order),
        confidence: CONFIDENCE_ASSUMED,
        notes: vec!["axis order assumed from the default".to_string()],
    }
}
