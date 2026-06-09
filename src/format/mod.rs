//! Presentation: locale-aware, round-trip-stable coordinate formatting.
//!
//! Formatting is the inverse of [`parse`](crate::parse): a [`Coordinate`] plus
//! [`FormatOptions`] renders to a string in a selectable representation. The
//! guarantee is **round-trip stability** — `parse → model → format → parse`
//! must not drift.

use crate::coord::Coordinate;

/// Target representation for rendering a coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub enum HemisphereStyle {
    /// Signed numbers (`-74.006`).
    Signed,
    /// Cardinal letters (`74.006 W`).
    Cardinal,
}

/// Options controlling how a coordinate is rendered.
#[derive(Debug, Clone)]
pub struct FormatOptions {
    /// Target representation.
    pub representation: Representation,
    /// Decimal places (DD) or sub-second/minute precision. When `None`, choose
    /// precision from the fix's accuracy rather than printing spurious digits.
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
#[must_use]
pub fn format(coord: &Coordinate, options: &FormatOptions) -> String {
    todo!("dispatch on representation; respect precision, symbols, hemisphere, locale")
}
