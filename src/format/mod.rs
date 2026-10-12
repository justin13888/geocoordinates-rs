//! Presentation: locale-aware, round-trip-stable coordinate formatting.
//!
//! Formatting is the inverse of parsing (the `parse` module):
//! a [`Coordinate`] plus [`FormatOptions`] renders to a string in a selectable
//! [`Representation`]. Each representation carries exactly the settings that
//! apply to it, so no combination of options is meaningless. The guarantee is
//! **round-trip stability** — `parse → model → format → parse` must not drift
//! beyond the rendered precision.
//!
//! The guarantee holds for DD at every decimal count, for DMS up to 9 second
//! decimals, and for DDM up to 12 minute decimals. Beyond those, the last
//! rendered digit lies below the resolution an `f64` keeps through the
//! degree / minute / second split and the re-parse, so a round trip can move
//! it by one (`…16.7753336411″W` re-renders as `…16.7753336410″W` at 10
//! second decimals). Every rendering still re-parses, to within that digit.
//!
//! Every output re-parses through
//! [`parse_coordinate_with`](crate::parse::parse_coordinate_with) given
//! [`FormatOptions::parse_options`]: DD / DMS / DDM in every [`SymbolStyle`],
//! [`HemisphereStyle`] and [`DecimalSeparator`], and every grid, to the
//! decoded cell's center. With the default options, and for DD / DMS / DDM
//! with a point separator, Plus Code or MGRS output, that is exactly
//! [`parse_coordinate`](crate::parse::parse_coordinate). A value that rounds
//! to zero renders unsigned (and N / E), so the round trip holds at the
//! equator and the prime meridian too. Grid output is the grid's canonical
//! string, which its own type ([`PlusCode`], [`Geohash`], [`Maidenhead`],
//! [`Mgrs`]) also parses back.

use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::fix::Fix;
use crate::grids::{Geohash, Maidenhead, Mgrs, MgrsPrecision, PlusCode};
use crate::parse::{AxisOrder, GridKind, ParseOptions};

mod render;
mod separator;

pub use separator::DecimalSeparator;

use render::AngleKind;

/// Highest useful decimal precision for an IEEE-754 `f64`.
const MAX_DECIMALS: u8 = 15;

/// Target representation for rendering a coordinate, carrying the settings
/// that apply to it.
///
/// The angle representations ([`DecimalDegrees`](Self::DecimalDegrees),
/// [`Dms`](Self::Dms), [`Ddm`](Self::Ddm)) take a decimal count for their last
/// rendered unit; `None` lets [`format_fix`] derive it from the fix's accuracy,
/// and otherwise uses a per-representation default. The grid representations
/// take the same size parameter as the grid's own `encode`. UTM has no string
/// form and so no variant. Exhaustive (no `#[non_exhaustive]`): adding a
/// variant compile-forces both the [`format()`] dispatch and the FFI mirror.
///
/// ```
/// use geocoordinates::Coordinate;
/// use geocoordinates::format::{
///     FormatOptions, HemisphereStyle, Representation, SymbolStyle, format,
/// };
///
/// let c = Coordinate::wgs84(40.7128, -74.006)?;
/// let ddm = FormatOptions {
///     representation: Representation::Ddm {
///         minute_decimals: Some(3),
///         symbols: SymbolStyle::Unicode,
///         hemisphere: HemisphereStyle::Cardinal,
///     },
///     ..FormatOptions::default()
/// };
/// assert_eq!(format(&c, &ddm)?, "40°42.768′N 74°00.360′W");
/// let maidenhead = FormatOptions {
///     representation: Representation::Maidenhead { pairs: 2 },
///     ..FormatOptions::default()
/// };
/// assert_eq!(format(&c, &maidenhead)?, "FN20");
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Representation {
    /// Decimal degrees (`40.712800, -74.006000`).
    DecimalDegrees {
        /// Decimal places of degrees, at most 15; `None` → 6 (~0.11 m), or
        /// derived from accuracy by [`format_fix`].
        decimals: Option<u8>,
        /// Sign or cardinal letter.
        hemisphere: HemisphereStyle,
    },
    /// Degrees-minutes-seconds (`40°42′46.08″N 74°00′21.60″W`).
    Dms {
        /// Decimal places of arcseconds, at most 15; `None` → 2, or derived
        /// from accuracy by [`format_fix`]. Round-trip stable up to 9; see
        /// the [module docs](self).
        second_decimals: Option<u8>,
        /// Degree / minute / second glyphs.
        symbols: SymbolStyle,
        /// Sign or cardinal letter.
        hemisphere: HemisphereStyle,
    },
    /// Degrees-decimal-minutes (`40°42.768′N 74°00.360′W`).
    Ddm {
        /// Decimal places of arcminutes, at most 15; `None` → 3, or derived
        /// from accuracy by [`format_fix`]. Round-trip stable up to 12; see
        /// the [module docs](self).
        minute_decimals: Option<u8>,
        /// Degree / minute glyphs.
        symbols: SymbolStyle,
        /// Sign or cardinal letter.
        hemisphere: HemisphereStyle,
    },
    /// Open Location Code / Plus Code (`8FVC2222+22`), as
    /// [`PlusCode::encode`] renders it.
    PlusCode {
        /// Code length: 2, 4, 6, 8, or 10 through 15 (10 is a ~14 m cell).
        length: u8,
    },
    /// Geohash (`ezs42`), as [`Geohash::encode`] renders it.
    Geohash {
        /// Character count, 1 through 12.
        length: u8,
    },
    /// Maidenhead locator (`FN20`), as [`Maidenhead::encode`] renders it.
    Maidenhead {
        /// Letter/digit pairs, 1 through 3.
        pairs: u8,
    },
    /// MGRS reference (`18TWK0000027757`), as [`Mgrs::try_from_coordinate`]
    /// renders it.
    Mgrs {
        /// The square size.
        precision: MgrsPrecision,
    },
}

/// Symbol style for DMS/DDM rendering.
///
/// ```
/// use geocoordinates::Coordinate;
/// use geocoordinates::format::{
///     FormatOptions, HemisphereStyle, Representation, SymbolStyle, format,
/// };
///
/// let c = Coordinate::wgs84(40.7128, -74.006)?;
/// let dms = |symbols| FormatOptions {
///     representation: Representation::Dms {
///         second_decimals: Some(0),
///         symbols,
///         hemisphere: HemisphereStyle::Cardinal,
///     },
///     ..FormatOptions::default()
/// };
/// assert_eq!(format(&c, &dms(SymbolStyle::Unicode))?, "40°42′46″N 74°00′22″W");
/// assert_eq!(format(&c, &dms(SymbolStyle::Ascii))?, "40°42'46\"N 74°00'22\"W");
/// assert_eq!(format(&c, &dms(SymbolStyle::Letters))?, "40d42m46sN 74d00m22sW");
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SymbolStyle {
    /// Unicode `°′″`.
    Unicode,
    /// ASCII `°'"`.
    Ascii,
    /// Plain letters `d m s`.
    Letters,
}

/// Sign style for hemispheres.
///
/// ```
/// use geocoordinates::Coordinate;
/// use geocoordinates::format::{FormatOptions, HemisphereStyle, Representation, format};
///
/// let c = Coordinate::wgs84(40.7128, -74.006)?;
/// let dd = |hemisphere| FormatOptions {
///     representation: Representation::DecimalDegrees {
///         decimals: Some(3),
///         hemisphere,
///     },
///     ..FormatOptions::default()
/// };
/// assert_eq!(format(&c, &dd(HemisphereStyle::Signed))?, "40.713, -74.006");
/// assert_eq!(format(&c, &dd(HemisphereStyle::Cardinal))?, "40.713 N, 74.006 W");
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HemisphereStyle {
    /// Signed numbers (`-74.006`).
    Signed,
    /// Cardinal letters (`74.006 W`).
    Cardinal,
}

/// Options controlling how a coordinate is rendered.
///
/// The default is signed decimal degrees with the decimal count unset (6 for
/// a bare coordinate, derived from accuracy by [`format_fix`]) and a
/// [`Point`](DecimalSeparator::Point) separator.
///
/// ```
/// use geocoordinates::Coordinate;
/// use geocoordinates::format::{DecimalSeparator, FormatOptions, format};
///
/// let c = Coordinate::wgs84(40.7128, -74.006)?;
/// assert_eq!(format(&c, &FormatOptions::default())?, "40.712800, -74.006000");
/// let german = FormatOptions {
///     decimal_separator: DecimalSeparator::for_locale("de-DE"),
///     ..FormatOptions::default()
/// };
/// assert_eq!(format(&c, &german)?, "40,712800 -74,006000");
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FormatOptions {
    /// Target representation and its settings.
    pub representation: Representation,
    /// Decimal separator for the angle representations; the grid
    /// representations have no decimals and ignore it. Pick it from a locale
    /// with [`DecimalSeparator::for_locale`].
    pub decimal_separator: DecimalSeparator,
}

impl FormatOptions {
    /// The [`ParseOptions`] that read back every string these options render
    /// through [`parse_coordinate_with`](crate::parse::parse_coordinate_with):
    /// lat-first, this [`decimal_separator`](Self::decimal_separator), and,
    /// for a grid representation, that grid as
    /// [`expect_grid`](ParseOptions::expect_grid) (so a Geohash or Maidenhead
    /// rendering, which is never detected unasked, decodes too).
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::format::{FormatOptions, Representation, format};
    /// use geocoordinates::parse::parse_coordinate_with;
    ///
    /// let options = FormatOptions {
    ///     representation: Representation::Geohash { length: 5 },
    ///     ..FormatOptions::default()
    /// };
    /// let c = Coordinate::wgs84(42.6, -5.6)?;
    /// let text = format(&c, &options)?; // "ezs42"
    /// let fix = parse_coordinate_with(&text, &options.parse_options())?;
    /// assert!((fix.coord.lat() - 42.6).abs() < 0.03);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn parse_options(&self) -> ParseOptions {
        let expect_grid = match self.representation {
            Representation::DecimalDegrees { .. }
            | Representation::Dms { .. }
            | Representation::Ddm { .. } => None,
            Representation::PlusCode { .. } => Some(GridKind::PlusCode),
            Representation::Geohash { .. } => Some(GridKind::Geohash),
            Representation::Maidenhead { .. } => Some(GridKind::Maidenhead),
            Representation::Mgrs { .. } => Some(GridKind::Mgrs),
        };
        ParseOptions {
            default_axis_order: AxisOrder::LatLon,
            decimal_separator: self.decimal_separator,
            expect_grid,
        }
    }
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            representation: Representation::DecimalDegrees {
                decimals: None,
                hemisphere: HemisphereStyle::Signed,
            },
            decimal_separator: DecimalSeparator::Point,
        }
    }
}

/// Render a coordinate to a string using the given options.
///
/// The angle representations render any datum's coordinate (the datum is not
/// written). The grid representations are defined on WGS-84 only.
///
/// # Errors
/// - [`Error::InvalidValue`] when an angle representation's decimal count
///   exceeds 15, or a grid representation's size is outside the range its
///   variant documents.
/// - [`Error::CrsMismatch`] when a grid representation is asked to render a
///   non-WGS-84 coordinate.
///
/// Otherwise the angle representations never fail: a [`Coordinate`] is in
/// range by construction.
///
/// ```
/// use geocoordinates::{Coordinate, Error};
/// use geocoordinates::format::{FormatOptions, Representation, format};
///
/// let options = FormatOptions {
///     representation: Representation::PlusCode { length: 10 },
///     ..FormatOptions::default()
/// };
/// let c = Coordinate::wgs84(40.7128, -74.006)?;
/// assert_eq!(format(&c, &options)?, "87G7PX7V+4J");
///
/// // A grid is defined on WGS-84 only.
/// let mars = Coordinate::gcj02(40.7128, -74.006)?;
/// assert!(matches!(format(&mars, &options), Err(Error::CrsMismatch { .. })));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn format(coord: &Coordinate, options: &FormatOptions) -> Result<String> {
    let separator = options.decimal_separator;
    match options.representation {
        Representation::DecimalDegrees {
            decimals,
            hemisphere,
        } => {
            let p = checked_decimals(decimals, 6, "decimals")?;
            Ok(render::dd(coord, p, hemisphere, separator))
        }
        Representation::Dms {
            second_decimals,
            symbols,
            hemisphere,
        } => {
            let p = checked_decimals(second_decimals, 2, "second_decimals")?;
            render::angle_pair(coord, AngleKind::Dms, p, symbols, hemisphere, separator)
        }
        Representation::Ddm {
            minute_decimals,
            symbols,
            hemisphere,
        } => {
            let p = checked_decimals(minute_decimals, 3, "minute_decimals")?;
            render::angle_pair(coord, AngleKind::Ddm, p, symbols, hemisphere, separator)
        }
        Representation::PlusCode { length } => Ok(PlusCode::encode(*coord, length)?.to_string()),
        Representation::Geohash { length } => Ok(Geohash::encode(*coord, length)?.to_string()),
        Representation::Maidenhead { pairs } => Ok(Maidenhead::encode(*coord, pairs)?.to_string()),
        Representation::Mgrs { precision } => {
            Ok(Mgrs::try_from_coordinate(*coord, precision)?.to_string())
        }
    }
}

/// Render a [`Fix`] to a string. Where the representation's decimal count is
/// `None`, it is derived from the fix's
/// [`accuracy`](crate::fix::Fix::accuracy), so digits finer than the fix's
/// resolution are not printed; a set decimal count, and every grid
/// representation, render exactly as [`format()`] does.
///
/// The derived count is in the representation's last rendered unit: decimals
/// of degrees for DD, of arcseconds for DMS (1″ ≈ 30.9 m), and of arcminutes
/// for DDM (1′ ≈ 1.85 km) — e.g. a 1 m fix renders as 5 DD decimals, 1 DMS
/// decimal, or 3 DDM decimals. A fix without a usable horizontal accuracy
/// falls back to the representation's default.
///
/// # Errors
/// As [`format()`].
///
/// ```
/// use geocoordinates::{Accuracy, Coordinate, Error, Fix};
/// use geocoordinates::format::{FormatOptions, HemisphereStyle, Representation, format_fix};
///
/// let mut fix = Fix::from_coord(Coordinate::wgs84(40.7128, -74.006)?);
/// fix.accuracy = Some(Accuracy {
///     horizontal_m: Some(1000.0),
///     vertical_m: None,
/// });
/// // A 1 km fix earns 2 decimals of degrees, not the default 6.
/// assert_eq!(format_fix(&fix, &FormatOptions::default())?, "40.71, -74.01");
///
/// let too_fine = FormatOptions {
///     representation: Representation::DecimalDegrees {
///         decimals: Some(16),
///         hemisphere: HemisphereStyle::Signed,
///     },
///     ..FormatOptions::default()
/// };
/// assert!(matches!(format_fix(&fix, &too_fine), Err(Error::InvalidValue { .. })));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn format_fix(fix: &Fix, options: &FormatOptions) -> Result<String> {
    let horizontal_m = fix.accuracy.and_then(|a| a.horizontal_m);
    let representation = match horizontal_m {
        Some(m) => with_derived_decimals(options.representation, m),
        None => options.representation,
    };
    format(
        &fix.coord,
        &FormatOptions {
            representation,
            ..*options
        },
    )
}

/// A decimal count, or `default` when unset; one above [`MAX_DECIMALS`] is an
/// error naming `field`.
fn checked_decimals(decimals: Option<u8>, default: u8, field: &'static str) -> Result<u8> {
    match decimals {
        Some(p) if p > MAX_DECIMALS => Err(Error::InvalidValue {
            field,
            detail: format!("must be no greater than {MAX_DECIMALS}"),
        }),
        Some(p) => Ok(p),
        None => Ok(default),
    }
}

/// Fill an unset decimal count from a horizontal accuracy; anything else is
/// returned unchanged.
fn with_derived_decimals(representation: Representation, horizontal_m: f64) -> Representation {
    let derived = precision_for_accuracy(horizontal_m, representation);
    match representation {
        Representation::DecimalDegrees {
            decimals: None,
            hemisphere,
        } => Representation::DecimalDegrees {
            decimals: derived,
            hemisphere,
        },
        Representation::Dms {
            second_decimals: None,
            symbols,
            hemisphere,
        } => Representation::Dms {
            second_decimals: derived,
            symbols,
            hemisphere,
        },
        Representation::Ddm {
            minute_decimals: None,
            symbols,
            hemisphere,
        } => Representation::Ddm {
            minute_decimals: derived,
            symbols,
            hemisphere,
        },
        other => other,
    }
}

/// Meters per degree of arc on the mean-radius sphere (≈ 111,195 m), a
/// heuristic used only to turn a horizontal accuracy into displayed decimal
/// places. Real ground distance per degree varies with latitude and azimuth;
/// grid decoders measure their bounds geodesically instead.
const MEAN_METERS_PER_DEGREE: f64 = 111_195.0;

/// Decimal places implied by a horizontal accuracy, so digits finer than the
/// fix's resolution are not printed. The places count the representation's
/// last rendered unit: degrees for DD, arcseconds for DMS, arcminutes for DDM.
/// `None` (unusable accuracy, or a grid representation, which has no decimal
/// count) → the caller's default.
fn precision_for_accuracy(horizontal_m: f64, representation: Representation) -> Option<u8> {
    if !horizontal_m.is_finite() || horizontal_m <= 0.0 {
        return None;
    }
    let meters_per_unit = match representation {
        Representation::DecimalDegrees { .. } => MEAN_METERS_PER_DEGREE,
        Representation::Dms { .. } => MEAN_METERS_PER_DEGREE / 3600.0,
        Representation::Ddm { .. } => MEAN_METERS_PER_DEGREE / 60.0,
        Representation::PlusCode { .. }
        | Representation::Geohash { .. }
        | Representation::Maidenhead { .. }
        | Representation::Mgrs { .. } => return None,
    };
    let places = (-(horizontal_m / meters_per_unit).log10())
        .round()
        .clamp(0.0, 8.0);
    Some(places as u8)
}

#[cfg(test)]
mod tests;
