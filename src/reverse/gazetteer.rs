//! [`Gazetteer`], re-exported flat from [`reverse`](super).

use std::collections::BTreeMap;
use std::path::Path;

use super::format::{Decoded, decode};
use super::geocoder::{ReverseGeocoder, ReverseOptions};
use super::place::Place;
use crate::convert::convert;
use crate::coord::{Coordinate, Crs};
use crate::error::{Error, Result};
use crate::index::{Neighbor, SpatialIndex};

/// How many neighbours the first k-nearest search asks for; each later search asks
/// for four times as many, until a place passes the filters or the search passes
/// the maximum distance.
const FIRST_K: usize = 8;

/// An offline [`ReverseGeocoder`] over GeoNames places.
///
/// A query resolves to the nearest place, by geodesic distance on WGS-84, that
/// passes the [`ReverseOptions`] filters and lies within
/// [`max_distance`](ReverseOptions::max_distance); with none, the answer is `None`.
/// Distance ties go to the place listed first in the file. The lookup runs on the
/// `index` feature's [`SpatialIndex`], so it is exact across the antimeridian and at
/// the poles.
///
/// A gazetteer is immutable once loaded and holds no handle to its source, so it
/// can be shared between threads. Loading builds the index, so load once and keep
/// it.
///
/// ```
/// use geocoordinates::{Coordinate, Length};
/// use geocoordinates::reverse::{FeatureClass, Gazetteer, ReverseGeocoder, ReverseOptions};
///
/// # let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz"));
/// let gazetteer = Gazetteer::from_bytes(bytes)?;
/// let sacre_coeur = Coordinate::wgs84(48.8867, 2.3431)?;
/// // The nearest place is the Montmartre neighbourhood...
/// let place = gazetteer.reverse(sacre_coeur, &ReverseOptions::default())?.unwrap();
/// assert_eq!(place.name(), "Montmartre");
/// // ...and the nearest locality or larger is Paris, with its name in Japanese.
/// let options = ReverseOptions::default()
///     .with_min_feature_class(FeatureClass::Locality)
///     .with_languages(["ja"]);
/// let place = gazetteer.reverse(sacre_coeur, &options)?.unwrap();
/// assert_eq!((place.name(), place.localized_name("ja")), ("Paris", Some("パリ")));
/// assert_eq!(place.admin1_name(), Some("Île-de-France"));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct Gazetteer {
    data: Decoded,
    index: SpatialIndex<()>,
}

impl Gazetteer {
    /// Loads a gazetteer from the bytes of a file in the format the
    /// [module docs](super) specify, checking every field.
    ///
    /// Memory use is proportional to `bytes.len()`, whatever the bytes claim.
    ///
    /// # Errors
    ///
    /// [`Error::Parse`] when the bytes are not a valid gazetteer file: kind
    /// [`Checksum`](crate::ParseErrorKind::Checksum) when the payload does not
    /// match its CRC-32, [`Unsupported`](crate::ParseErrorKind::Unsupported) for a
    /// format version or flag this build does not read, and
    /// [`Syntax`](crate::ParseErrorKind::Syntax) for anything else malformed.
    ///
    /// ```
    /// use geocoordinates::{Error, ParseErrorKind};
    /// use geocoordinates::reverse::Gazetteer;
    ///
    /// # let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz"));
    /// let gazetteer = Gazetteer::from_bytes(bytes)?;
    /// assert_eq!(gazetteer.len(), 4);
    ///
    /// let mut corrupt = bytes.to_vec();
    /// *corrupt.last_mut().unwrap() ^= 1;
    /// assert!(matches!(
    ///     Gazetteer::from_bytes(&corrupt),
    ///     Err(Error::Parse { kind: ParseErrorKind::Checksum, .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let data = decode(bytes)?;
        let entries = data
            .places
            .iter()
            .map(|place| {
                let lat = f64::from(place.lat_e5) / 1e5;
                let lon = f64::from(place.lon_e5) / 1e5;
                Coordinate::wgs84(lat, lon).map(|point| (point, ()))
            })
            .collect::<Result<Vec<_>>>()?;
        let index = SpatialIndex::new(entries)?;
        Ok(Self { data, index })
    }

    /// Loads a gazetteer from the file at `path`, as
    /// [`from_bytes`](Self::from_bytes) does.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (field `path`) when the file cannot be read, and
    /// every error of [`from_bytes`](Self::from_bytes).
    ///
    /// ```
    /// use geocoordinates::Error;
    /// use geocoordinates::reverse::Gazetteer;
    ///
    /// let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz");
    /// assert_eq!(Gazetteer::open(path)?.len(), 4);
    /// assert!(matches!(
    ///     Gazetteer::open("/no/such/file.gaz"),
    ///     Err(Error::InvalidValue { field: "path", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|e| Error::InvalidValue {
            field: "path",
            detail: format!("{}: {e}", path.display()),
        })?;
        Self::from_bytes(&bytes)
    }

    /// The gazetteer compiled in under the `reverse-bundled` feature: GeoNames
    /// `cities1000`, every place of 1,000 or more people, with no localized names.
    ///
    /// Each call decodes the data and builds the index again, which takes tens of
    /// milliseconds, so call it once and keep the result. Ship
    /// [`attribution`](Self::attribution) with any place it returns.
    ///
    /// ```
    /// use geocoordinates::Coordinate;
    /// use geocoordinates::reverse::{Gazetteer, ReverseGeocoder, ReverseOptions};
    ///
    /// let gazetteer = Gazetteer::bundled();
    /// let place = gazetteer.reverse(Coordinate::wgs84(-33.857, 151.215)?, &ReverseOptions::default())?;
    /// assert_eq!(place.unwrap().country_code(), "AU");
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[cfg(feature = "reverse-bundled")]
    #[cfg_attr(docsrs, doc(cfg(feature = "reverse-bundled")))]
    #[must_use]
    pub fn bundled() -> Self {
        // The bytes are compiled in, and a test loads them, so this cannot fail at
        // run time.
        Self::from_bytes(BUNDLED).expect("the bundled gazetteer is valid")
    }

    /// The number of places.
    ///
    /// ```
    /// # use geocoordinates::reverse::Gazetteer;
    /// # let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz"));
    /// assert_eq!(Gazetteer::from_bytes(bytes)?.len(), 4);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.places.len()
    }

    /// Whether the gazetteer has no places, so every lookup returns `None`.
    ///
    /// ```
    /// # use geocoordinates::reverse::Gazetteer;
    /// # let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz"));
    /// assert!(!Gazetteer::from_bytes(bytes)?.is_empty());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.places.is_empty()
    }

    /// When the source data was taken, as the file states it (e.g. `2026-11-03`).
    ///
    /// ```
    /// # use geocoordinates::reverse::Gazetteer;
    /// # let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz"));
    /// assert_eq!(Gazetteer::from_bytes(bytes)?.source_date(), "2026-11-03");
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn source_date(&self) -> &str {
        &self.data.source_date
    }

    /// The licence and attribution an application must ship with the places it
    /// shows; for GeoNames data, the CC BY 4.0 credit line.
    ///
    /// ```
    /// # use geocoordinates::reverse::Gazetteer;
    /// # let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz"));
    /// assert!(Gazetteer::from_bytes(bytes)?.attribution().contains("CC BY 4.0"));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn attribution(&self) -> &str {
        &self.data.attribution
    }

    /// The BCP-47 tags the file holds localized names in.
    ///
    /// ```
    /// # use geocoordinates::reverse::Gazetteer;
    /// # let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz"));
    /// assert_eq!(Gazetteer::from_bytes(bytes)?.languages(), ["fr", "ja"]);
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    #[must_use]
    pub fn languages(&self) -> &[String] {
        &self.data.languages
    }

    /// The file's language positions each requested tag resolves to, in order:
    /// an exact match first, else one in the tag's primary language.
    fn language_positions(&self, requested: &[String]) -> Vec<u32> {
        let mut positions = Vec::new();
        for tag in requested {
            let primary = tag.split('-').next().unwrap_or_default();
            let found = self
                .data
                .languages
                .iter()
                .position(|held| held.eq_ignore_ascii_case(tag))
                .or_else(|| {
                    self.data
                        .languages
                        .iter()
                        .position(|held| held.eq_ignore_ascii_case(primary))
                });
            if let Some(found) = found.map(|i| i as u32) {
                if !positions.contains(&found) {
                    positions.push(found);
                }
            }
        }
        positions
    }

    fn place(
        &self,
        hit: Neighbor<'_, ()>,
        query: crate::Approx<Coordinate>,
        languages: &[u32],
    ) -> Place {
        let d = &self.data;
        let entry = &d.places[hit.index()];
        let admin = |table: &[super::format::AdminEntry], slot: u32| {
            slot.checked_sub(1).map(|i| {
                let a = table[i as usize];
                (d.string(a.code).to_owned(), d.string(a.name).to_owned())
            })
        };
        let country = String::from_utf8_lossy(&entry.country).into_owned();
        let mut place = Place::from_parts(
            d.string(entry.name).to_owned(),
            hit.coordinate(),
            country,
            hit.distance(),
            query,
        );
        let population = (entry.population > 0).then_some(entry.population);
        place.set_details(
            admin(&d.admin1, entry.admin1),
            admin(&d.admin2, entry.admin2),
            entry.class,
            population,
        );
        let start = entry.localized_start as usize;
        let own = &d.localized[start..start + entry.localized_len as usize];
        let mut names = BTreeMap::new();
        for &language in languages {
            if let Some(&(_, name)) = own.iter().find(|(held, _)| *held == language) {
                names.insert(
                    d.languages[language as usize].clone(),
                    d.string(name).to_owned(),
                );
            }
        }
        place.set_localized_names(names);
        place
    }
}

impl ReverseGeocoder for Gazetteer {
    /// The nearest place to `coordinate` that passes `options`, or `None`.
    ///
    /// A coordinate on another datum is converted to WGS-84 first, and its height
    /// is ignored; [`Place::query`] carries the converted position and its bound.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] (field `max_distance`) for a negative or non-finite
    /// maximum distance, or (field `languages`) for a tag that is not BCP-47
    /// shaped; [`Error::OutsideDomain`] for a BD-09 coordinate within about 0.006°
    /// of the South Pole, which its inverse cannot carry.
    ///
    /// ```
    /// use geocoordinates::{Coordinate, Error, Length, LengthUnit};
    /// use geocoordinates::reverse::{Gazetteer, ReverseGeocoder, ReverseOptions};
    ///
    /// # let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz"));
    /// let gazetteer = Gazetteer::from_bytes(bytes)?;
    /// // Taveuni, east of the antimeridian: Savusavu is 82 km west, across it.
    /// let taveuni = Coordinate::wgs84(-16.8, -179.9)?;
    /// assert!(gazetteer.reverse(taveuni, &ReverseOptions::default())?.is_none());
    /// let wide = ReverseOptions::default().with_max_distance(Length::from_unit(100.0, LengthUnit::Kilometer));
    /// let place = gazetteer.reverse(taveuni, &wide)?.unwrap();
    /// assert_eq!((place.name(), place.country_code()), ("Savusavu", "FJ"));
    /// let bad = ReverseOptions::default().with_languages(["ja_JP"]);
    /// assert!(matches!(
    ///     gazetteer.reverse(Coordinate::wgs84(0.0, 0.0)?, &bad),
    ///     Err(Error::InvalidValue { field: "languages", .. })
    /// ));
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    fn reverse(&self, coordinate: Coordinate, options: &ReverseOptions) -> Result<Option<Place>> {
        let max_m = options.checked_max_distance_m()?;
        let flat = Coordinate::new(coordinate.lat(), coordinate.lon(), coordinate.crs())?;
        let query = convert(flat, Crs::Wgs84)?;
        let at = *query.value();
        let languages = self.language_positions(&options.languages);
        let mut checked = 0;
        let mut k = FIRST_K;
        loop {
            let hits = self.index.nearest(at, k)?;
            for hit in &hits[checked..] {
                if hit.distance().meters() > max_m {
                    return Ok(None);
                }
                let entry = &self.data.places[hit.index()];
                let population = (entry.population > 0).then_some(entry.population);
                if options.admits(entry.class, population) {
                    return Ok(Some(self.place(*hit, query, &languages)));
                }
            }
            if hits.len() < k {
                return Ok(None);
            }
            checked = hits.len();
            k = k.saturating_mul(4);
        }
    }
}

/// The bundled `cities1000` gazetteer.
#[cfg(feature = "reverse-bundled")]
static BUNDLED: &[u8] = include_bytes!("data/geonames-cities1000.gaz");
