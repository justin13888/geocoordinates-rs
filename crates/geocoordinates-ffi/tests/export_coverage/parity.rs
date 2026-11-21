//! The parity half of `export_coverage.rs`: reads the core crate's public API
//! snapshots (`public-api/geocoordinates/*.txt`) and the FFI crate's exports
//! and record fields, and names each core function or constant no FFI form
//! reaches.

use super::scan::{Export, Kind, ident, snake_case, strip};
use std::collections::{BTreeMap, BTreeSet};

/// A function or constant the core crate exports, outside any trait.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApiItem {
    /// The owning type for an associated item, `None` for a module item.
    pub owner: Option<String>,
    pub name: String,
}

impl ApiItem {
    /// `Owner::name`, or the bare name of a module item, as the allow-list
    /// spells it.
    pub fn label(&self) -> String {
        match &self.owner {
            Some(owner) => format!("{owner}::{}", self.name),
            None => self.name.clone(),
        }
    }
}

/// Every inherent or free `fn` and every `const` in a `cargo public-api`
/// listing, skipping the items of a trait (a trait impl's methods, a trait
/// declaration's own items), fields and variants, and the `prelude`, which only
/// re-exports.
pub fn api_items(listing: &str) -> Vec<ApiItem> {
    let mut items = Vec::new();
    // The path whose items are a trait's: a trait impl's self type, or a
    // trait declaration.
    let mut skip: Option<String> = None;
    let bare = |path: &str| format!("{}::", path.trim().trim_end_matches(':'));
    for line in listing.lines() {
        let line = without_generics(line.trim());
        if line.starts_with("impl") {
            skip = line
                .rsplit_once(" for ")
                .and_then(|(_, ty)| ty.split_whitespace().next())
                .map(bare);
            continue;
        }
        // `#[non_exhaustive] pub struct …` and the like.
        let line = line
            .find("] pub ")
            .map_or(line.as_str(), |at| &line[at + 2..]);
        let Some(line) = line.strip_prefix("pub ") else {
            continue;
        };
        let words: Vec<&str> = line.split_whitespace().collect();
        // `const fn`, `unsafe fn`: the qualifiers before the item kind.
        let mut k = 0;
        while k + 1 < words.len()
            && matches!(words[k], "const" | "unsafe" | "async")
            && matches!(words[k + 1], "fn" | "const" | "unsafe" | "async")
        {
            k += 1;
        }
        let path = words.get(k + 1).copied().unwrap_or("");
        let path = &path[..path.find('(').unwrap_or(path.len())];
        match words.get(k).copied().unwrap_or("") {
            "trait" => skip = Some(bare(path)),
            "fn" | "const" if !skip.as_ref().is_some_and(|s| path.starts_with(s.as_str())) => {
                items.extend(item(path));
            }
            _ => {}
        }
    }
    items
}

/// `line` with every `<…>` generic list removed (an `->` arrow kept).
fn without_generics(line: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    let mut prev = ' ';
    for ch in line.chars() {
        match ch {
            '<' => depth += 1,
            '>' if depth > 0 && prev != '-' => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
        prev = ch;
    }
    out
}

/// The item a `geocoordinates::…` path names; `None` outside the crate or in
/// the `prelude`.
fn item(path: &str) -> Option<ApiItem> {
    let path = path.strip_prefix("geocoordinates::")?;
    let segments: Vec<&str> = path
        .trim_end_matches(':')
        .split("::")
        .filter(|s| !s.is_empty())
        .collect();
    if segments.first() == Some(&"prelude") {
        return None;
    }
    let name = ident(segments.last()?);
    let owner = segments
        .len()
        .checked_sub(2)
        .map(|at| segments[at])
        .filter(|s| s.starts_with(char::is_uppercase))
        .map(str::to_string);
    Some(ApiItem { owner, name })
}

/// The `pub` field names of every `pub struct` in `sources`, by struct name.
pub fn record_fields(sources: &[String]) -> BTreeMap<String, BTreeSet<String>> {
    let mut fields: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for source in sources {
        let code = strip(source);
        for (at, _) in code.match_indices("pub struct ") {
            let rest = &code[at + "pub struct ".len()..];
            let name = ident(rest);
            let Some(open) = rest.find(['{', ';', '(']) else {
                continue;
            };
            if !rest[open..].starts_with('{') {
                continue;
            }
            let body = &rest[open..];
            let close = body.find('}').unwrap_or(body.len());
            let entry = fields.entry(name).or_default();
            for (f, _) in body[..close].match_indices("pub ") {
                let field = ident(&body[f + 4..]);
                if body[f + 4 + field.len()..].trim_start().starts_with(':') {
                    entry.insert(field);
                }
            }
        }
    }
    fields
}

/// The snake_case prefix the FFI flattens a core type's methods under: the
/// type's own name, but a cell id by its grid (`README.md#naming`).
fn flat_owner(owner: &str) -> String {
    match owner {
        "H3Cell" => "h3".to_string(),
        "S2CellId" => "s2".to_string(),
        _ => snake_case(owner),
    }
}

/// The unit suffixes an FFI name may add to the core name it mirrors.
const UNIT_SUFFIXES: &[&str] = &["", "_m", "_deg", "_ms"];

/// Whether some FFI form reaches `item`, by name alone:
///
/// - an export named as the item, or flattened as `<owner>_<name>`, with a
///   leading `try_` dropped, a constant's name lower-cased, and a unit suffix
///   allowed (`geodesic_distance` → `geodesic_distance_m`);
/// - a method or constructor of the same name on the exported object `owner`;
/// - for `new`, the mirror record's `<owner>_validate`;
/// - for a getter or a `with_*` builder, a field of the mirror record of that
///   name, with or without a unit suffix.
///
/// A capability the FFI reaches under another name is in
/// `parity_gaps::RENAMED`, not here.
pub fn reaches(
    item: &ApiItem,
    exports: &[Export],
    fields: &BTreeMap<String, BTreeSet<String>>,
) -> bool {
    let names: BTreeSet<&str> = exports.iter().map(|e| e.name.as_str()).collect();
    let name = item.name.strip_prefix("try_").unwrap_or(&item.name);
    let name = name.to_lowercase();
    let named = |base: &str| {
        UNIT_SUFFIXES
            .iter()
            .any(|s| names.contains(format!("{base}{s}").as_str()))
    };
    let Some(owner) = &item.owner else {
        return named(&name);
    };
    let flat = flat_owner(owner);
    if named(&format!("{flat}_{name}")) {
        return true;
    }
    if exports.iter().any(|e| {
        e.owner.as_deref() == Some(owner) && e.name == item.name && e.kind != Kind::Function
    }) {
        return true;
    }
    if item.name == "new" && names.contains(format!("{flat}_validate").as_str()) {
        return true;
    }
    let field = name.strip_prefix("with_").unwrap_or(&name);
    let field = UNIT_SUFFIXES
        .iter()
        .filter(|s| !s.is_empty())
        .find_map(|s| field.strip_suffix(s))
        .unwrap_or(field);
    fields.get(owner).is_some_and(|f| {
        UNIT_SUFFIXES
            .iter()
            .any(|s| f.contains(&format!("{field}{s}")))
    })
}
