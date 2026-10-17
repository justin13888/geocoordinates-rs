//! Encoded/discrete location systems: **Plus Codes** (Open Location Code),
//! **Geohash**, and **Maidenhead** locators.
//!
//! Each code type is valid by construction: the only ways to obtain one are
//! its `encode` (which clamps the pole and wraps the antimeridian, so it never
//! emits a code its own validator rejects) and its validating `parse` (with
//! `TryFrom<&str>` / `TryFrom<String>` / [`FromStr`] as sugar over it, which serde routes
//! through too). [`Display`] and `AsRef<str>` yield the canonical string, and
//! parsing that string back returns an equal value.
//!
//! Encoding a point into a cell is exact; `decode` returns the cell **center**
//! wrapped in [`Approx`]. Its [`max_error`](Approx::max_error) is the largest WGS-84 geodesic
//! (Karney) distance from the center to the cell's four corners, which is a
//! true upper bound on the distance to any point of the cell: along a meridian
//! or a parallel the distance from a fixed point grows monotonically away from
//! its nearest point, so it peaks at a corner.
//!
//! ```
//! use geocoordinates::Coordinate;
//! use geocoordinates::grids::Geohash;
//!
//! let code = Geohash::encode(Coordinate::wgs84(42.6, -5.6)?, 5)?;
//! assert_eq!(code.to_string(), "ezs42");
//! assert_eq!("EZS42".parse::<Geohash>()?, code);
//! # Ok::<(), geocoordinates::Error>(())
//! ```
//!
//! [`Display`]: core::fmt::Display
//! [`FromStr`]: core::str::FromStr

use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::Result;
use crate::geodesy::farthest_geodesic_m;

mod geohash;
mod maidenhead;
mod plus_code;

pub use geohash::Geohash;
pub use maidenhead::Maidenhead;
pub use plus_code::PlusCode;

/// The string conversions every code type shares: `TryFrom<String>` and
/// `FromStr` (validating through the inherent `parse`), `From<Self> for String`,
/// `AsRef<str>`, and `Display` (the canonical string). `TryFrom<String>` and
/// `From<Self> for String` are the serde route, so a deserialized code is as
/// valid as a parsed one.
macro_rules! string_conversions {
    ($($ty:ident),+) => {$(
        impl TryFrom<String> for $ty {
            type Error = crate::Error;

            /// Validate as the type's inherent `parse` does.
            ///
            /// # Errors
            /// Returns [`crate::Error::InvalidGridRef`] for a malformed code.
            fn try_from(s: String) -> Result<Self> {
                Self::parse(&s)
            }
        }

        impl core::str::FromStr for $ty {
            type Err = crate::Error;

            /// Validate as the type's inherent `parse` does.
            ///
            /// # Errors
            /// Returns [`crate::Error::InvalidGridRef`] for a malformed code.
            fn from_str(s: &str) -> Result<Self> {
                Self::parse(s)
            }
        }

        impl From<$ty> for String {
            fn from(code: $ty) -> String {
                code.0
            }
        }

        impl AsRef<str> for $ty {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl core::fmt::Display for $ty {
            /// Writes the canonical string, which parses back to an equal value.
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    )+};
}

string_conversions!(Geohash, Maidenhead, PlusCode);

/// Midpoint of a `[lo, hi]` range.
fn midpoint(lo: f64, hi: f64) -> f64 {
    (lo + hi) / 2.0
}

/// Added to every cell bound so `f64` rounding — an encoded point landing an
/// ulp outside its cell edge, or the geodesic solver's ~15 nm error — never
/// pushes a true distance past it.
const ROUNDING_MARGIN_M: f64 = 1e-6;

/// The center of a `[lat_lo, lat_hi] × [lon_lo, lon_hi]` cell, bounded by the
/// largest WGS-84 geodesic distance from that center to the cell's corners
/// (plus [`ROUNDING_MARGIN_M`]).
fn cell_center(lat_lo: f64, lat_hi: f64, lon_lo: f64, lon_hi: f64) -> Approx<Coordinate> {
    // Every caller passes a cell inside the globe, so its center is in range.
    let center = Coordinate::from_valid(
        midpoint(lat_lo, lat_hi),
        midpoint(lon_lo, lon_hi),
        None,
        crate::Crs::Wgs84,
    );
    let corners = [
        (lat_lo, lon_lo),
        (lat_lo, lon_hi),
        (lat_hi, lon_lo),
        (lat_hi, lon_hi),
    ];
    let max_error_m = farthest_geodesic_m(&center, corners) + ROUNDING_MARGIN_M;
    Approx::from_valid(center, max_error_m)
}

#[cfg(test)]
mod tests;
