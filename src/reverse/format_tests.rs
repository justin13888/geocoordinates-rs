//! Loader tests: every malformed file is a typed error, never a panic.

use geocoordinates_gazetteer::{Dataset, FeatureClass as Class, PlaceRow, encode};
use proptest::prelude::*;

use super::Gazetteer;
use super::format::crc32;
use crate::{Error, ParseErrorKind};

/// A file with a correct header around `payload`.
fn file(payload: &[u8]) -> Vec<u8> {
    let mut bytes = b"GCGZ\x01\x00\x00\x00".to_vec();
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&crc32(payload).to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

/// The payload of `dataset`, to edit before wrapping it with [`file`].
fn payload(dataset: &Dataset) -> Vec<u8> {
    encode(dataset)[16..].to_vec()
}

fn kind(bytes: &[u8]) -> ParseErrorKind {
    match Gazetteer::from_bytes(bytes) {
        Err(Error::Parse { kind, .. }) => kind,
        other => panic!("expected a parse error, got {other:?}"),
    }
}

fn detail(bytes: &[u8]) -> String {
    match Gazetteer::from_bytes(bytes) {
        Err(Error::Parse { detail, .. }) => detail,
        other => panic!("expected a parse error, got {other:?}"),
    }
}

fn one_place() -> Dataset {
    let mut d = Dataset::new("d", "a");
    d.languages = vec!["de".into()];
    let mut row = PlaceRow::new(100, 200, "P", *b"AB", Class::Locality);
    row.localized = vec![(0, "Q".into())];
    d.places.push(row);
    d
}

#[test]
fn the_crc_is_ieee() {
    assert_eq!(crc32(b""), 0);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn rejects_a_bad_header() {
    let good = encode(&one_place());
    assert!(detail(&good[..15]).contains("shorter than the 16-byte header"));
    assert_eq!(kind(&[]), ParseErrorKind::Syntax);

    let mut magic = good.clone();
    magic[0] = b'X';
    assert!(detail(&magic).contains("bad magic"));

    let mut version = good.clone();
    version[4] = 2;
    assert_eq!(kind(&version), ParseErrorKind::Unsupported);
    assert!(detail(&version).contains("format version 2"));

    let mut flags = good.clone();
    flags[7] = 1;
    assert_eq!(kind(&flags), ParseErrorKind::Unsupported);

    let mut long = good.clone();
    long.push(0);
    assert!(detail(&long).contains("file holds"));
    assert!(detail(&good[..good.len() - 1]).contains("file holds"));

    let mut crc = good.clone();
    crc[12] ^= 0xFF;
    assert_eq!(kind(&crc), ParseErrorKind::Checksum);
    let mut body = good;
    *body.last_mut().unwrap() ^= 0x01;
    assert_eq!(kind(&body), ParseErrorKind::Checksum);
}

#[test]
fn every_truncation_is_an_error() {
    let p = payload(&one_place());
    for len in 0..p.len() {
        let bytes = file(&p[..len]);
        assert_eq!(kind(&bytes), ParseErrorKind::Syntax, "length {len}");
    }
    assert!(Gazetteer::from_bytes(&file(&p)).is_ok());
}

#[test]
fn rejects_trailing_bytes() {
    let mut p = payload(&one_place());
    p.push(0);
    assert!(detail(&file(&p)).contains("1 trailing bytes"));
}

#[test]
fn rejects_counts_the_bytes_cannot_hold() {
    // An empty source date and attribution, then a pool claiming 2^63 strings.
    let mut p = vec![0, 0];
    p.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);
    assert!(detail(&file(&p)).contains("cannot fit"));
    // A varint that runs past 64 bits.
    let mut p = vec![0, 0];
    p.extend_from_slice(&[0xFF; 10]);
    p.push(0x01);
    assert!(detail(&file(&p)).contains("overflows"));
    let mut p = vec![0, 0];
    p.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x02]);
    assert!(detail(&file(&p)).contains("overflows"));
    // A string longer than the payload.
    let p = vec![0x7F];
    assert!(detail(&file(&p)).contains("truncated source date"));
}

/// Every count is checked against the bytes left before anything is allocated for
/// it. Unchecked, each of these would ask for 2^64 - 1 entries up front, which
/// aborts the process rather than returning.
#[test]
fn every_count_is_bounded_by_the_bytes_left() {
    const HUGE: [u8; 10] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01];
    let claims = [
        (vec![0, 0], "pool strings"),
        (vec![0, 0, 0], "languages"),
        (vec![0, 0, 0, 0], "admin1 entries"),
        (vec![0, 0, 0, 0, 0], "admin2 entries"),
        (vec![0, 0, 0, 0, 0, 0], "places"),
    ];
    for (prefix, what) in claims {
        let mut p = prefix;
        p.extend_from_slice(&HUGE);
        let found = detail(&file(&p));
        assert!(
            found.contains(&format!("{} {what} cannot fit", u64::MAX)),
            "{found}"
        );
    }
    // A place's localized-name count, the last byte but two of `one_place`.
    let p = payload(&one_place());
    let at = p.len() - 3;
    let mut spliced = p[..at].to_vec();
    spliced.extend_from_slice(&HUGE);
    spliced.extend_from_slice(&p[at + 1..]);
    assert!(detail(&file(&spliced)).contains("localized names cannot fit in the 2 bytes left"));
}

/// The smallest place is 16 bytes: two `i32`s, the country, the class byte, and
/// five one-byte varints. A count is checked against that, exactly.
#[test]
fn a_place_count_is_checked_against_the_smallest_place() {
    let mut d = Dataset::new("", "");
    d.places
        .push(PlaceRow::new(1, 2, "x", *b"AB", Class::Locality));
    let p = payload(&d);
    // Empty date and attribution, a pool of one string, no languages, two empty
    // admin tables, the place count, then the 16-byte place.
    assert_eq!(p.len(), 2 + 3 + 1 + 2 + 1 + 16);
    assert_eq!(Gazetteer::from_bytes(&file(&p)).unwrap().len(), 1);
    let short = &p[..p.len() - 1];
    assert!(detail(&file(short)).contains("1 places cannot fit in the 15 bytes left"));
}

#[test]
fn rejects_a_place_with_an_empty_name() {
    let mut d = Dataset::new("", "");
    d.places
        .push(PlaceRow::new(1, 2, "", *b"AB", Class::Locality));
    assert!(detail(&encode(&d)).contains("a place has an empty name"));
}

#[test]
fn a_header_with_an_empty_payload_reads_on_into_it() {
    // Exactly 16 bytes is a whole header, so the error is about the payload.
    assert!(detail(&file(&[])).contains("truncated source date at payload byte 0"));
}

#[test]
fn reads_the_largest_varint() {
    // A place whose population is u64::MAX, the ten-byte varint.
    let mut d = one_place();
    d.places[0].population = u64::MAX;
    let decoded = super::format::decode(&encode(&d)).unwrap();
    assert_eq!(decoded.places[0].population, u64::MAX);
}

/// Patches the byte at `at` in `dataset`'s payload, or past its end for a
/// negative offset, and returns the error detail.
fn patched(dataset: &Dataset, at: isize, byte: u8) -> String {
    let mut p = payload(dataset);
    let i = if at < 0 {
        p.len() - at.unsigned_abs()
    } else {
        at as usize
    };
    p[i] = byte;
    detail(&file(&p))
}

#[test]
fn rejects_bad_fields() {
    let d = one_place();
    let p = payload(&d);
    // Layout of `one_place`: date "d", attribution "a" (4 bytes); pool count 2 and
    // "P", "Q" (5 bytes); one language "de" (4 bytes); empty admin tables (2 bytes);
    // one place.
    let place = 4 + 5 + 4 + 2 + 1;
    assert_eq!(p.len(), place + 4 + 4 + 1 + 2 + 1 + 1 + 1 + 1 + 1 + 2);
    assert!(patched(&d, 6, 0xFF).contains("not UTF-8"));
    assert!(patched(&d, 11, b'1').contains("not a BCP-47 tag"));
    assert!(patched(&d, place as isize + 3, 0x7F).contains("out of range"));
    assert!(patched(&d, place as isize + 7, 0x7F).contains("out of range"));
    assert!(patched(&d, place as isize + 8, 2).contains("place name 2 is out of range"));
    assert!(patched(&d, place as isize + 9, b'a').contains("country"));
    assert!(patched(&d, place as isize + 11, 7).contains("feature class 7"));
    assert!(patched(&d, place as isize + 13, 1).contains("admin1 1 is out of range"));
    assert!(patched(&d, place as isize + 14, 1).contains("admin2 1 is out of range"));
    assert!(patched(&d, -2, 1).contains("localized name language 1"));
    assert!(patched(&d, -1, 2).contains("localized name 2"));
}

#[test]
fn rejects_admin_entries_past_the_pool() {
    let mut d = one_place();
    d.admin1.push(geocoordinates_gazetteer::Admin {
        code: "C".into(),
        name: "N".into(),
    });
    let mut p = payload(&d);
    // Admin strings are interned first, so the pool is C, N, P, Q; the entry's code
    // follows the metadata, the pool, the language and the admin1 count.
    let admin1 = 4 + 1 + 8 + 4 + 1;
    p[admin1] = 9;
    assert!(detail(&file(&p)).contains("admin1 entries 9 is out of range"));
}

#[test]
fn accepts_the_extremes_of_the_coordinate_range() {
    let mut d = Dataset::new("d", "a");
    for (lat, lon) in [(9_000_000, 18_000_000), (-9_000_000, -18_000_000)] {
        d.places
            .push(PlaceRow::new(lat, lon, "edge", *b"AQ", Class::Locality));
    }
    assert_eq!(Gazetteer::from_bytes(&encode(&d)).unwrap().len(), 2);
    d.places[0].lat_e5 = 9_000_001;
    assert!(detail(&encode(&d)).contains("out of range"));
    d.places[0].lat_e5 = i32::MIN;
    assert!(detail(&encode(&d)).contains("out of range"));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, max_shrink_iters: 256, ..ProptestConfig::default() })]

    /// Arbitrary bytes behind a valid header and checksum never panic the loader.
    #[test]
    fn arbitrary_payloads_never_panic(payload in proptest::collection::vec(any::<u8>(), 0..256)) {
        let _ = Gazetteer::from_bytes(&file(&payload));
    }

    /// Arbitrary bytes, header included, never panic the loader.
    #[test]
    fn arbitrary_files_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
        let _ = Gazetteer::from_bytes(&bytes);
    }

    /// Any single-byte change to a valid payload, re-checksummed, either loads or
    /// is a parse error.
    #[test]
    fn mutated_payloads_load_or_fail_cleanly(at in any::<prop::sample::Index>(), byte in any::<u8>()) {
        let mut p = payload(&super::tests::fixture_dataset());
        let i = at.index(p.len());
        p[i] = byte;
        match Gazetteer::from_bytes(&file(&p)) {
            Ok(_) | Err(Error::Parse { .. }) => {}
            Err(other) => prop_assert!(false, "unexpected error {other:?}"),
        }
    }
}
