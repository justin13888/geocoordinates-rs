//! The decimal separator a rendering uses, and the locale heuristic that picks
//! one.

/// The character that separates a number's integer part from its fraction.
///
/// With [`Comma`](Self::Comma) the two components of a coordinate are
/// separated by whitespace instead of `, `, so the rendering stays unambiguous
/// and re-parses with this separator as
/// [`ParseOptions::decimal_separator`](crate::parse::ParseOptions::decimal_separator).
/// The same type serves both directions, so formatting and parsing cannot
/// disagree about it.
///
/// ```
/// use geocoordinates::Coordinate;
/// use geocoordinates::format::{DecimalSeparator, FormatOptions, format};
/// use geocoordinates::parse::parse_coordinate_with;
///
/// let options = FormatOptions {
///     decimal_separator: DecimalSeparator::Comma,
///     ..FormatOptions::default()
/// };
/// let text = format(Coordinate::wgs84(48.8566, 2.3522)?, &options)?;
/// assert_eq!(text, "48,856600 2,352200");
/// let fix = parse_coordinate_with(&text, &options.parse_options())?;
/// assert_eq!((fix.coord.lat(), fix.coord.lon()), (48.8566, 2.3522));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DecimalSeparator {
    /// `.` (`40.712800, -74.006000`).
    Point,
    /// `,` (`40,712800 -74,006000`).
    Comma,
}

impl DecimalSeparator {
    /// The decimal separator a BCP-47 locale tag conventionally uses.
    ///
    /// This is a **heuristic**, not locale data: a small allowlist of primary
    /// languages that use a decimal comma (`de`, `fr`, `es`, `pt`, `ru`, …),
    /// overridden by the regions known to use a point despite their language
    /// (`de-CH`, `de-LI`, `it-CH`, `es-MX`, `es-US`, `es-PR`). Matching is
    /// case-insensitive and accepts `-` or `_` between subtags. Every other
    /// tag, including an unknown or malformed one, yields
    /// [`Point`](Self::Point). Full CLDR number formatting is out of scope;
    /// pick the separator explicitly when the heuristic is wrong for a locale.
    ///
    /// ```
    /// use geocoordinates::format::DecimalSeparator;
    ///
    /// assert_eq!(DecimalSeparator::for_locale("de-DE"), DecimalSeparator::Comma);
    /// assert_eq!(DecimalSeparator::for_locale("de-CH"), DecimalSeparator::Point);
    /// assert_eq!(DecimalSeparator::for_locale("en-US"), DecimalSeparator::Point);
    /// ```
    #[must_use]
    pub fn for_locale(tag: &str) -> Self {
        let mut subtags = tag.split(['-', '_']);
        let primary = subtags.next().unwrap_or_default().to_ascii_lowercase();
        // The region is the first two-letter subtag after the language (a
        // four-letter script subtag may precede it). A numeric region such as
        // `419` names no single country and leaves the language's default.
        let region = subtags
            .find(|s| s.len() == 2 && s.bytes().all(|b| b.is_ascii_alphabetic()))
            .map(str::to_ascii_uppercase);
        if region
            .as_deref()
            .is_some_and(|region| POINT_REGIONS.contains(&(primary.as_str(), region)))
        {
            return DecimalSeparator::Point;
        }
        if COMMA_LANGUAGES.contains(&primary.as_str()) {
            DecimalSeparator::Comma
        } else {
            DecimalSeparator::Point
        }
    }

    /// What separates the two components of a DD rendering: a comma decimal
    /// separator collides with a `, ` list separator, so it switches to
    /// whitespace (which the text parser also splits on).
    pub(super) fn list_separator(self) -> &'static str {
        match self {
            DecimalSeparator::Point => ", ",
            DecimalSeparator::Comma => " ",
        }
    }

    /// Replace the `.` of an already-formatted number with this separator.
    pub(super) fn apply(self, numeric: &str) -> String {
        match self {
            DecimalSeparator::Point => numeric.to_owned(),
            DecimalSeparator::Comma => numeric.replace('.', ","),
        }
    }
}

/// Primary languages that conventionally write a decimal comma.
const COMMA_LANGUAGES: [&str; 27] = [
    "de", "fr", "es", "it", "ru", "nl", "pl", "pt", "sv", "da", "fi", "cs", "tr", "hu", "ro", "nb",
    "nn", "uk", "bg", "hr", "sk", "sl", "lt", "lv", "ca", "is", "et",
];

/// `(language, region)` pairs that write a decimal point although the language
/// is in [`COMMA_LANGUAGES`].
const POINT_REGIONS: [(&str, &str); 6] = [
    ("de", "CH"),
    ("de", "LI"),
    ("it", "CH"),
    ("es", "MX"),
    ("es", "US"),
    ("es", "PR"),
];

#[cfg(test)]
mod tests {
    use super::DecimalSeparator::{Comma, Point};
    use super::*;

    #[test]
    fn for_locale_reads_the_primary_language() {
        assert_eq!(DecimalSeparator::for_locale("de-DE"), Comma);
        assert_eq!(DecimalSeparator::for_locale("fr_CA"), Comma);
        assert_eq!(DecimalSeparator::for_locale("PT"), Comma);
        assert_eq!(DecimalSeparator::for_locale("et"), Comma);
        assert_eq!(DecimalSeparator::for_locale("en-US"), Point);
        assert_eq!(DecimalSeparator::for_locale("ja"), Point);
    }

    #[test]
    fn for_locale_honours_point_regions() {
        assert_eq!(DecimalSeparator::for_locale("de-CH"), Point);
        assert_eq!(DecimalSeparator::for_locale("de_li"), Point);
        assert_eq!(DecimalSeparator::for_locale("it-CH"), Point);
        assert_eq!(DecimalSeparator::for_locale("es-MX"), Point);
        assert_eq!(DecimalSeparator::for_locale("es-US"), Point);
        assert_eq!(DecimalSeparator::for_locale("es-PR"), Point);
        // A script subtag before the region is skipped.
        assert_eq!(DecimalSeparator::for_locale("es-Latn-MX"), Point);
        // The region only overrides its own language.
        assert_eq!(DecimalSeparator::for_locale("fr-CH"), Comma);
        assert_eq!(DecimalSeparator::for_locale("es-ES"), Comma);
        assert_eq!(DecimalSeparator::for_locale("es-419"), Comma);
    }

    #[test]
    fn for_locale_defaults_unknown_tags_to_point() {
        assert_eq!(DecimalSeparator::for_locale(""), Point);
        assert_eq!(DecimalSeparator::for_locale("xx-YY"), Point);
        assert_eq!(DecimalSeparator::for_locale("not a tag"), Point);
    }

    #[test]
    fn apply_and_list_separator() {
        assert_eq!(Point.apply("40.5"), "40.5");
        assert_eq!(Comma.apply("40.5"), "40,5");
        assert_eq!(Point.list_separator(), ", ");
        assert_eq!(Comma.list_separator(), " ");
    }
}
