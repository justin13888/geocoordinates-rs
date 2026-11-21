//! The `geo:` URI reader ([RFC 5870](https://www.rfc-editor.org/rfc/rfc5870)).

use super::syntax;
use crate::coord::{Coordinate, Height};
use crate::error::{Error, ParseErrorKind, Result};
use crate::fix::{Accuracy, AccuracyKind, AxisOrder, Confidence, Fix, RawSource};
use crate::units::Length;

/// Parse a `geo:` URI per [RFC 5870](https://www.rfc-editor.org/rfc/rfc5870),
/// e.g. `geo:13.4125,103.8667` or `geo:48.2,16.3,183;crs=wgs84;u=40`.
///
/// Latitude comes first (the RFC fixes the axis order), an optional third
/// number is the altitude in meters, and the `u` parameter sets the horizontal
/// accuracy (meters) on the returned [`Fix`]. The RFC states no confidence
/// level for `u=`, so that accuracy is [`AccuracyKind::Unspecified`].
///
/// The `crs` parameter does not select a reference system: it must be absent
/// or `wgs84` (compared case-insensitively), and the returned position is
/// always WGS-84. Any other value is rejected as
/// [`ParseErrorKind::Unsupported`].
///
/// # Errors
/// Returns [`crate::Error::OutOfRange`], with the parsed latitude and
/// longitude, when the URI is well-formed but its position is outside
/// `[-90, 90]` / `[-180, 180]`. Returns [`crate::Error::Parse`] of kind
/// [`Unsupported`](crate::ParseErrorKind::Unsupported) when the `crs`
/// parameter names a reference system other than `wgs84`, and of kind
/// [`Syntax`](crate::ParseErrorKind::Syntax) when the input is otherwise not a
/// well-formed `geo:` URI.
///
/// ```
/// use geocoordinates::fix::AccuracyKind;
/// use geocoordinates::{Error, Height, ParseErrorKind};
/// use geocoordinates::parse::parse_geo_uri;
///
/// let fix = parse_geo_uri("geo:48.2,16.3,183;crs=wgs84;u=40")?;
/// assert_eq!((fix.coord.lat(), fix.coord.lon()), (48.2, 16.3));
/// assert_eq!(fix.coord.height(), Some(Height::Ellipsoidal(183.0)));
/// let accuracy = fix.accuracy.expect("u= sets the accuracy");
/// assert_eq!(accuracy.horizontal().map(|h| h.meters()), Some(40.0));
/// assert_eq!(accuracy.kind(), AccuracyKind::Unspecified);
///
/// assert!(matches!(
///     parse_geo_uri("geo:48.2"),
///     Err(Error::Parse { kind: ParseErrorKind::Syntax, .. })
/// ));
/// assert!(matches!(
///     parse_geo_uri("geo:48.2,16.3;crs=nad27"),
///     Err(Error::Parse { kind: ParseErrorKind::Unsupported, .. })
/// ));
/// // Well-formed, but latitude 91 is off the globe: the error carries both values.
/// assert_eq!(
///     parse_geo_uri("geo:91,16.3"),
///     Err(Error::OutOfRange { lat: 91.0, lon: 16.3 })
/// );
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn parse_geo_uri(input: &str) -> Result<Fix> {
    let trimmed = input.trim();
    let scheme = trimmed.get(..4);
    if scheme.is_none_or(|s| !s.eq_ignore_ascii_case("geo:")) {
        return Err(syntax(format!("not a geo: URI: {input}")));
    }
    let rest = &trimmed[4..];

    // The coordinate part precedes the first ';'; parameters follow it.
    let mut parts = rest.split(';');
    let coords = parts.next().unwrap_or_default();

    let mut numbers = coords.split(',');
    let lat = parse_number(numbers.next(), input)?;
    let lon = parse_number(numbers.next(), input)?;
    let altitude = match numbers.next() {
        Some(alt) => Some(parse_number(Some(alt), input)?),
        None => None,
    };
    if numbers.next().is_some() {
        return Err(syntax(format!("too many coordinate fields: {input}")));
    }

    let mut horizontal_m = None;
    let mut seen_crs = false;
    let mut seen_u = false;
    let mut notes = Vec::new();
    for param in parts {
        let param = param.trim();
        if param.is_empty() {
            return Err(syntax(format!("empty geo: parameter: {input}")));
        }
        let (key, value) = param.split_once('=').unwrap_or((param, ""));
        match key.trim().to_ascii_lowercase().as_str() {
            "crs" => {
                if seen_crs || seen_u {
                    return Err(syntax(format!(
                        "crs must appear once and before u: {input}"
                    )));
                }
                seen_crs = true;
                if !value.trim().eq_ignore_ascii_case("wgs84") {
                    return Err(Error::parse(
                        ParseErrorKind::Unsupported,
                        format!("unsupported geo: CRS '{}': {input}", value.trim()),
                    ));
                }
            }
            "u" => {
                if seen_u {
                    return Err(syntax(format!(
                        "duplicate geo: uncertainty parameter: {input}"
                    )));
                }
                seen_u = true;
                let u = parse_number(Some(value), input)?;
                if u < 0.0 {
                    return Err(syntax(format!(
                        "geo: uncertainty must be nonnegative: {input}"
                    )));
                }
                horizontal_m = Some(u);
            }
            _ => notes.push(format!("ignored unknown parameter '{key}'")),
        }
    }

    // A position off the globe is the typed pair error, carrying its values;
    // `parse_number` already rejected a non-finite altitude.
    let mut coord = Coordinate::wgs84(lat, lon)?;
    if let Some(alt) = altitude {
        coord = coord.with_height(Height::Ellipsoidal(alt))?;
    }

    let mut fix = Fix::from_coord(coord);
    // RFC 5870 states no confidence level for `u=`.
    fix.accuracy = horizontal_m
        .map(|h| {
            Accuracy::new(
                AccuracyKind::Unspecified,
                Some(Length::from_meters(h)),
                None,
            )
        })
        .transpose()?;
    let mut source = RawSource::new(input, Confidence::CERTAIN); // geo: is unambiguous
    source.axis_order = Some(AxisOrder::LatLon); // RFC 5870 lists lat first
    source.notes = notes;
    fix.source = Some(source);
    Ok(fix)
}

/// Parse a required `f64` field of a `geo:` URI, erroring with context.
fn parse_number(field: Option<&str>, input: &str) -> Result<f64> {
    let value = field
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or_else(|| syntax(format!("invalid geo: URI: {input}")))?;
    if !value.is_finite() {
        return Err(syntax(format!(
            "geo: numeric fields must be finite: {input}"
        )));
    }
    Ok(value)
}
