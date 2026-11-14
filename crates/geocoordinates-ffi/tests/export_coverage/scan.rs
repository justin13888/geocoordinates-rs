//! The source scanner behind `export_coverage.rs`: finds the FFI crate's
//! exports and reads test code, both through [`strip`], so a comment or a
//! string literal never counts as code.

use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A `#[uniffi::export]` free function, called as `name(`.
    Function,
    /// A `#[uniffi::constructor]` in an exported `impl`, called as `Owner::name(`.
    Constructor,
    /// A method of an exported `impl` or foreign trait, called as `.name(`.
    Method,
}

#[derive(Debug, Clone)]
pub struct Export {
    pub name: String,
    pub owner: Option<String>,
    pub kind: Kind,
    /// The doc comment's lines, `///` removed and trimmed.
    pub doc: Vec<String>,
    /// The parameter list, without its parentheses.
    pub params: String,
    /// The return type, empty for `()`.
    pub ret: String,
}

impl Export {
    /// The name a failure message shows: `Owner::name` for a method.
    pub fn label(&self) -> String {
        match &self.owner {
            Some(owner) => format!("{owner}::{}", self.name),
            None => self.name.clone(),
        }
    }
}

pub fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `src` with every comment, string literal and char literal replaced by
/// spaces. Newlines are kept, so line `n` of the result is line `n` of `src`.
pub fn strip(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let blank = |out: &mut String, s: &[char]| {
        out.extend(s.iter().map(|&ch| if ch == '\n' { '\n' } else { ' ' }));
    };
    let mut i = 0;
    while i < c.len() {
        // `b` before a literal is a prefix (`br"…"`), not part of an identifier.
        let prefix = i > 0 && c[i - 1] == 'b' && (i < 2 || !is_ident(c[i - 2]));
        let prev_ident = i > 0 && is_ident(c[i - 1]) && !prefix;
        let start = i;
        if c[i] == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if c[i] == '/' && c.get(i + 1) == Some(&'*') {
            let mut depth = 0;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if c[i] == 'r' && !prev_ident && matches!(c.get(i + 1).copied(), Some('"' | '#')) {
            // A raw string `r"…"` / `r#"…"#`; `r#ident` is a raw identifier.
            let hashes = c[i + 1..].iter().take_while(|&&ch| ch == '#').count();
            if c.get(i + 1 + hashes) != Some(&'"') {
                out.push(c[i]);
                i += 1;
                continue;
            }
            i += 2 + hashes;
            while i < c.len() {
                if c[i] == '"'
                    && c[i + 1..]
                        .iter()
                        .take(hashes)
                        .filter(|&&h| h == '#')
                        .count()
                        == hashes
                {
                    i += 1 + hashes;
                    break;
                }
                i += 1;
            }
        } else if c[i] == '"' {
            i += 1;
            while i < c.len() && c[i] != '"' {
                i += if c[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
        } else if c[i] == '\'' && (c.get(i + 1) == Some(&'\\') || c.get(i + 2) == Some(&'\'')) {
            // A char literal; a lifetime `'a` has no closing quote.
            i += 1;
            while i < c.len() && c[i] != '\'' {
                i += if c[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
        } else {
            out.push(c[i]);
            i += 1;
            continue;
        }
        let end = i.min(c.len());
        blank(&mut out, &c[start..end]);
    }
    out
}

/// The identifier at the start of `s`.
pub fn ident(s: &str) -> String {
    s.chars().take_while(|&c| is_ident(c)).collect()
}

/// The text inside the parentheses that open at the first `(` of `s`, and the
/// text after the closing one.
pub fn parens(s: &str) -> (&str, &str) {
    let open = s.find('(').expect("an opening parenthesis");
    let mut depth = 0;
    for (i, ch) in s[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return (&s[open + 1..open + i], &s[open + i + 1..]);
        }
    }
    panic!("unbalanced parentheses in {s:?}")
}

/// The contiguous doc-comment lines that end just above line `at`, skipping
/// attributes in between.
pub fn doc_above(lines: &[&str], at: usize) -> Vec<String> {
    let mut doc: Vec<String> = lines[..at]
        .iter()
        .rev()
        .map(|l| l.trim())
        .take_while(|l| l.starts_with("///") || l.starts_with("#["))
        .filter_map(|l| l.strip_prefix("///"))
        .map(|l| l.trim().to_string())
        .collect();
    doc.reverse();
    doc
}

/// The function whose signature starts on stripped line `at` (`pub fn` or
/// `fn`), with the doc comment above it.
pub fn function(raw: &[&str], code: &[&str], at: usize, owner: Option<&str>, kind: Kind) -> Export {
    let sig: String = code[at..]
        .iter()
        .scan(false, |done, l| {
            (!*done).then(|| {
                *done = l.contains('{') || l.trim_end().ends_with(';');
                format!("{l} ")
            })
        })
        .collect();
    let sig = sig.split('{').next().unwrap();
    let after_fn = &sig[sig.find("fn ").expect("a function") + 3..];
    let (params, tail) = parens(after_fn);
    let ret = tail
        .trim()
        .strip_prefix("->")
        .unwrap_or("")
        .split(" where ")
        .next()
        .unwrap()
        .trim()
        .trim_end_matches(';')
        .trim();
    Export {
        name: ident(after_fn.trim_start()),
        owner: owner.map(str::to_string),
        kind,
        doc: doc_above(raw, at),
        params: params.split_whitespace().collect::<Vec<_>>().join(" "),
        ret: ret.split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

/// Every export in `source`: each item under a `#[uniffi::export]` or
/// `#[uniffi::export(…)]` attribute, with blank lines, comments, doc lines and
/// other attributes allowed between the two. A free function is one export; an
/// exported `impl` contributes each `pub fn`, and an exported trait each `fn`.
pub fn exports(source: &str) -> Vec<Export> {
    let stripped = strip(source);
    let raw: Vec<&str> = source.lines().collect();
    let code: Vec<&str> = stripped.lines().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < code.len() {
        let attr = code[i].trim();
        let is_export = attr
            .strip_prefix("#[uniffi::export")
            .is_some_and(|rest| rest.starts_with(']') || rest.starts_with('('));
        i += 1;
        if !is_export {
            continue;
        }
        while i < code.len() && (code[i].trim().is_empty() || code[i].trim().starts_with("#[")) {
            i += 1;
        }
        let item = code[i].trim();
        let item = item.strip_prefix("pub ").unwrap_or(item);
        if item.starts_with("fn ") {
            found.push(function(&raw, &code, i, None, Kind::Function));
            continue;
        }
        let (owner, trait_item) = if let Some(rest) = item.strip_prefix("impl ") {
            (ident(rest), false)
        } else if let Some(rest) = item.strip_prefix("trait ") {
            (ident(rest), true)
        } else {
            panic!("unrecognised exported item: {item:?}");
        };
        // Walk the block, taking each function at depth 1.
        let mut depth = 0i32;
        let mut constructor = false;
        loop {
            let line = code[i].trim();
            if depth == 1 {
                if line.starts_with("#[uniffi::constructor") {
                    constructor = true;
                }
                let method = if trait_item {
                    line.starts_with("fn ")
                } else {
                    line.starts_with("pub fn ")
                };
                if method {
                    let kind = if constructor {
                        Kind::Constructor
                    } else {
                        Kind::Method
                    };
                    found.push(function(&raw, &code, i, Some(&owner), kind));
                    constructor = false;
                }
            }
            depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
            i += 1;
            if depth == 0 && line.contains('}') {
                break;
            }
        }
    }
    found
}

/// The type aliases in `sources` that resolve to a `Result`.
pub fn result_aliases(sources: &[String]) -> BTreeSet<String> {
    let aliases: Vec<(String, String)> = sources
        .iter()
        .flat_map(|s| {
            let code = strip(s);
            code.split([';', '{', '}'])
                .filter_map(|stmt| {
                    let stmt = stmt.trim();
                    let stmt = stmt.strip_prefix("pub ").unwrap_or(stmt);
                    let (lhs, rhs) = stmt.strip_prefix("type ")?.split_once('=')?;
                    Some((ident(lhs.trim()), rhs.trim().to_string()))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let mut fallible = BTreeSet::new();
    // Repeat until no alias is added, so an alias of an alias resolves.
    loop {
        let before = fallible.len();
        for (name, rhs) in &aliases {
            if is_result(rhs, &fallible) {
                fallible.insert(name.clone());
            }
        }
        if fallible.len() == before {
            return fallible;
        }
    }
}

/// Whether the type `ty` resolves to a `Result`, given the `Result` aliases.
pub fn is_result(ty: &str, aliases: &BTreeSet<String>) -> bool {
    let head = ty.split('<').next().unwrap_or("").trim();
    let last = head.rsplit("::").next().unwrap_or("").trim();
    last == "Result" || aliases.contains(last)
}

/// The value a caller reads from the return type `ty`: the first type argument
/// of a `Result`, an `Option` or a `Result` alias in `aliases` (unwrapped
/// again where it nests), else `ty` itself.
pub fn value_type<'a>(ty: &'a str, aliases: &BTreeSet<String>) -> &'a str {
    let ty = ty.trim();
    let Some((head, args)) = ty.split_once('<') else {
        return ty;
    };
    let last = head.rsplit("::").next().unwrap_or("").trim();
    if !(last == "Option" || is_result(head, aliases)) {
        return ty;
    }
    let mut depth = 0;
    for (i, ch) in args.char_indices() {
        match ch {
            '<' | '(' => depth += 1,
            '>' | ')' if depth > 0 => depth -= 1,
            ',' | '>' if depth == 0 => return value_type(&args[..i], aliases),
            _ => {}
        }
    }
    ty
}

/// Whether `doc` has an `# Errors` heading with at least one non-empty line
/// before the next heading.
pub fn documents_errors(doc: &[String]) -> bool {
    doc.iter().position(|l| l == "# Errors").is_some_and(|at| {
        doc[at + 1..]
            .iter()
            .take_while(|l| !l.starts_with('#'))
            .any(|l| !l.is_empty())
    })
}

/// Whether the stripped test code `code` calls `export`, other than in the
/// `fn` that defines a function of the same name.
pub fn calls(code: &str, export: &Export) -> bool {
    let needle = match (export.kind, &export.owner) {
        (Kind::Constructor, Some(owner)) => format!("{owner}::{}(", export.name),
        (Kind::Method, _) => format!(".{}(", export.name),
        _ => format!("{}(", export.name),
    };
    code.match_indices(&needle).any(|(i, _)| {
        let before = code[..i].trim_end();
        let bounded = export.kind == Kind::Method || !code[..i].ends_with(is_ident);
        bounded && !(before.ends_with("fn") && !before[..before.len() - 2].ends_with(is_ident))
    })
}

/// The name and body of every `fn` in the stripped code `code`.
pub fn fn_bodies(code: &str) -> Vec<(String, &str)> {
    let mut bodies = Vec::new();
    for (at, _) in code.match_indices("fn ") {
        if code[..at].ends_with(is_ident) {
            continue;
        }
        let Some(open) = code[at..].find('{').map(|o| at + o) else {
            continue;
        };
        if code[at..open].contains(';') {
            continue;
        }
        let mut depth = 0;
        for (i, ch) in code[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                bodies.push((ident(&code[at + 3..]), &code[open..open + i + 1]));
                break;
            }
        }
    }
    bodies
}

/// Whether `code` opens with a macro that fails the test, optionally inside a
/// block: the body of an arm or branch that rejects the value it matched.
fn fails_first(code: &str) -> bool {
    let code = code.trim_start();
    let code = code.strip_prefix('{').unwrap_or(code).trim_start();
    ["panic!", "unreachable!", "todo!", "unimplemented!"]
        .iter()
        .any(|m| code.starts_with(m))
}

/// Whether `body` checks an error: `is_err()`, `unwrap_err()`, `expect_err(…)`,
/// a `matches!(…, Err(…))`, an `Err(…) =>` match arm (or `if let Err(…)`, or
/// `let Err(…) = … else`), or a call to one of the `helpers` that does. An arm
/// or `if let` branch whose body opens with `panic!` / `unreachable!` (or
/// `todo!` / `unimplemented!`) rejects the error rather than checking it, so it
/// does not count.
pub fn checks_error(body: &str, helpers: &BTreeSet<String>) -> bool {
    body.contains(".is_err()")
        || body.contains(".unwrap_err()")
        || body.contains(".expect_err(")
        || body
            .match_indices("matches!(")
            .any(|(i, _)| parens(&body[i..]).0.contains("Err("))
        || body.match_indices("Err(").any(|(i, _)| {
            if body[..i].ends_with(is_ident) {
                return false;
            }
            let rest = parens(&body[i..]).1.trim_start();
            let before = body[..i].trim_end();
            if rest.starts_with("=>") || rest.starts_with("if ") {
                // A match arm: its body follows the (guarded) `=>`.
                rest.split_once("=>")
                    .is_some_and(|(_, arm)| !fails_first(arm))
            } else if before.ends_with("if let") || before.ends_with("while let") {
                // The branch body follows the scrutinee's opening `{`.
                rest.split_once('{')
                    .is_some_and(|(_, branch)| !fails_first(branch))
            } else {
                // `let Err(…) = … else { … }` fails the test on success.
                before.ends_with("let")
            }
        })
        || helpers.iter().any(|h| {
            body.match_indices(&format!("{h}("))
                .any(|(i, _)| !body[..i].ends_with(is_ident))
        })
}

/// The `fn`s in `bodies` that check an error themselves or through another
/// such `fn`, so a test calling one checks an error.
pub fn error_helpers(bodies: &[(String, &str)]) -> BTreeSet<String> {
    let mut helpers = BTreeSet::new();
    loop {
        let found: BTreeSet<String> = bodies
            .iter()
            .filter(|(_, body)| checks_error(body, &helpers))
            .map(|(name, _)| name.clone())
            .collect();
        if found.len() == helpers.len() {
            return helpers;
        }
        helpers = found;
    }
}

pub fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Every record with an `impl TryFrom<Record> for gc::…` in `sources`, with a
/// macro's `impl TryFrom<$t>` expanded over that macro's invocations.
pub fn validated_records(sources: &[String]) -> BTreeSet<String> {
    let mut records = BTreeSet::new();
    for source in sources {
        let code = strip(source);
        let mut macro_name = None;
        for line in code.lines() {
            if let Some(rest) = line.trim().strip_prefix("macro_rules! ") {
                macro_name = Some(ident(rest));
            }
            let Some((ty, target)) = line
                .trim()
                .strip_prefix("impl TryFrom<")
                .and_then(|rest| rest.split_once("> for "))
            else {
                continue;
            };
            if !target.starts_with("gc::") {
                continue;
            }
            if ty.starts_with('$') {
                let name = macro_name.clone().expect("a macro around `$t`");
                let invocation = format!("{name}!(");
                for (at, _) in code.match_indices(&invocation) {
                    let args = parens(&code[at..]).0;
                    records.extend(args.split(',').map(|a| a.trim().to_string()));
                }
            } else {
                records.insert(ident(ty));
            }
        }
    }
    records
}
