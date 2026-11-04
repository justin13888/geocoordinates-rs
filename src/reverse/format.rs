//! The validating loader for the gazetteer file format the module docs specify.

use super::place::{FeatureClass, is_language_tag};
use crate::error::{Error, ParseErrorKind, Result};

/// The four bytes every gazetteer file starts with.
const MAGIC: [u8; 4] = *b"GCGZ";
/// The one format version this build reads.
const VERSION: u16 = 1;
/// Magic, version, flags, payload length and checksum.
const HEADER_LEN: usize = 16;
/// The fewest payload bytes a place takes: two `i32`s, a one-byte name varint, the
/// two-byte country, the class byte, and four one-byte varints.
const MIN_PLACE_LEN: usize = 4 + 4 + 1 + 2 + 1 + 4;

/// The CRC-32 (IEEE) of `bytes`.
pub(super) fn crc32(bytes: &[u8]) -> u32 {
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

fn malformed(kind: ParseErrorKind, detail: impl Into<String>) -> Error {
    Error::parse(kind, format!("gazetteer: {}", detail.into()))
}

/// An admin division: the pool positions of its code and name.
#[derive(Debug, Clone, Copy)]
pub(super) struct AdminEntry {
    pub(super) code: u32,
    pub(super) name: u32,
}

/// One place as the file stores it, with its coordinates in 1e-5 degree.
#[derive(Debug, Clone, Copy)]
pub(super) struct PlaceEntry {
    pub(super) lat_e5: i32,
    pub(super) lon_e5: i32,
    pub(super) name: u32,
    pub(super) country: [u8; 2],
    pub(super) class: FeatureClass,
    pub(super) population: u64,
    /// `0` for none, else the admin1 table position plus one.
    pub(super) admin1: u32,
    /// `0` for none, else the admin2 table position plus one.
    pub(super) admin2: u32,
    /// The place's first entry in [`Decoded::localized`].
    pub(super) localized_start: u32,
    /// How many entries of [`Decoded::localized`] are the place's.
    pub(super) localized_len: u32,
}

/// Everything a file holds, every reference checked.
#[derive(Debug, Clone)]
pub(super) struct Decoded {
    pub(super) source_date: String,
    pub(super) attribution: String,
    /// Every pool string, back to back; [`spans`](Self::spans) cuts them out.
    pub(super) text: String,
    /// Each pool string's start and end in [`text`](Self::text).
    pub(super) spans: Vec<(u32, u32)>,
    pub(super) languages: Vec<String>,
    pub(super) admin1: Vec<AdminEntry>,
    pub(super) admin2: Vec<AdminEntry>,
    pub(super) places: Vec<PlaceEntry>,
    /// Every place's localized names: a language position and a pool position.
    pub(super) localized: Vec<(u32, u32)>,
}

impl Decoded {
    /// The pool string at a position [`decode`] checked.
    pub(super) fn string(&self, id: u32) -> &str {
        let (start, end) = self.spans[id as usize];
        &self.text[start as usize..end as usize]
    }
}

/// A cursor over the payload; every read checks the bytes left.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    fn take(&mut self, n: usize, what: &str) -> Result<&'a [u8]> {
        if n > self.remaining() {
            return Err(malformed(
                ParseErrorKind::Syntax,
                format!("truncated {what} at payload byte {}", self.at),
            ));
        }
        let slice = &self.bytes[self.at..self.at + n];
        self.at += n;
        Ok(slice)
    }

    fn varint(&mut self, what: &str) -> Result<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.take(1, what)?[0];
            let bits = u64::from(byte & 0x7F);
            // The tenth byte holds bit 63 alone.
            if shift == 63 && bits > 1 {
                break;
            }
            value |= bits << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(malformed(
            ParseErrorKind::Syntax,
            format!(
                "{what} varint overflows 64 bits at payload byte {}",
                self.at
            ),
        ))
    }

    /// A varint that must be below `limit`.
    fn index(&mut self, limit: usize, what: &str) -> Result<u32> {
        let value = self.varint(what)?;
        if value >= limit as u64 {
            return Err(malformed(
                ParseErrorKind::Syntax,
                format!("{what} {value} is out of range (limit {limit})"),
            ));
        }
        // `limit` counts entries the payload holds, so it fits in a `u32`.
        Ok(value as u32)
    }

    /// A count of entries that each take at least `min_len` bytes, checked
    /// against the bytes left before anything is allocated for it.
    fn count(&mut self, min_len: usize, what: &str) -> Result<usize> {
        let count = self.varint(what)?;
        let most = self.remaining() / min_len;
        if count > most as u64 {
            return Err(malformed(
                ParseErrorKind::Syntax,
                format!(
                    "{count} {what} cannot fit in the {} bytes left",
                    self.remaining()
                ),
            ));
        }
        Ok(count as usize)
    }

    fn string(&mut self, what: &str) -> Result<&'a str> {
        let len = self.varint(what)?;
        let len = usize::try_from(len).unwrap_or(usize::MAX);
        let bytes = self.take(len, what)?;
        std::str::from_utf8(bytes)
            .map_err(|_| malformed(ParseErrorKind::Syntax, format!("{what} is not UTF-8")))
    }

    fn i32(&mut self, what: &str) -> Result<i32> {
        let bytes = self.take(4, what)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

/// Checks the header and checksum, and returns the payload.
fn payload(bytes: &[u8]) -> Result<&[u8]> {
    if bytes.len() < HEADER_LEN {
        return Err(malformed(
            ParseErrorKind::Syntax,
            format!(
                "{} bytes is shorter than the {HEADER_LEN}-byte header",
                bytes.len()
            ),
        ));
    }
    let (header, payload) = bytes.split_at(HEADER_LEN);
    let word = |at: usize| {
        u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
    };
    if header[..4] != MAGIC {
        return Err(malformed(
            ParseErrorKind::Syntax,
            "not a gazetteer file (bad magic)",
        ));
    }
    let version = u16::from_le_bytes([header[4], header[5]]);
    if version != VERSION {
        return Err(malformed(
            ParseErrorKind::Unsupported,
            format!("format version {version}; this build reads version {VERSION}"),
        ));
    }
    let flags = u16::from_le_bytes([header[6], header[7]]);
    if flags != 0 {
        return Err(malformed(
            ParseErrorKind::Unsupported,
            format!("flags {flags:#06x}; this build knows none"),
        ));
    }
    let stated = word(8) as usize;
    if stated != payload.len() {
        return Err(malformed(
            ParseErrorKind::Syntax,
            format!(
                "header states {stated} payload bytes, file holds {}",
                payload.len()
            ),
        ));
    }
    let (expected, found) = (word(12), crc32(payload));
    if expected != found {
        return Err(malformed(
            ParseErrorKind::Checksum,
            format!("payload CRC-32 is {found:#010x}, header states {expected:#010x}"),
        ));
    }
    Ok(payload)
}

fn admin_table(r: &mut Reader<'_>, pool: usize, what: &str) -> Result<Vec<AdminEntry>> {
    let count = r.count(2, what)?;
    let mut table = Vec::with_capacity(count);
    for _ in 0..count {
        table.push(AdminEntry {
            code: r.index(pool, what)?,
            name: r.index(pool, what)?,
        });
    }
    Ok(table)
}

fn place(r: &mut Reader<'_>, d: &mut Decoded) -> Result<PlaceEntry> {
    let lat_e5 = r.i32("latitude")?;
    let lon_e5 = r.i32("longitude")?;
    if lat_e5.unsigned_abs() > 9_000_000 || lon_e5.unsigned_abs() > 18_000_000 {
        return Err(malformed(
            ParseErrorKind::Syntax,
            format!("place at {lat_e5}e-5°, {lon_e5}e-5° is out of range"),
        ));
    }
    let name = r.index(d.spans.len(), "place name")?;
    // A `Place` always has a name.
    if d.string(name).is_empty() {
        return Err(malformed(
            ParseErrorKind::Syntax,
            "a place has an empty name",
        ));
    }
    let country = r.take(2, "country")?;
    let country = [country[0], country[1]];
    if !country.iter().all(u8::is_ascii_uppercase) {
        return Err(malformed(
            ParseErrorKind::Syntax,
            format!("country {country:?} is not two ASCII capital letters"),
        ));
    }
    let byte = r.take(1, "feature class")?[0];
    let class = FeatureClass::from_byte(byte).ok_or_else(|| {
        malformed(
            ParseErrorKind::Syntax,
            format!("feature class {byte} is unknown"),
        )
    })?;
    let population = r.varint("population")?;
    let admin1 = r.index(d.admin1.len() + 1, "admin1")?;
    let admin2 = r.index(d.admin2.len() + 1, "admin2")?;
    let localized_len = r.count(2, "localized names")?;
    let localized_start = d.localized.len() as u32;
    d.localized.reserve(localized_len);
    for _ in 0..localized_len {
        let language = r.index(d.languages.len(), "localized name language")?;
        let name = r.index(d.spans.len(), "localized name")?;
        d.localized.push((language, name));
    }
    Ok(PlaceEntry {
        lat_e5,
        lon_e5,
        name,
        country,
        class,
        population,
        admin1,
        admin2,
        localized_start,
        localized_len: localized_len as u32,
    })
}

/// Reads and checks a whole gazetteer file.
pub(super) fn decode(bytes: &[u8]) -> Result<Decoded> {
    let payload = payload(bytes)?;
    let mut r = Reader {
        bytes: payload,
        at: 0,
    };
    let source_date = r.string("source date")?.to_owned();
    let attribution = r.string("attribution")?.to_owned();
    let pool = r.count(1, "pool strings")?;
    let mut d = Decoded {
        source_date,
        attribution,
        text: String::new(),
        spans: Vec::with_capacity(pool),
        languages: Vec::new(),
        admin1: Vec::new(),
        admin2: Vec::new(),
        places: Vec::new(),
        localized: Vec::new(),
    };
    for _ in 0..pool {
        let s = r.string("pool string")?;
        let start = d.text.len() as u32;
        d.text.push_str(s);
        d.spans.push((start, d.text.len() as u32));
    }
    let languages = r.count(1, "languages")?;
    d.languages.reserve(languages);
    for _ in 0..languages {
        let tag = r.string("language")?;
        if !is_language_tag(tag) {
            return Err(malformed(
                ParseErrorKind::Syntax,
                format!("language `{tag}` is not a BCP-47 tag"),
            ));
        }
        d.languages.push(tag.to_owned());
    }
    d.admin1 = admin_table(&mut r, d.spans.len(), "admin1 entries")?;
    d.admin2 = admin_table(&mut r, d.spans.len(), "admin2 entries")?;
    let places = r.count(MIN_PLACE_LEN, "places")?;
    d.places.reserve(places);
    for _ in 0..places {
        let entry = place(&mut r, &mut d)?;
        d.places.push(entry);
    }
    if r.remaining() != 0 {
        return Err(malformed(
            ParseErrorKind::Syntax,
            format!("{} trailing bytes after the last place", r.remaining()),
        ));
    }
    Ok(d)
}
