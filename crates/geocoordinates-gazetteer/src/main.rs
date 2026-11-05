//! `geocoordinates-gazetteer`: builds a gazetteer file from GeoNames dumps.
//!
//! ```text
//! geocoordinates-gazetteer --cities cities1000.txt --admin1 admin1CodesASCII.txt \
//!     --admin2 admin2Codes.txt --source-date 2026-11-03 --out cities1000.gaz \
//!     [--alternate-names alternateNamesV2.txt --languages de,ja,zh-Hant]
//! ```
//!
//! The dumps are the unzipped files from <https://download.geonames.org/export/dump/>.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::process::ExitCode;

use geocoordinates_gazetteer::{encode, from_geonames};

const USAGE: &str = "usage: geocoordinates-gazetteer --cities FILE --admin1 FILE --admin2 FILE \
--source-date YYYY-MM-DD --out FILE [--alternate-names FILE --languages TAG,TAG...]";

fn open(path: &str) -> Result<BufReader<File>, String> {
    File::open(path)
        .map(BufReader::new)
        .map_err(|e| format!("{path}: {e}"))
}

fn run() -> Result<(), String> {
    let mut args: HashMap<String, String> = HashMap::new();
    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        let Some(name) = flag.strip_prefix("--") else {
            return Err(format!("unexpected argument `{flag}`"));
        };
        let value = argv
            .next()
            .ok_or_else(|| format!("`{flag}` needs a value"))?;
        args.insert(name.to_owned(), value);
    }
    let required = |name: &str| {
        args.get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("missing --{name}"))
    };
    let languages: Vec<String> = args
        .get("languages")
        .map(|list| list.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    let alternate_names = args
        .get("alternate-names")
        .map(|path| open(path))
        .transpose()?;
    if languages.is_empty() != alternate_names.is_none() {
        return Err("--alternate-names and --languages go together".to_owned());
    }
    let dataset = from_geonames(
        open(required("cities")?)?,
        open(required("admin1")?)?,
        open(required("admin2")?)?,
        alternate_names,
        &languages,
        required("source-date")?,
    )
    .map_err(|e| e.to_string())?;
    let bytes = encode(&dataset);
    let out = required("out")?;
    std::fs::write(out, &bytes).map_err(|e| format!("{out}: {e}"))?;
    eprintln!(
        "{out}: {} places, {} admin1, {} admin2, {} languages, {} bytes",
        dataset.places.len(),
        dataset.admin1.len(),
        dataset.admin2.len(),
        dataset.languages.len(),
        bytes.len()
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("geocoordinates-gazetteer: {message}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
