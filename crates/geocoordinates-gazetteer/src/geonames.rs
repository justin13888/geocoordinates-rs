//! Reads the GeoNames dump files (tab-separated UTF-8, one record per line) into a
//! [`Dataset`].

use std::collections::HashMap;
use std::fmt;
use std::io::BufRead;

use crate::{Admin, Dataset, FeatureClass, PlaceRow};

/// A dump line that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoNamesError {
    /// Which input the line came from: `cities`, `admin1`, `admin2`,
    /// `alternate names`, or `languages`.
    pub file: &'static str,
    /// The 1-based line number, or `0` for an error about the input as a whole.
    pub line: usize,
    /// What was wrong.
    pub detail: String,
}

impl fmt::Display for GeoNamesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} line {}: {}", self.file, self.line, self.detail)
    }
}

impl std::error::Error for GeoNamesError {}

fn error(file: &'static str, line: usize, detail: impl Into<String>) -> GeoNamesError {
    GeoNamesError {
        file,
        line,
        detail: detail.into(),
    }
}

/// Each line of `reader` split on tabs, with its 1-based number; blank and `#`
/// comment lines are skipped.
fn rows(
    reader: impl BufRead,
    file: &'static str,
    min_columns: usize,
    mut each: impl FnMut(usize, &[&str]) -> Result<(), GeoNamesError>,
) -> Result<(), GeoNamesError> {
    for (i, line) in reader.lines().enumerate() {
        let number = i + 1;
        let line = line.map_err(|e| error(file, number, e.to_string()))?;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() < min_columns {
            return Err(error(
                file,
                number,
                format!("{} columns, expected {min_columns}", columns.len()),
            ));
        }
        each(number, &columns)?;
    }
    Ok(())
}

/// A `CODE<TAB>name...` admin table, keyed by its full dotted code.
fn admin_names(
    reader: impl BufRead,
    file: &'static str,
) -> Result<HashMap<String, String>, GeoNamesError> {
    let mut names = HashMap::new();
    rows(reader, file, 2, |_, columns| {
        names.insert(columns[0].to_owned(), columns[1].to_owned());
        Ok(())
    })?;
    Ok(names)
}

/// Degrees as a whole number of 1e-5 degree, within `±limit` degrees.
fn fixed_point(text: &str, limit: f64) -> Option<i32> {
    let degrees: f64 = text.parse().ok()?;
    (degrees.is_finite() && degrees.abs() <= limit).then(|| (degrees * 1e5).round() as i32)
}

/// Whether `tag` has the shape of a BCP-47 tag: a 2–8 letter language subtag and
/// any number of 1–8 character alphanumeric subtags, joined by `-`.
#[must_use]
pub(crate) fn is_language_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let language = parts.next().unwrap_or_default();
    (2..=8).contains(&language.len())
        && language.bytes().all(|b| b.is_ascii_alphabetic())
        && parts.all(|p| (1..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// Interns admin divisions in the order places first name them.
#[derive(Default)]
struct AdminTable {
    index: HashMap<String, u32>,
    list: Vec<Admin>,
}

impl AdminTable {
    fn intern(&mut self, key: String, code: &str, name: &str) -> u32 {
        *self.index.entry(key).or_insert_with(|| {
            self.list.push(Admin {
                code: code.to_owned(),
                name: name.to_owned(),
            });
            (self.list.len() - 1) as u32
        })
    }
}

/// How good a GeoNames alternate name is as a place's name in one language: the
/// preferred full name beats any other full name, which beats a short name.
fn alternate_rank(columns: &[&str]) -> Option<u8> {
    let flag = |i: usize| columns.get(i).is_some_and(|v| *v == "1");
    // A colloquial or historic name is not what the place is called today.
    if flag(6) || flag(7) {
        return None;
    }
    Some(match (flag(4), flag(5)) {
        (true, false) => 3,
        (_, false) => 2,
        (_, true) => 1,
    })
}

/// Builds a [`Dataset`] from the GeoNames dumps.
///
/// - `cities`: a `citiesN.txt` (or `allCountries.txt`) table. Rows of feature class
///   `P` are kept, except places GeoNames marks as no longer populated (`PPLH`,
///   `PPLQ`, `PPLW`) and rows without a two-letter country code. Places keep the
///   file's order.
/// - `admin1` / `admin2`: `admin1CodesASCII.txt` and `admin2Codes.txt`. A place
///   names a division only when its code is in the table.
/// - `alternate_names`: `alternateNamesV2.txt` (or `alternateNames.txt`), read only
///   when `languages` is not empty. Each place keeps its best current name in each
///   language: the preferred name, else any full name, else a short name, never a
///   colloquial or historic one. Language codes match case-insensitively and are
///   stored as `languages` spells them.
///
/// # Errors
///
/// A [`GeoNamesError`] for an unreadable line, a line with too few columns, an
/// unparsable coordinate, population or flag, or a `languages` entry that is not a
/// BCP-47 tag.
///
/// ```
/// use geocoordinates_gazetteer::from_geonames;
///
/// let cities = "2988507\tParis\tParis\t\t48.85341\t2.3488\tP\tPPLC\tFR\t\t11\t75\t751\t75056\t2138551\t\t42\tEurope/Paris\t2024-01-01\n";
/// let admin1 = "FR.11\tÎle-de-France\tIle-de-France\t3012874\n";
/// let dataset = from_geonames(cities.as_bytes(), admin1.as_bytes(), &b""[..], None::<&[u8]>, &[], "2026-11-03")?;
/// assert_eq!(dataset.places[0].name, "Paris");
/// assert_eq!(dataset.admin1[0].name, "Île-de-France");
/// assert!(from_geonames(&b"1\tBad\n"[..], &b""[..], &b""[..], None::<&[u8]>, &[], "x").is_err());
/// # Ok::<(), geocoordinates_gazetteer::GeoNamesError>(())
/// ```
pub fn from_geonames(
    cities: impl BufRead,
    admin1: impl BufRead,
    admin2: impl BufRead,
    alternate_names: Option<impl BufRead>,
    languages: &[String],
    source_date: &str,
) -> Result<Dataset, GeoNamesError> {
    if let Some(bad) = languages.iter().find(|tag| !is_language_tag(tag)) {
        return Err(error(
            "languages",
            0,
            format!("`{bad}` is not a BCP-47 tag"),
        ));
    }
    let admin1_names = admin_names(admin1, "admin1")?;
    let admin2_names = admin_names(admin2, "admin2")?;

    let mut dataset = Dataset::new(source_date, crate::GEONAMES_ATTRIBUTION);
    dataset.languages = languages.to_vec();
    let (mut admin1_table, mut admin2_table) = (AdminTable::default(), AdminTable::default());
    let mut by_id: HashMap<u64, usize> = HashMap::new();

    rows(cities, "cities", 15, |number, c| {
        let bad = |detail: &str| error("cities", number, detail.to_owned());
        let id: u64 = c[0].parse().map_err(|_| bad("unparsable geonameid"))?;
        let lat = fixed_point(c[4], 90.0).ok_or_else(|| bad("latitude outside ±90°"))?;
        let lon = fixed_point(c[5], 180.0).ok_or_else(|| bad("longitude outside ±180°"))?;
        let population: u64 = if c[14].is_empty() {
            0
        } else {
            c[14].parse().map_err(|_| bad("unparsable population"))?
        };
        let country: [u8; 2] = match c[8].as_bytes() {
            &[a, b] if a.is_ascii_uppercase() && b.is_ascii_uppercase() => [a, b],
            _ => return Ok(()),
        };
        let class = match FeatureClass::from_geonames_code(c[7]) {
            Some(class) if c[6] == "P" && !c[1].is_empty() => class,
            _ => return Ok(()),
        };
        let mut row = PlaceRow::new(lat, lon, c[1], country, class);
        row.population = population;
        let a1_key = format!("{}.{}", c[8], c[10]);
        if let Some(name) = admin1_names.get(&a1_key) {
            row.admin1 = Some(admin1_table.intern(a1_key.clone(), c[10], name));
        }
        let a2_key = format!("{a1_key}.{}", c[11]);
        if let Some(name) = admin2_names.get(&a2_key) {
            row.admin2 = Some(admin2_table.intern(a2_key, c[11], name));
        }
        by_id.insert(id, dataset.places.len());
        dataset.places.push(row);
        Ok(())
    })?;

    if let Some(alternate_names) = alternate_names.filter(|_| !languages.is_empty()) {
        let lowered: Vec<String> = languages.iter().map(|t| t.to_ascii_lowercase()).collect();
        // (place, language) -> (rank, name)
        let mut best: HashMap<(usize, u32), (u8, String)> = HashMap::new();
        rows(alternate_names, "alternate names", 4, |number, c| {
            let id: u64 = c[1]
                .parse()
                .map_err(|_| error("alternate names", number, "unparsable geonameid"))?;
            let Some(&place) = by_id.get(&id) else {
                return Ok(());
            };
            let tag = c[2].to_ascii_lowercase();
            let Some(lang) = lowered.iter().position(|t| *t == tag) else {
                return Ok(());
            };
            let Some(rank) = alternate_rank(c) else {
                return Ok(());
            };
            if c[3].is_empty() {
                return Ok(());
            }
            let key = (place, lang as u32);
            if best.get(&key).is_none_or(|(have, _)| rank > *have) {
                best.insert(key, (rank, c[3].to_owned()));
            }
            Ok(())
        })?;
        let mut best: Vec<_> = best.into_iter().collect();
        best.sort_unstable_by_key(|((place, lang), _)| (*place, *lang));
        for ((place, lang), (_, name)) in best {
            dataset.places[place].localized.push((lang, name));
        }
    }

    dataset.admin1 = admin1_table.list;
    dataset.admin2 = admin2_table.list;
    Ok(dataset)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CITIES: &str = "\
# a comment line
2988507\tParis\tParis\t\t48.85341\t2.3488\tP\tPPLC\tFR\t\t11\t75\t751\t75056\t2138551\t\t42\tEurope/Paris\t2024-01-01
2995469\tMarseille\tMarseille\t\t43.29695\t5.38107\tP\tPPLA\tFR\t\t93\t13\t133\t13055\t870731\t\t28\tEurope/Paris\t2024-01-01
1\tGone\tGone\t\t1.0\t1.0\tP\tPPLQ\tFR\t\t11\t75\t\t\t0\t\t\t\t
2\tNowhere\tNowhere\t\t1.0\t1.0\tP\tPPL\t\t\t\t\t\t\t\t\t\t\t
3\tUnpopulated\tUnpopulated\t\t2.5\t-3.25\tP\tPPL\tFR\t\t99\t\t\t\t\t\t\t\t
";
    const ADMIN1: &str =
        "FR.11\tÎle-de-France\tIle-de-France\t3012874\nFR.93\tProvence\tProvence\t2985244\n";
    const ADMIN2: &str = "FR.11.75\tParis\tParis\t2968815\n";
    const ALTERNATES: &str = "\
1\t2988507\tde\tParis\t\t\t\t\t\t
2\t2988507\tja\tパリ\t1\t\t\t\t\t
3\t2988507\tja\tパリ市\t\t\t\t\t\t
4\t2988507\tja\tP\t\t1\t\t\t\t
5\t2995469\tja\tマルセイユ\t\t1\t\t\t\t
6\t2995469\tja\tマッサリア\t1\t\t\t1\t\t
7\t2995469\tde\tMarsilia\t1\t\t1\t\t\t
8\t9999999\tde\tElsewhere\t1\t\t\t\t\t
9\t2995469\tfr\tMarseille\t1\t\t\t\t\t
";

    fn build(languages: &[&str]) -> Result<Dataset, GeoNamesError> {
        let languages: Vec<String> = languages.iter().map(|s| (*s).to_owned()).collect();
        from_geonames(
            CITIES.as_bytes(),
            ADMIN1.as_bytes(),
            ADMIN2.as_bytes(),
            Some(ALTERNATES.as_bytes()),
            &languages,
            "2026-11-03",
        )
    }

    #[test]
    fn keeps_populated_places_in_file_order() {
        let dataset = build(&[]).unwrap();
        let names: Vec<&str> = dataset.places.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Paris", "Marseille", "Unpopulated"]);
        let paris = &dataset.places[0];
        assert_eq!((paris.lat_e5, paris.lon_e5), (4_885_341, 234_880));
        assert_eq!(paris.country, *b"FR");
        assert_eq!(paris.class, FeatureClass::Capital);
        assert_eq!(paris.population, 2_138_551);
        assert_eq!(dataset.places[2].population, 0);
        assert_eq!(
            (dataset.places[2].lat_e5, dataset.places[2].lon_e5),
            (250_000, -325_000)
        );
        assert_eq!(dataset.source_date, "2026-11-03");
        assert_eq!(dataset.attribution, crate::GEONAMES_ATTRIBUTION);
    }

    #[test]
    fn names_only_divisions_the_tables_hold() {
        let dataset = build(&[]).unwrap();
        let (paris, marseille, other) =
            (&dataset.places[0], &dataset.places[1], &dataset.places[2]);
        assert_eq!(
            dataset.admin1[paris.admin1.unwrap() as usize].name,
            "Île-de-France"
        );
        assert_eq!(dataset.admin1[paris.admin1.unwrap() as usize].code, "11");
        assert_eq!(dataset.admin2[paris.admin2.unwrap() as usize].code, "75");
        assert_eq!(
            dataset.admin1[marseille.admin1.unwrap() as usize].name,
            "Provence"
        );
        assert_eq!(marseille.admin2, None);
        assert_eq!((other.admin1, other.admin2), (None, None));
        assert_eq!(dataset.admin1.len(), 2);
        assert_eq!(dataset.admin2.len(), 1);
    }

    #[test]
    fn picks_the_best_current_name_per_language() {
        let dataset = build(&["ja", "DE"]).unwrap();
        assert_eq!(dataset.languages, ["ja", "DE"]);
        assert_eq!(
            dataset.places[0].localized,
            [(0, "パリ".to_owned()), (1, "Paris".to_owned())]
        );
        // The short name is all Marseille has in Japanese once the historic one is
        // dropped; its colloquial German name is dropped too.
        assert_eq!(dataset.places[1].localized, [(0, "マルセイユ".to_owned())]);
        assert!(dataset.places[2].localized.is_empty());
    }

    #[test]
    fn reads_no_alternate_names_without_languages() {
        let dataset = build(&[]).unwrap();
        assert!(dataset.places.iter().all(|p| p.localized.is_empty()));
    }

    #[test]
    fn rejects_malformed_input() {
        let run = |cities: &str| {
            from_geonames(
                cities.as_bytes(),
                &b""[..],
                &b""[..],
                None::<&[u8]>,
                &[],
                "x",
            )
            .unwrap_err()
        };
        let row = |id: &str, lat: &str, lon: &str, pop: &str| {
            format!("{id}\tA\tA\t\t{lat}\t{lon}\tP\tPPL\tFR\t\t\t\t\t\t{pop}\n")
        };
        assert_eq!(run("1\tA\n").detail, "2 columns, expected 15");
        assert_eq!(run(&row("x", "1", "1", "")).detail, "unparsable geonameid");
        assert_eq!(
            run(&row("1", "91", "1", "")).detail,
            "latitude outside ±90°"
        );
        assert_eq!(
            run(&row("1", "1", "180.5", "")).detail,
            "longitude outside ±180°"
        );
        assert_eq!(
            run(&row("1", "1", "NaN", "")).detail,
            "longitude outside ±180°"
        );
        assert_eq!(
            run(&row("1", "1", "1", "-5")).detail,
            "unparsable population"
        );
        let err = run(&format!("\n{}", row("1", "x", "1", "")));
        assert_eq!((err.file, err.line), ("cities", 2));
        assert_eq!(err.to_string(), "cities line 2: latitude outside ±90°");

        let bad_alternate = from_geonames(
            CITIES.as_bytes(),
            &b""[..],
            &b""[..],
            Some(&b"1\tx\tde\tName\n"[..]),
            &["de".to_owned()],
            "x",
        );
        assert_eq!(bad_alternate.unwrap_err().file, "alternate names");
        let bad_tag = from_geonames(
            CITIES.as_bytes(),
            &b""[..],
            &b""[..],
            None::<&[u8]>,
            &["not a tag".to_owned()],
            "x",
        );
        assert_eq!(bad_tag.unwrap_err().file, "languages");
    }

    #[test]
    fn recognises_language_tags() {
        for tag in ["en", "zh-Hant", "pt-BR", "es-419", "sgn-ase"] {
            assert!(is_language_tag(tag), "{tag}");
        }
        for tag in [
            "",
            "e",
            "en-",
            "-en",
            "en us",
            "e1",
            "toolonglang",
            "en-toolongsub",
        ] {
            assert!(!is_language_tag(tag), "{tag}");
        }
    }
}
