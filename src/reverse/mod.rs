//! Offline reverse geocoding: a coordinate to the place it lies in (feature
//! `reverse`).
//!
//! [`ReverseGeocoder`] is the seam: anything that maps a [`Coordinate`] to a
//! [`Place`] implements it, so a caller can swap the offline [`Gazetteer`] for a
//! platform provider (Apple's `CLGeocoder`, a web service) behind one type.
//! [`Gazetteer`] answers from GeoNames data with no network: a point resolves to
//! its nearest populated place within [`ReverseOptions::max_distance`], and to
//! `None` beyond it, never to a far-away guess.
//!
//! Every item is reached flat at `reverse::NAME`.
//!
//! ```
//! use geocoordinates::Coordinate;
//! use geocoordinates::reverse::{Gazetteer, ReverseGeocoder, ReverseOptions};
//!
//! # let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz"));
//! let gazetteer = Gazetteer::from_bytes(bytes)?;
//! let near_paris = Coordinate::wgs84(48.86, 2.35)?;
//! let place = gazetteer.reverse(near_paris, &ReverseOptions::default())?.expect("a place");
//! assert_eq!((place.name(), place.country_code()), ("Paris", "FR"));
//!
//! // Mid-Atlantic: nothing within the default 50 km.
//! let ocean = Coordinate::wgs84(30.0, -40.0)?;
//! assert!(gazetteer.reverse(ocean, &ReverseOptions::default())?.is_none());
//! # Ok::<(), geocoordinates::Error>(())
//! ```
//!
//! # Datums
//!
//! The data is WGS-84. A query on any other datum is converted to WGS-84 first with
//! [`convert`](crate::convert::convert), and [`Place::query`] carries the position
//! the lookup ran at with that conversion's error bound. GCJ-02 and BD-09 inverses
//! are approximate (about 0.5 m and 1 m), so a point that close to the edge between
//! two places' catchments may resolve to either.
//!
//! # Data and licence
//!
//! Loading is separate from lookup: [`Gazetteer::from_bytes`] and
//! [`Gazetteer::open`] read a file in the format below, which the
//! `geocoordinates-gazetteer` tool in this repository builds from the GeoNames
//! dumps (`citiesN.txt`, `admin1CodesASCII.txt`, `admin2Codes.txt`, and optionally
//! `alternateNamesV2.txt` filtered to chosen languages). Under the
//! `reverse-bundled` feature, `Gazetteer::bundled` loads a copy of GeoNames
//! `cities1000` (every place of 1,000 or more people, about 170,000) compiled into
//! the binary, with no localized names: about 6.0 MB in the binary, 3.6 MB
//! gzip-compressed.
//!
//! GeoNames data is licensed under
//! [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). An application that
//! shows places from it must credit GeoNames; [`Gazetteer::attribution`] returns
//! the credit line each file carries, and the bundled one reads:
//!
//! > Contains data from GeoNames (<https://www.geonames.org/>), licensed under CC
//! > BY 4.0 (<https://creativecommons.org/licenses/by/4.0/>). Filtered and
//! > converted to the geocoordinates gazetteer format.
//!
//! # File format, version 1
//!
//! All integers are little-endian. A *varint* is an unsigned LEB128 integer of at
//! most 10 bytes; a *string* is a varint byte length followed by that many bytes of
//! UTF-8.
//!
//! | Bytes | Field |
//! |---|---|
//! | 4 | magic `GCGZ` |
//! | 2 | format version, `1` |
//! | 2 | flags, `0` |
//! | 4 | payload length in bytes, the rest of the file |
//! | 4 | CRC-32 (IEEE, as in zlib) of the payload |
//! | … | payload |
//!
//! The payload, in order:
//!
//! 1. source date (string), then attribution (string);
//! 2. the string pool: a varint count, then that many strings, referenced below by
//!    position;
//! 3. the languages: a varint count, then that many BCP-47 tags (strings);
//! 4. the admin1 table, then the admin2 table: each a varint count, then per entry
//!    the pool positions of its code and its name (two varints);
//! 5. the places: a varint count, then per place its latitude and longitude in
//!    1e-5 degree (two `i32`), the pool position of its name (varint), its
//!    ISO 3166-1 alpha-2 country (2 ASCII bytes), its [`FeatureClass`] (1 byte, `0`
//!    for [`Section`](FeatureClass::Section) up to `6` for
//!    [`Capital`](FeatureClass::Capital)), its population (varint), its admin1 and
//!    admin2 (varints: `0` for none, else the table position plus one), and its
//!    localized names: a varint count, then per name a language position and a pool
//!    position (two varints).
//!
//! The payload must end exactly after the last place. Loading checks every field,
//! so malformed or hostile bytes are an [`Error::Parse`](crate::Error::Parse),
//! never a panic, and every count is checked against the bytes left before
//! anything is allocated for it, so memory stays proportional to the input.

// Every submodule is private: each public item is re-exported flat below, so
// `reverse::NAME` is its one path.
mod format; // the validating loader
mod gazetteer; // the GeoNames-backed `ReverseGeocoder`
mod geocoder; // the trait and its options
mod place; // the result type

pub use gazetteer::Gazetteer;
pub use geocoder::{ReverseGeocoder, ReverseOptions};
pub use place::{FeatureClass, Place};

#[cfg(test)]
mod format_tests;
#[cfg(all(test, feature = "reverse-bundled"))]
mod golden_tests;
#[cfg(test)]
mod tests;
