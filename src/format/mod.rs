//! Presentation: locale-aware, round-trip-stable coordinate formatting.
//!
//! Formatting is the inverse of [`parse`](crate::parse): a [`Coordinate`] plus
//! [`FormatOptions`] renders to a string in a selectable representation. The
//! guarantee is **round-trip stability** — `parse → model → format → parse`
//! must not drift.

use crate::coord::Coordinate;
use crate::error::Result;
use crate::fix::Fix;

/// Target representation for rendering a coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Representation {
    /// Decimal degrees (`40.712800, -74.006000`).
    DecimalDegrees,
    /// Degrees-minutes-seconds (`40°42′46″N 74°00′22″W`).
    Dms,
    /// Degrees-decimal-minutes (`40°42.766′N`).
    Ddm,
    /// UTM grid reference.
    Utm,
    /// MGRS string.
    Mgrs,
    /// Plus Code (Open Location Code).
    PlusCode,
    /// Geohash.
    Geohash,
}

/// Symbol style for DMS/DDM rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum SymbolStyle {
    /// Unicode `°′″`.
    Unicode,
    /// ASCII `°'"`.
    Ascii,
    /// Plain letters `d m s`.
    Letters,
}

/// Sign style for hemispheres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum HemisphereStyle {
    /// Signed numbers (`-74.006`).
    Signed,
    /// Cardinal letters (`74.006 W`).
    Cardinal,
}

/// Options controlling how a coordinate is rendered.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FormatOptions {
    /// Target representation.
    pub representation: Representation,
    /// Decimal places (DD) or sub-second/minute precision. When `None`, a
    /// sensible default is used for a bare coordinate; [`format_fix`] instead
    /// derives precision from the fix's accuracy to avoid spurious digits.
    pub precision: Option<u8>,
    /// Symbol style for DMS/DDM.
    pub symbol_style: SymbolStyle,
    /// Hemisphere rendering.
    pub hemisphere_style: HemisphereStyle,
    /// BCP-47 locale tag for number formatting (e.g. decimal comma).
    pub locale: Option<String>,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            representation: Representation::DecimalDegrees,
            precision: Some(6), // ~0.11 m; printing 9 would be lying
            symbol_style: SymbolStyle::Unicode,
            hemisphere_style: HemisphereStyle::Signed,
            locale: None,
        }
    }
}

/// Render a coordinate to a string using the given options.
///
/// # Errors
/// Returns an error when the requested [`Representation`] is undefined for the
/// coordinate — e.g. [`Representation::Utm`] at the poles (see
/// [`crate::Error::InvalidGridRef`]). The DD/DMS/DDM representations never fail.
pub fn format(coord: &Coordinate, options: &FormatOptions) -> Result<String> {
    todo!("dispatch on representation; respect precision, symbols, hemisphere, locale")
}

/// Render a [`Fix`] to a string, deriving display precision from its
/// [`accuracy`](crate::fix::Fix::accuracy) when `options.precision` is `None`
/// (so spurious digits beyond the fix's resolution are not printed).
///
/// # Errors
/// As [`format()`].
pub fn format_fix(fix: &Fix, options: &FormatOptions) -> Result<String> {
    todo!(
        "choose precision from fix.accuracy when options.precision is None, then format(&fix.coord, ..)"
    )
}
