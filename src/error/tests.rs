use super::*;

#[test]
fn outside_domain_names_the_operation_and_point() {
    let e = Error::OutsideDomain {
        operation: "UTM",
        lat: 85.5,
        lon: 10.0,
    };
    assert_eq!(
        e.to_string(),
        "coordinate outside the domain of UTM: lat=85.5, lon=10"
    );
}

#[test]
fn errors_compare_by_value_and_clone_equal() {
    let e = Error::CrsMismatch {
        expected: Crs::Wgs84,
        found: Crs::Gcj02,
    };
    assert_eq!(e.clone(), e);
    assert_ne!(
        e,
        Error::CrsMismatch {
            expected: Crs::Wgs84,
            found: Crs::Bd09,
        }
    );
    let parse = |kind| Error::Parse {
        kind,
        detail: "a".into(),
    };
    assert_ne!(
        parse(ParseErrorKind::Syntax),
        Error::InvalidGridRef {
            grid: "MGRS",
            detail: "a".into(),
        }
    );
    assert_ne!(parse(ParseErrorKind::Syntax), parse(ParseErrorKind::NoFix));
    // The kind is not in the message: `detail` alone is.
    assert_eq!(
        parse(ParseErrorKind::TooDeep).to_string(),
        "could not parse coordinate: a"
    );
}

/// Every non-test source file under `src/`, with its path and text. A file
/// is a test file when it is named `tests.rs` or `*_tests.rs`, or sits in a
/// `tests` directory.
fn library_sources() -> Vec<(std::path::PathBuf, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).expect("readable directory") {
            let path = entry.expect("directory entry").path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                if name != "tests" {
                    walk(&path, out);
                }
            } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
                let text = std::fs::read_to_string(&path).expect("readable source");
                out.push((path, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut out,
    );
    assert!(out.len() > 50, "the walk found only {} files", out.len());
    out
}

/// Each string literal written right after `key` (as in `field: "x"`),
/// with the file and line it appears on.
fn literals_after(key: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (path, text) in library_sources() {
        for (n, line) in text.lines().enumerate() {
            let mut rest = line;
            while let Some(at) = rest.find(key) {
                rest = &rest[at + key.len()..];
                if let Some(value) = rest.split('"').next().filter(|_| rest.contains('"')) {
                    found.push((value.to_owned(), format!("{}:{}", path.display(), n + 1)));
                }
            }
        }
    }
    found
}

/// The argument list `text` opens with, its `(` already consumed, split
/// at its top-level commas, or `None` when it never closes. A string
/// literal is skipped whole, so a comma or bracket inside one is not read.
fn arguments(text: &str) -> Option<Vec<&str>> {
    let (mut depth, mut start, mut in_string, mut escaped) = (1_usize, 0, false, false);
    let mut args = Vec::new();
    for (i, c) in text.char_indices() {
        if in_string {
            (in_string, escaped) = (escaped || c != '"', !escaped && c == '\\');
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ',' if depth == 1 => {
                args.push(&text[start..i]);
                start = i + 1;
            }
            ')' | ']' | '}' if depth == 1 => {
                args.push(&text[start..i]);
                args.retain(|a| !a.trim().is_empty());
                return Some(args);
            }
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Each string literal passed for the parameter named `param` (as in
/// `field: &'static str`) of a library function, with the file and byte
/// offset of the call: the producers that hand a name to a helper rather
/// than write `field: "x"` inline. A call resolves to the helper of that
/// name in its own file, or else to one declared `pub` in another.
fn helper_arguments(param: &str) -> Vec<(String, String)> {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let sources = library_sources();
    let wanted = format!("{param}: &'static str");
    // (file index, name, parameter position, takes `self`, `pub`)
    let mut helpers = Vec::new();
    for (file, (_, text)) in sources.iter().enumerate() {
        for (at, _) in text.match_indices("fn ") {
            let rest = &text[at + 3..];
            let name: String = rest.chars().take_while(|&c| is_ident(c)).collect();
            let params = rest[name.len()..].strip_prefix('(').and_then(arguments);
            let params = params.unwrap_or_default();
            if let Some(index) = params.iter().position(|p| p.trim() == wanted) {
                let method = params[0].trim().ends_with("self");
                let shared = text[..at]
                    .rsplit('\n')
                    .next()
                    .is_some_and(|l| l.contains("pub"));
                helpers.push((file, name, index, method, shared));
            }
        }
    }
    let mut found = Vec::new();
    for (file, (path, text)) in sources.iter().enumerate() {
        for (home, name, index, method, shared) in &helpers {
            let local = helpers.iter().any(|h| h.0 == file && h.1 == *name);
            if *home != file && (!shared || local) {
                continue;
            }
            for (at, _) in text.match_indices(&format!("{name}(")) {
                let before = text[..at].chars().next_back().unwrap_or(' ');
                // A definition, a longer name, or another type's method.
                if is_ident(before) || text[..at].ends_with("fn ") || before == '.' && !method {
                    continue;
                }
                let args = arguments(&text[at + name.len() + 1..]).unwrap_or_default();
                let arg = args.get(index - usize::from(before == '.'));
                if let Some(literal) = arg.and_then(|a| a.trim().strip_prefix('"')) {
                    let value = literal.split('"').next().unwrap_or_default();
                    found.push((value.to_owned(), format!("{}@{at}", path.display())));
                }
            }
        }
    }
    found
}

fn is_snake_case(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[test]
fn snake_case_check_accepts_only_identifiers() {
    for good in ["value", "semi_major", "x_m", "level2"] {
        assert!(is_snake_case(good), "{good}");
    }
    for bad in [
        "",
        "S2 level",
        "decimal degrees",
        "Value",
        "2d",
        "_x",
        "semi-major",
    ] {
        assert!(!is_snake_case(bad), "{bad}");
    }
}

#[test]
fn invalid_value_fields_are_snake_case_names() {
    let fields = literals_after("field: \"");
    // The scan sees the producers and the doctests that match on them.
    assert!(
        fields.iter().any(|(v, _)| v == "minutes"),
        "the scan found no `field:` literal"
    );
    let bad: Vec<_> = fields.iter().filter(|(v, _)| !is_snake_case(v)).collect();
    assert!(bad.is_empty(), "`field` values not in snake_case: {bad:?}");
}

/// A `field` name a producer hands to a helper (`checked_decimals(..,
/// "decimals")`) is held to the same rule as one written inline.
#[test]
fn invalid_value_fields_include_helper_arguments() {
    let fields = helper_arguments("field");
    // A private helper (`fix.rs`), a crate-wide one called from another
    // file with a one-letter field (`finite`, `baidu_mercator.rs`), and
    // another crate-wide one (`ellipsoid_of_argument`).
    for expected in ["horizontal", "minute_decimals", "x", "from_crs"] {
        assert!(
            fields.iter().any(|(v, _)| v == expected),
            "the helper scan missed {expected:?}: {fields:?}"
        );
    }
    let bad: Vec<_> = fields.iter().filter(|(v, _)| !is_snake_case(v)).collect();
    assert!(bad.is_empty(), "`field` values not in snake_case: {bad:?}");
}

/// A `field` naming a parameter is that parameter's exact name, unit
/// suffix included: wherever `finite` is handed a bare identifier, it
/// reports that identifier (`finite(east_m, "east_m")`, not `"east"`).
#[test]
fn finite_reports_the_exact_name_of_the_value_it_checks() {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let (mut checked, mut bad) = (Vec::new(), Vec::new());
    for (path, text) in library_sources() {
        for (at, _) in text.match_indices("finite(") {
            let before = text[..at].chars().next_back().unwrap_or(' ');
            // `is_finite(`, a method, or the definition itself.
            if is_ident(before) || before == '.' || text[..at].ends_with("fn ") {
                continue;
            }
            let args = arguments(&text[at + "finite(".len()..]).unwrap_or_default();
            let [value, field] = args.as_slice() else {
                continue;
            };
            let value = value.trim();
            let Some(field) = field
                .trim()
                .strip_prefix('"')
                .and_then(|f| f.strip_suffix('"'))
            else {
                continue;
            };
            if !value.chars().all(is_ident) {
                continue;
            }
            checked.push(field.to_owned());
            if value != field {
                bad.push(format!(
                    "{}@{at}: finite({value}, {field:?})",
                    path.display()
                ));
            }
        }
    }
    for expected in [
        "east_m",
        "down_m",
        "azimuth_deg",
        "bearing_deg",
        "bearing_b_deg",
    ] {
        assert!(
            checked.iter().any(|f| f == expected),
            "the scan missed {expected:?}: {checked:?}"
        );
    }
    assert!(
        bad.is_empty(),
        "`field` is not the checked value's name: {bad:?}"
    );
}

/// `InvalidGridRef` / `InvalidCellId` name their grid by the stable names
/// the variants document, inline or through `Error::invalid_grid_ref`.
#[test]
fn grid_names_are_the_documented_names() {
    const GRID_REF: [&str; 5] = ["MGRS", "geohash", "Plus Code", "Maidenhead", "quadkey"];
    const CELL_ID: [&str; 2] = ["H3", "S2"];
    // Inline, either variant; through the helper, `InvalidGridRef` only.
    for (found, sentinel, cell_ids) in [
        (literals_after("grid: \""), "H3", true),
        (helper_arguments("grid"), "Plus Code", false),
    ] {
        assert!(found.iter().any(|(v, _)| v == sentinel), "{found:?}");
        let documented = |v: &str| GRID_REF.contains(&v) || cell_ids && CELL_ID.contains(&v);
        let bad: Vec<_> = found.iter().filter(|(v, _)| !documented(v)).collect();
        assert!(bad.is_empty(), "undocumented `grid` values: {bad:?}");
    }
}

#[test]
fn outside_domain_operations_are_the_documented_names() {
    const DOCUMENTED: [&str; 5] = ["UTM", "UPS", "Baidu Mercator", "BD-09", "BD-09 inverse"];
    let mut operations = literals_after("operation: \"");
    assert!(
        operations.iter().any(|(v, _)| v == "UTM"),
        "the scan found no `operation:` literal"
    );
    // `bd09.rs` names its operation through `within_poles`.
    let through_helper = helper_arguments("operation");
    assert!(
        through_helper.iter().any(|(v, _)| v == "BD-09 inverse"),
        "the helper scan found no `operation` argument"
    );
    operations.extend(through_helper);
    let bad: Vec<_> = operations
        .iter()
        .filter(|(v, _)| !DOCUMENTED.contains(&v.as_str()))
        .collect();
    assert!(bad.is_empty(), "undocumented `operation` values: {bad:?}");
}

#[cfg(feature = "serde")]
#[test]
fn parse_error_kind_round_trips_through_serde() {
    for kind in [
        ParseErrorKind::Syntax,
        ParseErrorKind::Checksum,
        ParseErrorKind::NoFix,
        ParseErrorKind::Ambiguous,
        ParseErrorKind::Unsupported,
        ParseErrorKind::TooDeep,
    ] {
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(serde_json::from_str::<ParseErrorKind>(&json).unwrap(), kind);
    }
    assert_eq!(
        serde_json::to_string(&ParseErrorKind::NoFix).unwrap(),
        r#""NoFix""#
    );
}
