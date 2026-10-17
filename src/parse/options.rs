//! Options for [`parse_coordinate_with`](super::parse_coordinate_with) and the
//! free-text parser, and the grid kinds a caller can ask it to expect.

use crate::fix::AxisOrder;
use crate::format::DecimalSeparator;

/// A grid reference system whose strings
/// [`parse_coordinate_with`](super::parse_coordinate_with) can decode.
///
/// [`PlusCode`](Self::PlusCode) and [`Mgrs`](Self::Mgrs) strings are
/// self-delimiting (a `+`, or a zone-band-square prefix no coordinate text
/// has), so they are always detected. [`Geohash`](Self::Geohash) and
/// [`Maidenhead`](Self::Maidenhead) strings are not: `"42"` is a valid
/// geohash and `"bed"` a valid one too, so they decode only when
/// [`ParseOptions::expect_grid`] names them. Exhaustive (no
/// `#[non_exhaustive]`): adding a kind compile-forces the FFI mirror.
///
/// ```
/// use geocoordinates::parse::{GridKind, ParseOptions, parse_coordinate, parse_coordinate_with};
///
/// // A bare geohash is not detected: it is not valid coordinate text either.
/// assert!(parse_coordinate("ezs42").is_err());
/// let options = ParseOptions::default().with_expect_grid(Some(GridKind::Geohash));
/// let fix = parse_coordinate_with("ezs42", &options)?;
/// assert!((fix.coord.lat() - 42.6).abs() < 0.03 && (fix.coord.lon() + 5.6).abs() < 0.03);
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GridKind {
    /// Open Location Code / Plus Code (`8FVC2222+22`), see
    /// [`PlusCode`](crate::grids::PlusCode).
    PlusCode,
    /// Geohash (`ezs42`), see [`Geohash`](crate::grids::Geohash).
    Geohash,
    /// Maidenhead locator (`FN20`), see
    /// [`Maidenhead`](crate::grids::Maidenhead).
    Maidenhead,
    /// MGRS reference (`18TWK0000027757`), see [`Mgrs`](crate::grids::Mgrs).
    Mgrs,
}

/// Options controlling [`parse_coordinate_with`](super::parse_coordinate_with)
/// and [`parse_text_with`](super::parse_text_with).
///
/// The default reads lat-first, `.`-decimal text and expects no particular
/// grid. To read back what [`format`](crate::format::format) rendered, take
/// these from [`FormatOptions::parse_options`](crate::format::FormatOptions::parse_options)
/// rather than building them by hand. The field types live with the module
/// that owns them: [`AxisOrder`] in [`fix`](crate::fix) and
/// [`DecimalSeparator`] in [`format`](crate::format); the
/// [`prelude`](crate::prelude) brings all three.
///
/// `#[non_exhaustive]`, so a field (such as a datum to tag parsed text with)
/// can be added without a breaking release: outside this crate, start from
/// [`default`](Self::default) and chain the `with_*` builders (or assign the
/// public fields of a `mut` binding). Under `serde`, a missing field
/// deserializes to its default, so data written before a field existed still
/// reads.
///
/// ```
/// use geocoordinates::fix::AxisOrder;
/// use geocoordinates::format::DecimalSeparator;
/// use geocoordinates::parse::{ParseOptions, parse_coordinate_with};
///
/// // `2,3522 48,8566` with decimal commas, longitude first.
/// let options = ParseOptions::default()
///     .with_default_axis_order(AxisOrder::LonLat)
///     .with_decimal_separator(DecimalSeparator::Comma);
/// let fix = parse_coordinate_with("2,3522 48,8566", &options)?;
/// assert_eq!((fix.coord.lat(), fix.coord.lon()), (48.8566, 2.3522));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[non_exhaustive]
pub struct ParseOptions {
    /// Axis order to assume for free text when neither a hemisphere letter
    /// nor the `|value| > 90` range rule decides it.
    pub default_axis_order: AxisOrder,
    /// Decimal separator of free text. With
    /// [`Comma`](DecimalSeparator::Comma), a comma between two digits is a
    /// decimal point, and the remaining commas, `;` and whitespace separate
    /// the two components (`48,8566, 2,3522` and `48,8566 2,3522` both read
    /// as 48.8566, 2.3522).
    pub decimal_separator: DecimalSeparator,
    /// A grid whose strings to try before anything but a `geo:` URI. Needed
    /// for [`GridKind::Geohash`] and [`GridKind::Maidenhead`], which are never
    /// detected otherwise; for the always-detected kinds it only moves them
    /// first. Input that is not a valid reference of this kind still parses
    /// by the other routes.
    pub expect_grid: Option<GridKind>,
}

impl ParseOptions {
    /// These options with `default_axis_order` as the
    /// [`default_axis_order`](Self::default_axis_order).
    ///
    /// ```
    /// use geocoordinates::fix::AxisOrder;
    /// use geocoordinates::parse::{ParseOptions, parse_coordinate_with};
    ///
    /// let options = ParseOptions::default().with_default_axis_order(AxisOrder::LonLat);
    /// assert_eq!(options.default_axis_order, AxisOrder::LonLat);
    /// let fix = parse_coordinate_with("2.3522, 48.8566", &options)?;
    /// assert_eq!((fix.coord.lat(), fix.coord.lon()), (48.8566, 2.3522));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn with_default_axis_order(mut self, default_axis_order: AxisOrder) -> Self {
        self.default_axis_order = default_axis_order;
        self
    }

    /// These options with `decimal_separator` as the
    /// [`decimal_separator`](Self::decimal_separator).
    ///
    /// ```
    /// use geocoordinates::format::DecimalSeparator;
    /// use geocoordinates::parse::{ParseOptions, parse_coordinate_with};
    ///
    /// let options = ParseOptions::default().with_decimal_separator(DecimalSeparator::Comma);
    /// assert_eq!(options.decimal_separator, DecimalSeparator::Comma);
    /// let fix = parse_coordinate_with("48,8566 2,3522", &options)?;
    /// assert_eq!((fix.coord.lat(), fix.coord.lon()), (48.8566, 2.3522));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn with_decimal_separator(mut self, decimal_separator: DecimalSeparator) -> Self {
        self.decimal_separator = decimal_separator;
        self
    }

    /// These options with `expect_grid` as the
    /// [`expect_grid`](Self::expect_grid); `None` expects no particular
    /// grid again.
    ///
    /// ```
    /// use geocoordinates::parse::{GridKind, ParseOptions, parse_coordinate_with};
    ///
    /// let options = ParseOptions::default().with_expect_grid(Some(GridKind::Maidenhead));
    /// assert_eq!(options.expect_grid, Some(GridKind::Maidenhead));
    /// let fix = parse_coordinate_with("FN20", &options)?;
    /// assert_eq!((fix.coord.lat(), fix.coord.lon()), (40.5, -75.0));
    /// // Without it, a bare locator is not detected.
    /// assert!(parse_coordinate_with("FN20", &options.with_expect_grid(None)).is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn with_expect_grid(mut self, expect_grid: Option<GridKind>) -> Self {
        self.expect_grid = expect_grid;
        self
    }
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            default_axis_order: AxisOrder::LatLon,
            decimal_separator: DecimalSeparator::Point,
            expect_grid: None,
        }
    }
}
