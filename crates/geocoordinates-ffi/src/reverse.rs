//! Offline reverse geocoding.
//!
//! `Place` and `ReverseOptions` cross as records, `Gazetteer` as an object, and
//! the `ReverseGeocoder` trait as a foreign trait: Swift, Kotlin, Python or
//! TypeScript code can implement it (a `CLGeocoder` adapter, say) and hand it to
//! [`reverse_geocode`], which checks what it returns. `Approx<Coordinate>` inside a
//! place flattens to the `query` and `query_max_error_m` fields.

use std::collections::HashMap;
use std::sync::Arc;

use gc::reverse::ReverseGeocoder as _;
use geocoordinates as gc;

use crate::{Coordinate, GeoError};

/// How significant a populated place is — mirror of
/// [`gc::reverse::FeatureClass`], from a neighbourhood up to a capital.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum FeatureClass {
    /// A section of a populated place, such as a neighbourhood (`PPLX`).
    Section,
    /// A populated place with no administrative role (`PPL`).
    Locality,
    /// The seat of a fourth- or fifth-order division (`PPLA4`, `PPLA5`).
    Admin4Seat,
    /// The seat of a third-order division (`PPLA3`).
    Admin3Seat,
    /// The seat of a second-order division (`PPLA2`).
    Admin2Seat,
    /// The seat of a first-order division (`PPLA`).
    Admin1Seat,
    /// A national capital or seat of government (`PPLC`, `PPLG`).
    Capital,
}

impl From<gc::reverse::FeatureClass> for FeatureClass {
    fn from(class: gc::reverse::FeatureClass) -> Self {
        match class {
            gc::reverse::FeatureClass::Section => Self::Section,
            gc::reverse::FeatureClass::Locality => Self::Locality,
            gc::reverse::FeatureClass::Admin4Seat => Self::Admin4Seat,
            gc::reverse::FeatureClass::Admin3Seat => Self::Admin3Seat,
            gc::reverse::FeatureClass::Admin2Seat => Self::Admin2Seat,
            gc::reverse::FeatureClass::Admin1Seat => Self::Admin1Seat,
            gc::reverse::FeatureClass::Capital => Self::Capital,
        }
    }
}

impl From<FeatureClass> for gc::reverse::FeatureClass {
    fn from(class: FeatureClass) -> Self {
        match class {
            FeatureClass::Section => Self::Section,
            FeatureClass::Locality => Self::Locality,
            FeatureClass::Admin4Seat => Self::Admin4Seat,
            FeatureClass::Admin3Seat => Self::Admin3Seat,
            FeatureClass::Admin2Seat => Self::Admin2Seat,
            FeatureClass::Admin1Seat => Self::Admin1Seat,
            FeatureClass::Capital => Self::Capital,
        }
    }
}

/// A place a coordinate resolved to — mirror of [`gc::reverse::Place`].
///
/// Plain data: [`place_new`] builds a valid one, and [`place_validate`] checks a
/// hand-built one against the core crate's rules (a non-empty name, a two-letter
/// capital country code, both halves of an admin division or neither, BCP-47 keys,
/// and `distance_m` within a millimetre of the distance from `query` to
/// `coordinate`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Place {
    /// The place's default name, as its source spells it.
    pub name: String,
    /// The place's center.
    pub coordinate: Coordinate,
    /// ISO 3166-1 alpha-2 country code, e.g. `FR`.
    pub country_code: String,
    /// The first-order division's code (GeoNames codes from a gazetteer).
    pub admin1_code: Option<String>,
    /// The first-order division's name.
    pub admin1_name: Option<String>,
    /// The second-order division's code.
    pub admin2_code: Option<String>,
    /// The second-order division's name.
    pub admin2_name: Option<String>,
    /// How significant the place is, when the source says.
    pub feature_class: Option<FeatureClass>,
    /// The population, when the source has a figure.
    pub population: Option<u64>,
    /// The geodesic distance from `query` to the place, in meters.
    pub distance_m: f64,
    /// The position the lookup ran at: the input, converted to the data's datum.
    pub query: Coordinate,
    /// The error bound of that conversion in meters: `0` for input already on the
    /// data's datum, positive for GCJ-02 / BD-09 input.
    pub query_max_error_m: f64,
    /// Localized names keyed by BCP-47 tag.
    pub localized_names: HashMap<String, String>,
}

impl From<gc::reverse::Place> for Place {
    fn from(place: gc::reverse::Place) -> Self {
        let query = place.query();
        Self {
            name: place.name().to_owned(),
            coordinate: place.coordinate().into(),
            country_code: place.country_code().to_owned(),
            admin1_code: place.admin1_code().map(str::to_owned),
            admin1_name: place.admin1_name().map(str::to_owned),
            admin2_code: place.admin2_code().map(str::to_owned),
            admin2_name: place.admin2_name().map(str::to_owned),
            feature_class: place.feature_class().map(Into::into),
            population: place.population(),
            distance_m: place.distance().meters(),
            query: (*query.value()).into(),
            query_max_error_m: query.max_error().meters(),
            localized_names: place
                .localized_names()
                .iter()
                .map(|(tag, name)| (tag.clone(), name.clone()))
                .collect(),
        }
    }
}

fn admin_pair(
    field: &str,
    code: Option<String>,
    name: Option<String>,
) -> Result<Option<(String, String)>, GeoError> {
    match (code, name) {
        (Some(code), Some(name)) => Ok(Some((code, name))),
        (None, None) => Ok(None),
        _ => Err(GeoError::InvalidValue {
            field: field.to_owned(),
            detail: "needs both a code and a name, or neither".to_owned(),
        }),
    }
}

impl TryFrom<Place> for gc::reverse::Place {
    type Error = GeoError;

    fn try_from(place: Place) -> Result<Self, GeoError> {
        let query = gc::Approx::new(
            gc::Coordinate::try_from(place.query)?,
            gc::Length::from_meters(place.query_max_error_m),
        )?;
        let mut core = Self::new(
            place.name,
            place.coordinate.try_into()?,
            &place.country_code,
            query,
        )?;
        let computed = core.distance().meters();
        // Written so a NaN `distance_m` is rejected too.
        let matches = (computed - place.distance_m).abs() <= 1e-3;
        if !matches {
            return Err(GeoError::InvalidValue {
                field: "distance".to_owned(),
                detail: format!(
                    "{} m is not the {computed} m from the query to the place",
                    place.distance_m
                ),
            });
        }
        if let Some((code, name)) = admin_pair("admin1", place.admin1_code, place.admin1_name)? {
            core = core.with_admin1(code, name);
        }
        if let Some((code, name)) = admin_pair("admin2", place.admin2_code, place.admin2_name)? {
            core = core.with_admin2(code, name);
        }
        if let Some(class) = place.feature_class {
            core = core.with_feature_class(class.into());
        }
        if let Some(population) = place.population {
            core = core.with_population(population);
        }
        for (tag, name) in place.localized_names {
            core = core.with_localized_name(&tag, name)?;
        }
        Ok(core)
    }
}

/// A place named `name` at `coordinate` in the country `country_code`, resolved
/// from `query` with a conversion error of `query_max_error_m` meters (`0` for an
/// exact query). The distance is computed; set the optional fields on the record.
///
/// # Errors
/// `GeoError::InvalidValue` (field `name`) for an empty name, (field
/// `country_code`) for a code that is not two ASCII capital letters, or (field
/// `max_error`) for a negative or NaN bound; `GeoError::CrsMismatch` when
/// `coordinate` and `query` are on different reference systems or on GCJ-02 /
/// BD-09; `GeoError::OutOfRange` / `GeoError::InvalidValue` for an invalid
/// coordinate record.
#[uniffi::export]
pub fn place_new(
    name: String,
    coordinate: Coordinate,
    country_code: String,
    query: Coordinate,
    query_max_error_m: f64,
) -> Result<Place, GeoError> {
    let query = gc::Approx::new(
        gc::Coordinate::try_from(query)?,
        gc::Length::from_meters(query_max_error_m),
    )?;
    let place = gc::reverse::Place::new(name, coordinate.try_into()?, &country_code, query)?;
    Ok(place.into())
}

/// Checks a hand-built [`Place`] against the core crate's rules.
///
/// # Errors
/// Every error of [`place_new`]; `GeoError::InvalidValue` (field `admin1` /
/// `admin2`) for a division with a code but no name or the reverse, (field
/// `language`) for a localized-name key that is not a BCP-47 tag, or (field
/// `distance`) for a `distance_m` more than a millimetre off.
#[uniffi::export]
pub fn place_validate(place: Place) -> Result<(), GeoError> {
    gc::reverse::Place::try_from(place)?;
    Ok(())
}

/// What a reverse geocoder may return — mirror of
/// [`gc::reverse::ReverseOptions`]. Start from [`reverse_options_default`].
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ReverseOptions {
    /// BCP-47 tags of the languages to return localized names in, most preferred
    /// first.
    pub languages: Vec<String>,
    /// The farthest a place may be from the query, inclusive, in meters.
    pub max_distance_m: f64,
    /// The smallest population a place may have; `0` admits any.
    pub min_population: u64,
    /// The least significant class a place may have; none admits any.
    pub min_feature_class: Option<FeatureClass>,
}

impl From<ReverseOptions> for gc::reverse::ReverseOptions {
    fn from(options: ReverseOptions) -> Self {
        let core = Self::new()
            .with_languages(options.languages)
            .with_max_distance(gc::Length::from_meters(options.max_distance_m))
            .with_min_population(options.min_population);
        match options.min_feature_class {
            Some(class) => core.with_min_feature_class(class.into()),
            None => core,
        }
    }
}

impl From<gc::reverse::ReverseOptions> for ReverseOptions {
    fn from(options: gc::reverse::ReverseOptions) -> Self {
        Self {
            languages: options.languages,
            max_distance_m: options.max_distance.meters(),
            min_population: options.min_population,
            min_feature_class: options.min_feature_class.map(Into::into),
        }
    }
}

/// The default options: no localized names, a 50 km maximum distance, and no
/// population or class floor.
#[uniffi::export]
pub fn reverse_options_default() -> ReverseOptions {
    gc::reverse::ReverseOptions::default().into()
}

/// Anything that resolves a coordinate to the place it lies in — the foreign form
/// of [`gc::reverse::ReverseGeocoder`].
///
/// Implement it in Swift, Kotlin, Python or TypeScript to put a platform provider
/// behind the same interface as [`Gazetteer`]; return `None` when no place is
/// close enough, and build each place with [`place_new`].
#[uniffi::export(with_foreign)]
pub trait ReverseGeocoder: Send + Sync {
    /// The place `coordinate` lies in, or `None` when nothing qualifies within
    /// `options`.
    ///
    /// # Errors
    /// Implementation-defined; a [`Gazetteer`] fails as its own `reverse` does.
    fn reverse(
        &self,
        coordinate: Coordinate,
        options: ReverseOptions,
    ) -> Result<Option<Place>, GeoError>;
}

/// Asks `geocoder`, Rust or foreign, for the place `coordinate` lies in, and checks
/// the answer: a foreign implementation's place must pass [`place_validate`].
///
/// # Errors
/// `GeoError::OutOfRange` / `GeoError::InvalidValue` for an invalid coordinate
/// record; whatever `geocoder` returns; and every error of [`place_validate`] for
/// an invalid place.
#[uniffi::export]
pub fn reverse_geocode(
    geocoder: Arc<dyn ReverseGeocoder>,
    coordinate: Coordinate,
    options: ReverseOptions,
) -> Result<Option<Place>, GeoError> {
    gc::Coordinate::try_from(coordinate)?;
    let found = geocoder.reverse(coordinate, options)?;
    if let Some(place) = &found {
        place_validate(place.clone())?;
    }
    Ok(found)
}

/// An offline reverse geocoder over GeoNames places — the object form of
/// [`gc::reverse::Gazetteer`]. Immutable once loaded, so it is safe to share
/// between threads; loading builds an index, so load once and keep it.
#[derive(Debug, uniffi::Object)]
pub struct Gazetteer {
    inner: gc::reverse::Gazetteer,
}

#[uniffi::export]
impl Gazetteer {
    /// Loads a gazetteer from the bytes of a gazetteer file, checking every field.
    ///
    /// # Errors
    /// `GeoError::Parse` when the bytes are not a valid gazetteer file: kind
    /// `Checksum` for a payload that fails its CRC-32, `Unsupported` for a format
    /// version this build does not read, `Syntax` for anything else.
    #[uniffi::constructor]
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Arc<Self>, GeoError> {
        Ok(Arc::new(Self {
            inner: gc::reverse::Gazetteer::from_bytes(&bytes)?,
        }))
    }

    /// Loads a gazetteer from the file at `path`.
    ///
    /// # Errors
    /// `GeoError::InvalidValue` (field `path`) when the file cannot be read, and
    /// every error of `from_bytes`.
    #[uniffi::constructor]
    pub fn open(path: String) -> Result<Arc<Self>, GeoError> {
        Ok(Arc::new(Self {
            inner: gc::reverse::Gazetteer::open(path)?,
        }))
    }

    /// The nearest place to `coordinate` that passes `options`, or `None`. A
    /// coordinate on another datum is converted to WGS-84 first.
    ///
    /// # Errors
    /// `GeoError::InvalidValue` (field `max_distance`) for a negative or non-finite
    /// maximum distance, or (field `languages`) for a tag that is not BCP-47
    /// shaped; `GeoError::OutsideDomain` for a BD-09 coordinate at the South Pole;
    /// `GeoError::OutOfRange` / `GeoError::InvalidValue` for an invalid coordinate
    /// record.
    pub fn reverse(
        &self,
        coordinate: Coordinate,
        options: ReverseOptions,
    ) -> Result<Option<Place>, GeoError> {
        Ok(self
            .inner
            .reverse(coordinate.try_into()?, &options.into())?
            .map(Into::into))
    }

    /// The number of places.
    pub fn len(&self) -> u64 {
        self.inner.len() as u64
    }

    /// Whether the gazetteer has no places.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// When the source data was taken, as the file states it.
    pub fn source_date(&self) -> String {
        self.inner.source_date().to_owned()
    }

    /// The licence and attribution to ship with the places it returns.
    pub fn attribution(&self) -> String {
        self.inner.attribution().to_owned()
    }

    /// The BCP-47 tags the file holds localized names in.
    pub fn languages(&self) -> Vec<String> {
        self.inner.languages().to_vec()
    }
}

impl ReverseGeocoder for Gazetteer {
    fn reverse(
        &self,
        coordinate: Coordinate,
        options: ReverseOptions,
    ) -> Result<Option<Place>, GeoError> {
        Gazetteer::reverse(self, coordinate, options)
    }
}

/// `gazetteer` as a [`ReverseGeocoder`], so it can stand where one is expected:
/// passed to [`reverse_geocode`], or chained with a foreign implementation.
#[uniffi::export]
pub fn gazetteer_as_reverse_geocoder(gazetteer: Arc<Gazetteer>) -> Arc<dyn ReverseGeocoder> {
    gazetteer
}
