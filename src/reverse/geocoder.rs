//! [`ReverseGeocoder`] and [`ReverseOptions`], re-exported flat from
//! [`reverse`](super).

use super::place::{FeatureClass, Place, bad_language_tag, is_language_tag};
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::units::Length;

/// Anything that resolves a coordinate to the place it lies in.
///
/// [`Gazetteer`](super::Gazetteer) is the offline implementation. A platform
/// provider (Apple's `CLGeocoder`, a web service) implements it too, so callers
/// can swap one for another behind a single type; the FFI bindings expose it as a
/// callback interface for exactly that.
///
/// An implementation returns `Ok(None)` when no place is close enough, never a
/// far-away guess, and honours the [`ReverseOptions`] it understands.
///
/// ```
/// use geocoordinates::{Approx, Coordinate, Length, Result};
/// use geocoordinates::reverse::{Place, ReverseGeocoder, ReverseOptions};
///
/// /// Resolves everything to one fixed place.
/// struct Everywhere(Coordinate);
///
/// impl ReverseGeocoder for Everywhere {
///     fn reverse(&self, at: Coordinate, _: &ReverseOptions) -> Result<Option<Place>> {
///         let query = Approx::new(at, Length::ZERO)?;
///         Ok(Some(Place::new("Everywhere", self.0, "XX", query)?))
///     }
/// }
///
/// let geocoder: &dyn ReverseGeocoder = &Everywhere(Coordinate::wgs84(0.0, 0.0)?);
/// let place = geocoder.reverse(Coordinate::wgs84(0.0, 1.0)?, &ReverseOptions::default())?;
/// assert_eq!(place.map(|p| p.name().to_owned()), Some("Everywhere".to_owned()));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub trait ReverseGeocoder {
    /// The place `coordinate` lies in, or `None` when nothing qualifies within
    /// `options`.
    ///
    /// # Errors
    ///
    /// Implementation-defined. [`Gazetteer`](super::Gazetteer) fails on invalid
    /// options and on a coordinate its datum conversion cannot carry; see its
    /// implementation.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::reverse::{Gazetteer, ReverseGeocoder, ReverseOptions};
    ///
    /// # let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz"));
    /// let gazetteer = Gazetteer::from_bytes(bytes)?;
    /// let lyon = gazetteer.reverse(Coordinate::wgs84(45.75, 4.85)?, &ReverseOptions::default())?;
    /// assert_eq!(lyon.unwrap().name(), "Lyon");
    /// let options = ReverseOptions::default().with_max_distance(geocoordinates::Length::from_meters(-1.0));
    /// assert!(gazetteer.reverse(Coordinate::wgs84(45.75, 4.85)?, &options).is_err());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    fn reverse(&self, coordinate: Coordinate, options: &ReverseOptions) -> Result<Option<Place>>;
}

/// The default [`ReverseOptions::max_distance`]: 50 km.
const DEFAULT_MAX_DISTANCE_M: f64 = 50_000.0;

/// What a [`ReverseGeocoder`] may return.
///
/// `#[non_exhaustive]`: build one from [`Default`] (or [`new`](Self::new)) and the
/// `with_*` builders. Values are checked when a lookup reads them.
///
/// ```
/// use geocoordinates::{Length, LengthUnit};
/// use geocoordinates::reverse::{FeatureClass, ReverseOptions};
///
/// let options = ReverseOptions::default()
///     .with_languages(["ja", "en"])
///     .with_max_distance(Length::from_unit(10.0, LengthUnit::Kilometer))
///     .with_min_population(5_000)
///     .with_min_feature_class(FeatureClass::Locality);
/// assert_eq!(options.languages, ["ja", "en"]);
/// assert_eq!(options.min_feature_class, Some(FeatureClass::Locality));
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[non_exhaustive]
pub struct ReverseOptions {
    /// BCP-47 tags of the languages to return localized names in, most preferred
    /// first. Empty (the default) returns none. A tag matches a name in the same
    /// tag, case-insensitively, or else one in its primary language (`pt-BR`
    /// matches `pt`).
    pub languages: Vec<String>,
    /// The farthest a place may be from the query, inclusive. Beyond it the
    /// result is `None`. Default 50 km; must be finite and nonnegative.
    pub max_distance: Length,
    /// The smallest population a place may have. Default `0`; a place with no
    /// population figure passes only `0`.
    pub min_population: u64,
    /// The least significant class a place may have. Default `None`, any class; a
    /// place whose class is unknown passes only `None`.
    pub min_feature_class: Option<FeatureClass>,
}

impl Default for ReverseOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl ReverseOptions {
    /// The default options, usable in a `const`.
    ///
    /// ```
    /// use geocoordinates::reverse::ReverseOptions;
    ///
    /// const OPTIONS: ReverseOptions = ReverseOptions::new();
    /// assert_eq!(OPTIONS, ReverseOptions::default());
    /// assert_eq!(OPTIONS.max_distance.meters(), 50_000.0);
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            languages: Vec::new(),
            max_distance: Length::from_meters(DEFAULT_MAX_DISTANCE_M),
            min_population: 0,
            min_feature_class: None,
        }
    }

    /// Set [`languages`](Self::languages). Not `const`: it replaces a `Vec`.
    ///
    /// ```
    /// use geocoordinates::reverse::ReverseOptions;
    ///
    /// let options = ReverseOptions::default().with_languages(["de", "fr"]);
    /// assert_eq!(options.languages, ["de", "fr"]);
    /// ```
    #[must_use]
    pub fn with_languages(
        mut self,
        languages: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.languages = languages.into_iter().map(Into::into).collect();
        self
    }

    /// Set [`max_distance`](Self::max_distance).
    ///
    /// ```
    /// use geocoordinates::Length;
    /// use geocoordinates::reverse::ReverseOptions;
    ///
    /// const NEAR: ReverseOptions = ReverseOptions::new().with_max_distance(Length::from_meters(500.0));
    /// assert_eq!(NEAR.max_distance.meters(), 500.0);
    /// ```
    #[must_use]
    pub const fn with_max_distance(mut self, max_distance: Length) -> Self {
        self.max_distance = max_distance;
        self
    }

    /// Set [`min_population`](Self::min_population).
    ///
    /// ```
    /// use geocoordinates::reverse::ReverseOptions;
    ///
    /// const TOWNS: ReverseOptions = ReverseOptions::new().with_min_population(10_000);
    /// assert_eq!(TOWNS.min_population, 10_000);
    /// ```
    #[must_use]
    pub const fn with_min_population(mut self, min_population: u64) -> Self {
        self.min_population = min_population;
        self
    }

    /// Set [`min_feature_class`](Self::min_feature_class).
    ///
    /// ```
    /// use geocoordinates::reverse::{FeatureClass, ReverseOptions};
    ///
    /// const SEATS: ReverseOptions = ReverseOptions::new().with_min_feature_class(FeatureClass::Admin2Seat);
    /// assert_eq!(SEATS.min_feature_class, Some(FeatureClass::Admin2Seat));
    /// ```
    #[must_use]
    pub const fn with_min_feature_class(mut self, min_feature_class: FeatureClass) -> Self {
        self.min_feature_class = Some(min_feature_class);
        self
    }

    /// The maximum distance in meters, after checking it and every language tag.
    pub(super) fn checked_max_distance_m(&self) -> Result<f64> {
        if let Some(bad) = self.languages.iter().find(|tag| !is_language_tag(tag)) {
            return Err(bad_language_tag("languages", bad));
        }
        let meters = self.max_distance.meters();
        if !meters.is_finite() || meters < 0.0 {
            return Err(Error::InvalidValue {
                field: "max_distance",
                detail: format!("must be finite and nonnegative, got {meters} m"),
            });
        }
        Ok(meters)
    }

    /// Whether a place of this class and population passes the filters.
    pub(super) fn admits(&self, class: FeatureClass, population: Option<u64>) -> bool {
        let class_ok = self.min_feature_class.is_none_or(|min| class >= min);
        let population_ok = self.min_population == 0
            || population.is_some_and(|population| population >= self.min_population);
        class_ok && population_ok
    }
}
