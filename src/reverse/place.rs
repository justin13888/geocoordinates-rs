//! [`Place`] and [`FeatureClass`], re-exported flat from [`reverse`](super).

use std::collections::BTreeMap;

use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::geodesy::geodesic_distance;
use crate::units::Length;

/// How significant a populated place is, from a neighbourhood up to a capital.
///
/// Variants are ordered by significance, so
/// [`ReverseOptions::min_feature_class`](super::ReverseOptions::min_feature_class)
/// keeps a place whose class is at least the one asked for. Each maps from the
/// GeoNames feature codes named on it.
///
/// Exhaustive (no `#[non_exhaustive]`): the FFI mirror enumerates every variant.
///
/// ```
/// use geocoordinates::reverse::FeatureClass;
///
/// assert!(FeatureClass::Capital > FeatureClass::Admin1Seat);
/// assert!(FeatureClass::Locality > FeatureClass::Section);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FeatureClass {
    /// A section of a populated place, such as a neighbourhood (`PPLX`).
    Section,
    /// A populated place with no administrative role (`PPL` and the other `P`
    /// codes).
    Locality,
    /// The seat of a fourth- or fifth-order division (`PPLA4`, `PPLA5`).
    Admin4Seat,
    /// The seat of a third-order division (`PPLA3`).
    Admin3Seat,
    /// The seat of a second-order division, such as a county town (`PPLA2`).
    Admin2Seat,
    /// The seat of a first-order division, such as a state capital (`PPLA`).
    Admin1Seat,
    /// A national capital or seat of government (`PPLC`, `PPLG`).
    Capital,
}

impl FeatureClass {
    /// The class a gazetteer file stores as `byte`, or `None` past the last.
    pub(super) const fn from_byte(byte: u8) -> Option<Self> {
        Some(match byte {
            0 => Self::Section,
            1 => Self::Locality,
            2 => Self::Admin4Seat,
            3 => Self::Admin3Seat,
            4 => Self::Admin2Seat,
            5 => Self::Admin1Seat,
            6 => Self::Capital,
            _ => return None,
        })
    }
}

/// Whether `tag` has the shape of a BCP-47 language tag: a 2–8 letter language
/// subtag, then any number of 1–8 character alphanumeric subtags, joined by `-`.
pub(super) fn is_language_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let language = parts.next().unwrap_or_default();
    (2..=8).contains(&language.len())
        && language.bytes().all(|b| b.is_ascii_alphabetic())
        && parts.all(|p| (1..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// The error for a language tag that is not BCP-47 shaped.
pub(super) fn bad_language_tag(field: &'static str, tag: &str) -> Error {
    Error::InvalidValue {
        field,
        detail: format!("`{tag}` is not a BCP-47 language tag"),
    }
}

/// A place a [`ReverseGeocoder`](super::ReverseGeocoder) resolved a coordinate to.
///
/// Valid by construction: the name is not empty, the country is an ISO 3166-1
/// alpha-2 code, every localized name is keyed by a BCP-47 tag, and the distance
/// is the geodesic distance from [`query`](Self::query) to the place. Build one
/// with [`new`](Self::new) and the `with_*` builders, which a custom
/// `ReverseGeocoder` uses to return its own results.
///
/// Under the `serde` feature, deserialization applies the same checks and
/// recomputes the distance, rejecting a stated distance more than a millimetre
/// off.
///
/// ```
/// use geocoordinates::{Approx, Coordinate, Error, Length};
/// use geocoordinates::reverse::{FeatureClass, Place};
///
/// let query = Approx::new(Coordinate::wgs84(48.86, 2.35)?, Length::ZERO)?;
/// let paris = Place::new("Paris", Coordinate::wgs84(48.85341, 2.3488)?, "FR", query)?
///     .with_admin1("11", "Île-de-France")
///     .with_feature_class(FeatureClass::Capital)
///     .with_population(2_138_551)
///     .with_localized_name("ja", "パリ")?;
/// assert_eq!(paris.admin1_name(), Some("Île-de-France"));
/// assert_eq!(paris.localized_name("JA"), Some("パリ"));
/// assert!((paris.distance().meters() - 738.1).abs() < 0.1);
/// assert!(matches!(
///     Place::new("Paris", Coordinate::wgs84(48.85, 2.35)?, "France", query),
///     Err(Error::InvalidValue { field: "country_code", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "PlaceRepr", try_from = "PlaceRepr"))]
pub struct Place {
    name: String,
    coordinate: Coordinate,
    country_code: String,
    admin1: Option<(String, String)>,
    admin2: Option<(String, String)>,
    feature_class: Option<FeatureClass>,
    population: Option<u64>,
    distance: Length,
    query: Approx<Coordinate>,
    localized_names: BTreeMap<String, String>,
}

/// The wire shape of a [`Place`], validated on its way in.
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct PlaceRepr {
    name: String,
    coordinate: Coordinate,
    country_code: String,
    admin1_code: Option<String>,
    admin1_name: Option<String>,
    admin2_code: Option<String>,
    admin2_name: Option<String>,
    feature_class: Option<FeatureClass>,
    population: Option<u64>,
    distance: Length,
    query: Approx<Coordinate>,
    #[serde(default)]
    localized_names: BTreeMap<String, String>,
}

#[cfg(feature = "serde")]
impl From<Place> for PlaceRepr {
    fn from(place: Place) -> Self {
        let (admin1_code, admin1_name) = place.admin1.unzip();
        let (admin2_code, admin2_name) = place.admin2.unzip();
        Self {
            name: place.name,
            coordinate: place.coordinate,
            country_code: place.country_code,
            admin1_code,
            admin1_name,
            admin2_code,
            admin2_name,
            feature_class: place.feature_class,
            population: place.population,
            distance: place.distance,
            query: place.query,
            localized_names: place.localized_names,
        }
    }
}

/// Both halves of an admin division, or neither.
#[cfg(feature = "serde")]
fn admin_pair(
    field: &'static str,
    code: Option<String>,
    name: Option<String>,
) -> Result<Option<(String, String)>> {
    match (code, name) {
        (Some(code), Some(name)) => Ok(Some((code, name))),
        (None, None) => Ok(None),
        _ => Err(Error::InvalidValue {
            field,
            detail: "needs both a code and a name, or neither".into(),
        }),
    }
}

#[cfg(feature = "serde")]
impl TryFrom<PlaceRepr> for Place {
    type Error = Error;

    fn try_from(repr: PlaceRepr) -> Result<Self> {
        let mut place = Self::new(repr.name, repr.coordinate, &repr.country_code, repr.query)?;
        if (place.distance.meters() - repr.distance.meters()).abs() > 1e-3 {
            return Err(Error::InvalidValue {
                field: "distance",
                detail: format!(
                    "{} m is not the {} m from the query to the place",
                    repr.distance.meters(),
                    place.distance.meters()
                ),
            });
        }
        place.admin1 = admin_pair("admin1", repr.admin1_code, repr.admin1_name)?;
        place.admin2 = admin_pair("admin2", repr.admin2_code, repr.admin2_name)?;
        place.feature_class = repr.feature_class;
        place.population = repr.population;
        for (tag, name) in repr.localized_names {
            place = place.with_localized_name(&tag, name)?;
        }
        Ok(place)
    }
}

impl Place {
    /// A place named `name` at `coordinate` in the country `country_code`,
    /// resolved from `query`: the position the lookup ran at, with the error bound
    /// of any datum conversion it took to get there.
    ///
    /// The distance is the geodesic distance from `query` to `coordinate`. Admin
    /// divisions, a feature class, a population and localized names are added with
    /// the `with_*` builders.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (field `name`) for an empty name, or (field
    /// `country_code`) for a code that is not two ASCII capital letters;
    /// [`Error::CrsMismatch`] when `coordinate` and `query` are on different CRSs, or
    /// on GCJ-02 / BD-09, which have no ellipsoid to measure on.
    ///
    /// ```
    /// use geocoordinates::{Approx, Coordinate, Error, Length};
    /// use geocoordinates::reverse::Place;
    ///
    /// let query = Approx::new(Coordinate::wgs84(0.0, 0.0)?, Length::ZERO)?;
    /// let place = Place::new("Origin", Coordinate::wgs84(0.0, 0.01)?, "XX", query)?;
    /// assert!((place.distance().meters() - 1113.2).abs() < 0.1);
    /// assert!(matches!(
    ///     Place::new("", Coordinate::wgs84(0.0, 0.0)?, "XX", query),
    ///     Err(Error::InvalidValue { field: "name", .. })
    /// ));
    /// assert!(matches!(
    ///     Place::new("Mars", Coordinate::gcj02(0.0, 0.0)?, "XX", query),
    ///     Err(Error::CrsMismatch { .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn new(
        name: impl Into<String>,
        coordinate: Coordinate,
        country_code: &str,
        query: Approx<Coordinate>,
    ) -> Result<Self> {
        let name = name.into();
        if name.is_empty() {
            return Err(Error::InvalidValue {
                field: "name",
                detail: "must not be empty".into(),
            });
        }
        if !matches!(country_code.as_bytes(), [a, b] if a.is_ascii_uppercase() && b.is_ascii_uppercase())
        {
            return Err(Error::InvalidValue {
                field: "country_code",
                detail: format!("`{country_code}` is not an ISO 3166-1 alpha-2 code"),
            });
        }
        let distance = geodesic_distance(*query.value(), coordinate)?;
        Ok(Self::from_parts(
            name,
            coordinate,
            country_code.to_owned(),
            distance,
            query,
        ))
    }

    /// A place whose fields the caller has already checked.
    pub(super) fn from_parts(
        name: String,
        coordinate: Coordinate,
        country_code: String,
        distance: Length,
        query: Approx<Coordinate>,
    ) -> Self {
        Self {
            name,
            coordinate,
            country_code,
            admin1: None,
            admin2: None,
            feature_class: None,
            population: None,
            distance,
            query,
            localized_names: BTreeMap::new(),
        }
    }

    /// Set the first-order administrative division (a state, province or region)
    /// by its code and name. A [`Gazetteer`](super::Gazetteer) uses GeoNames codes.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// # let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// let place = place.with_admin1("CA", "California");
    /// assert_eq!((place.admin1_code(), place.admin1_name()), (Some("CA"), Some("California")));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn with_admin1(mut self, code: impl Into<String>, name: impl Into<String>) -> Self {
        self.admin1 = Some((code.into(), name.into()));
        self
    }

    /// Set the second-order administrative division (a county or district) by its
    /// code and name.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// # let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// let place = place.with_admin2("075", "City and County of San Francisco");
    /// assert_eq!(place.admin2_code(), Some("075"));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn with_admin2(mut self, code: impl Into<String>, name: impl Into<String>) -> Self {
        self.admin2 = Some((code.into(), name.into()));
        self
    }

    /// Set how significant the place is.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// use geocoordinates::reverse::{FeatureClass, Place};
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// # let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// let place = place.with_feature_class(FeatureClass::Admin2Seat);
    /// assert_eq!(place.feature_class(), Some(FeatureClass::Admin2Seat));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn with_feature_class(mut self, feature_class: FeatureClass) -> Self {
        self.feature_class = Some(feature_class);
        self
    }

    /// Set the population.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// # let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(place.with_population(808_437).population(), Some(808_437));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn with_population(mut self, population: u64) -> Self {
        self.population = Some(population);
        self
    }

    /// Add the place's name in the language `tag` (BCP-47, e.g. `ja` or
    /// `zh-Hant`), replacing any name already held under the same tag.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (field `language`) when `tag` is not shaped like a
    /// BCP-47 tag.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Error, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// # let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// let place = place.with_localized_name("zh-Hant", "舊金山")?;
    /// assert_eq!(place.localized_name("zh-hant"), Some("舊金山"));
    /// assert!(matches!(
    ///     place.with_localized_name("zh_Hant", "舊金山"),
    ///     Err(Error::InvalidValue { field: "language", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn with_localized_name(mut self, tag: &str, name: impl Into<String>) -> Result<Self> {
        if !is_language_tag(tag) {
            return Err(bad_language_tag("language", tag));
        }
        self.localized_names
            .retain(|held, _| !held.eq_ignore_ascii_case(tag));
        self.localized_names.insert(tag.to_owned(), name.into());
        Ok(self)
    }

    /// The place's default name, as its source spells it.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(place.name(), "San Francisco");
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where the place is: its center, as its source records it.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// let at = Coordinate::wgs84(37.77, -122.42)?;
    /// let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(place.coordinate(), at);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn coordinate(&self) -> Coordinate {
        self.coordinate
    }

    /// The ISO 3166-1 alpha-2 code of the place's country, e.g. `FR`.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(place.country_code(), "US");
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn country_code(&self) -> &str {
        &self.country_code
    }

    /// The code of the first-order division, or `None` when the source names none.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(place.admin1_code(), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn admin1_code(&self) -> Option<&str> {
        self.admin1.as_ref().map(|(code, _)| code.as_str())
    }

    /// The name of the first-order division, or `None` when the source names none.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(place.admin1_name(), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn admin1_name(&self) -> Option<&str> {
        self.admin1.as_ref().map(|(_, name)| name.as_str())
    }

    /// The code of the second-order division, or `None` when the source names none.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(place.admin2_code(), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn admin2_code(&self) -> Option<&str> {
        self.admin2.as_ref().map(|(code, _)| code.as_str())
    }

    /// The name of the second-order division, or `None` when the source names none.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(place.admin2_name(), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn admin2_name(&self) -> Option<&str> {
        self.admin2.as_ref().map(|(_, name)| name.as_str())
    }

    /// How significant the place is, or `None` when the source does not say.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(place.feature_class(), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn feature_class(&self) -> Option<FeatureClass> {
        self.feature_class
    }

    /// The population, or `None` when the source has no figure.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(place.population(), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn population(&self) -> Option<u64> {
        self.population
    }

    /// The geodesic distance from [`query`](Self::query) to the place.
    ///
    /// For a query converted from another datum, the distance from the true input
    /// position differs from this by at most `query().max_error()`.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// let at = Coordinate::wgs84(37.77, -122.42)?;
    /// let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(place.distance(), Length::ZERO);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn distance(&self) -> Length {
        self.distance
    }

    /// The position the lookup ran at, with the error bound of the datum
    /// conversion that produced it: exact (a zero bound) for a query already on the
    /// data's datum, approximate for a GCJ-02 or BD-09 query.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// let at = Coordinate::wgs84(37.77, -122.42)?;
    /// let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// assert_eq!(*place.query().value(), at);
    /// assert_eq!(place.query().max_error(), Length::ZERO);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn query(&self) -> Approx<Coordinate> {
        self.query
    }

    /// Every localized name, keyed by BCP-47 tag.
    ///
    /// A [`Gazetteer`](super::Gazetteer) fills in only the languages
    /// [`ReverseOptions::languages`](super::ReverseOptions::languages) asked for,
    /// keyed by the tag its data spells.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// # let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// let place = place.with_localized_name("es", "San Francisco")?;
    /// assert_eq!(place.localized_names().len(), 1);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub const fn localized_names(&self) -> &BTreeMap<String, String> {
        &self.localized_names
    }

    /// The name in the language `tag`, matched case-insensitively, or `None`.
    ///
    /// ```
    /// # use geocoordinates::{Approx, Coordinate, Length};
    /// # use geocoordinates::reverse::Place;
    /// # let at = Coordinate::wgs84(37.77, -122.42)?;
    /// # let place = Place::new("San Francisco", at, "US", Approx::new(at, Length::ZERO)?)?;
    /// let place = place.with_localized_name("zh-Hant", "舊金山")?;
    /// assert_eq!(place.localized_name("ZH-HANT"), Some("舊金山"));
    /// assert_eq!(place.localized_name("zh"), None);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn localized_name(&self, tag: &str) -> Option<&str> {
        self.localized_names
            .iter()
            .find(|(held, _)| held.eq_ignore_ascii_case(tag))
            .map(|(_, name)| name.as_str())
    }

    /// Set every localized name at once; the gazetteer has already checked them.
    pub(super) fn set_localized_names(&mut self, names: BTreeMap<String, String>) {
        self.localized_names = names;
    }

    /// Set the admin divisions, class and population the gazetteer read.
    pub(super) fn set_details(
        &mut self,
        admin1: Option<(String, String)>,
        admin2: Option<(String, String)>,
        feature_class: FeatureClass,
        population: Option<u64>,
    ) {
        self.admin1 = admin1;
        self.admin2 = admin2;
        self.feature_class = Some(feature_class);
        self.population = population;
    }
}
