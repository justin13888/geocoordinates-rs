//! Writes the gazetteer format that `geocoordinates::reverse::Gazetteer` loads,
//! and reads the GeoNames dumps it is built from.
//!
//! The format is specified once, in the core crate's `reverse` module docs; this
//! crate is its only writer. The core crate's tests build their fixtures with
//! [`encode`], so a writer that drifted from the reader fails there.
//!
//! ```
//! use geocoordinates_gazetteer::{Dataset, FeatureClass, PlaceRow, encode};
//!
//! let mut dataset = Dataset::new("2026-11-03", "Test data");
//! dataset.places.push(PlaceRow::new(4_885_000, 235_000, "Paris", *b"FR", FeatureClass::Capital));
//! let bytes = encode(&dataset);
//! assert_eq!(&bytes[..4], b"GCGZ");
//! ```

mod geonames;

use std::collections::HashMap;

pub use geonames::{GeoNamesError, from_geonames};

/// The four bytes every gazetteer file starts with.
pub const MAGIC: [u8; 4] = *b"GCGZ";
/// The format version this crate writes.
pub const VERSION: u16 = 1;

/// The attribution GeoNames' CC BY 4.0 licence asks for, naming the change made.
pub const GEONAMES_ATTRIBUTION: &str = "Contains data from GeoNames (https://www.geonames.org/), \
    licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/). \
    Filtered and converted to the geocoordinates gazetteer format.";

/// How significant a populated place is, from a neighbourhood to a capital.
///
/// The byte each variant writes is the order the reader ranks them in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FeatureClass {
    /// A section of a populated place (GeoNames `PPLX`).
    Section = 0,
    /// A populated place with no administrative role (`PPL` and the rest).
    Locality = 1,
    /// The seat of a fourth- or fifth-order division (`PPLA4`, `PPLA5`).
    Admin4Seat = 2,
    /// The seat of a third-order division (`PPLA3`).
    Admin3Seat = 3,
    /// The seat of a second-order division (`PPLA2`).
    Admin2Seat = 4,
    /// The seat of a first-order division (`PPLA`).
    Admin1Seat = 5,
    /// A capital or seat of government (`PPLC`, `PPLG`).
    Capital = 6,
}

impl FeatureClass {
    /// The class of a GeoNames feature code, or `None` for a place that is no
    /// longer populated (`PPLH` historical, `PPLQ` abandoned, `PPLW` destroyed).
    ///
    /// ```
    /// use geocoordinates_gazetteer::FeatureClass;
    ///
    /// assert_eq!(FeatureClass::from_geonames_code("PPLA2"), Some(FeatureClass::Admin2Seat));
    /// assert_eq!(FeatureClass::from_geonames_code("PPLQ"), None);
    /// ```
    #[must_use]
    pub fn from_geonames_code(code: &str) -> Option<Self> {
        Some(match code {
            "PPLH" | "PPLQ" | "PPLW" => return None,
            "PPLC" | "PPLG" => Self::Capital,
            "PPLA" => Self::Admin1Seat,
            "PPLA2" => Self::Admin2Seat,
            "PPLA3" => Self::Admin3Seat,
            "PPLA4" | "PPLA5" => Self::Admin4Seat,
            "PPLX" => Self::Section,
            _ => Self::Locality,
        })
    }
}

/// A first- or second-order administrative division a place names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Admin {
    /// The GeoNames code within its parent, e.g. `08` for an admin1 or `001` for an
    /// admin2.
    pub code: String,
    /// Its name.
    pub name: String,
}

/// One place to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceRow {
    /// Latitude in units of 1e-5 degree.
    pub lat_e5: i32,
    /// Longitude in units of 1e-5 degree.
    pub lon_e5: i32,
    /// The place's default name.
    pub name: String,
    /// ISO 3166-1 alpha-2 country code, uppercase ASCII.
    pub country: [u8; 2],
    /// How significant the place is.
    pub class: FeatureClass,
    /// The population, `0` where GeoNames has none.
    pub population: u64,
    /// Index into [`Dataset::admin1`].
    pub admin1: Option<u32>,
    /// Index into [`Dataset::admin2`].
    pub admin2: Option<u32>,
    /// Localized names: an index into [`Dataset::languages`] and the name.
    pub localized: Vec<(u32, String)>,
}

impl PlaceRow {
    /// A place with no population, divisions or localized names.
    ///
    /// ```
    /// use geocoordinates_gazetteer::{FeatureClass, PlaceRow};
    ///
    /// let row = PlaceRow::new(0, 0, "Null Island", *b"XX", FeatureClass::Locality);
    /// assert_eq!(row.population, 0);
    /// ```
    #[must_use]
    pub fn new(
        lat_e5: i32,
        lon_e5: i32,
        name: impl Into<String>,
        country: [u8; 2],
        class: FeatureClass,
    ) -> Self {
        Self {
            lat_e5,
            lon_e5,
            name: name.into(),
            country,
            class,
            population: 0,
            admin1: None,
            admin2: None,
            localized: Vec::new(),
        }
    }
}

/// Everything a gazetteer file holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dataset {
    /// When the source data was taken, e.g. `2026-11-03`.
    pub source_date: String,
    /// The licence and attribution consumers must ship.
    pub attribution: String,
    /// The BCP-47 tags of the localized names.
    pub languages: Vec<String>,
    /// First-order divisions the places name.
    pub admin1: Vec<Admin>,
    /// Second-order divisions the places name.
    pub admin2: Vec<Admin>,
    /// The places, in the order the reader breaks distance ties by.
    pub places: Vec<PlaceRow>,
}

impl Dataset {
    /// An empty dataset with its metadata.
    ///
    /// ```
    /// use geocoordinates_gazetteer::Dataset;
    ///
    /// assert!(Dataset::new("2026-11-03", "CC BY 4.0").places.is_empty());
    /// ```
    #[must_use]
    pub fn new(source_date: impl Into<String>, attribution: impl Into<String>) -> Self {
        Self {
            source_date: source_date.into(),
            attribution: attribution.into(),
            ..Self::default()
        }
    }
}

/// The CRC-32 (IEEE 802.3, as in zlib and PNG) of `bytes`.
///
/// ```
/// assert_eq!(geocoordinates_gazetteer::crc32(b"123456789"), 0xCBF4_3926);
/// ```
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut crc = i as u32;
            let mut bit = 0;
            while bit < 8 {
                crc = if crc & 1 == 1 {
                    0xEDB8_8320 ^ (crc >> 1)
                } else {
                    crc >> 1
                };
                bit += 1;
            }
            table[i] = crc;
            i += 1;
        }
        table
    };
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc = TABLE[((crc ^ u32::from(byte)) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

/// Writes the payload with every string interned once.
struct Writer {
    out: Vec<u8>,
}

impl Writer {
    fn varint(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.out.push((value as u8) | 0x80);
            value >>= 7;
        }
        self.out.push(value as u8);
    }

    fn string(&mut self, value: &str) {
        self.varint(value.len() as u64);
        self.out.extend_from_slice(value.as_bytes());
    }
}

/// The string pool: every distinct name once, referenced by position.
#[derive(Default)]
struct Pool {
    ids: HashMap<String, u64>,
    strings: Vec<String>,
}

impl Pool {
    fn id(&mut self, value: &str) -> u64 {
        if let Some(&id) = self.ids.get(value) {
            return id;
        }
        let id = self.strings.len() as u64;
        self.ids.insert(value.to_owned(), id);
        self.strings.push(value.to_owned());
        id
    }
}

/// Encodes `dataset` in the gazetteer format.
///
/// The writer trusts its input: a dataset whose indexes point past its tables
/// encodes, and the reader rejects the result.
///
/// ```
/// use geocoordinates_gazetteer::{Dataset, crc32, encode};
///
/// let bytes = encode(&Dataset::new("2026-11-03", "Test data"));
/// assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), crc32(&bytes[16..]));
/// ```
#[must_use]
pub fn encode(dataset: &Dataset) -> Vec<u8> {
    let mut pool = Pool::default();
    let admin = |pool: &mut Pool, list: &[Admin]| -> Vec<(u64, u64)> {
        list.iter()
            .map(|a| (pool.id(&a.code), pool.id(&a.name)))
            .collect()
    };
    let admin1 = admin(&mut pool, &dataset.admin1);
    let admin2 = admin(&mut pool, &dataset.admin2);
    let places: Vec<(u64, Vec<(u32, u64)>)> = dataset
        .places
        .iter()
        .map(|place| {
            let localized = place
                .localized
                .iter()
                .map(|(lang, name)| (*lang, pool.id(name)))
                .collect();
            (pool.id(&place.name), localized)
        })
        .collect();

    let mut w = Writer { out: Vec::new() };
    w.string(&dataset.source_date);
    w.string(&dataset.attribution);
    w.varint(pool.strings.len() as u64);
    for s in &pool.strings {
        w.string(s);
    }
    w.varint(dataset.languages.len() as u64);
    for tag in &dataset.languages {
        w.string(tag);
    }
    for table in [&admin1, &admin2] {
        w.varint(table.len() as u64);
        for &(code, name) in table {
            w.varint(code);
            w.varint(name);
        }
    }
    w.varint(dataset.places.len() as u64);
    for (place, (name, localized)) in dataset.places.iter().zip(&places) {
        w.out.extend_from_slice(&place.lat_e5.to_le_bytes());
        w.out.extend_from_slice(&place.lon_e5.to_le_bytes());
        w.varint(*name);
        w.out.extend_from_slice(&place.country);
        w.out.push(place.class as u8);
        w.varint(place.population);
        w.varint(place.admin1.map_or(0, |i| u64::from(i) + 1));
        w.varint(place.admin2.map_or(0, |i| u64::from(i) + 1));
        w.varint(localized.len() as u64);
        for &(lang, name) in localized {
            w.varint(u64::from(lang));
            w.varint(name);
        }
    }

    let payload = w.out;
    let mut bytes = Vec::with_capacity(16 + payload.len());
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&crc32(&payload).to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_reference_vectors() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414F_A339
        );
    }

    #[test]
    fn varints_use_seven_bits_per_byte() {
        let mut w = Writer { out: Vec::new() };
        for value in [0, 0x7F, 0x80, 300, u64::MAX] {
            w.varint(value);
        }
        assert_eq!(
            w.out,
            [
                0x00, 0x7F, 0x80, 0x01, 0xAC, 0x02, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                0xFF, 0x01
            ]
        );
    }

    #[test]
    fn strings_are_interned_once() {
        let mut dataset = Dataset::new("d", "a");
        dataset
            .places
            .push(PlaceRow::new(1, 2, "Same", *b"FR", FeatureClass::Locality));
        dataset
            .places
            .push(PlaceRow::new(3, 4, "Same", *b"FR", FeatureClass::Locality));
        let bytes = encode(&dataset);
        let text = String::from_utf8_lossy(&bytes);
        assert_eq!(text.matches("Same").count(), 1);
    }

    #[test]
    fn the_header_states_the_payload_length() {
        let bytes = encode(&Dataset::new("2026-11-03", "x"));
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        assert_eq!(length as usize, bytes.len() - 16);
        assert_eq!(&bytes[4..8], &[1, 0, 0, 0]);
    }

    #[test]
    fn feature_codes_rank_by_role() {
        let class = |code| FeatureClass::from_geonames_code(code);
        assert_eq!(class("PPLC"), Some(FeatureClass::Capital));
        assert_eq!(class("PPLG"), Some(FeatureClass::Capital));
        assert_eq!(class("PPLA"), Some(FeatureClass::Admin1Seat));
        assert_eq!(class("PPLA3"), Some(FeatureClass::Admin3Seat));
        assert_eq!(class("PPLA4"), Some(FeatureClass::Admin4Seat));
        assert_eq!(class("PPLA5"), Some(FeatureClass::Admin4Seat));
        assert_eq!(class("PPLX"), Some(FeatureClass::Section));
        assert_eq!(class("PPL"), Some(FeatureClass::Locality));
        assert_eq!(class("PPLH"), None);
        assert_eq!(class("PPLW"), None);
        assert!(FeatureClass::Section < FeatureClass::Locality);
        assert!(FeatureClass::Admin1Seat < FeatureClass::Capital);
    }
}
