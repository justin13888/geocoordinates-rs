//! Receiver typing for `scan::calls`: whether a `.name(` method call in test
//! code is on a value of the method's owner, so that a test calling `Vec::len`
//! never counts as testing an exported `len`.
//!
//! The typing is local and syntactic. A receiver is of type `Owner` when it
//! is, after any number of `unwrap()` / `expect(…)` / `clone()` / `as_ref()`
//! calls or `?`:
//!
//! - a call `Owner::f(…)`;
//! - a call `f(…)` of a test helper or export whose return type names `Owner`;
//! - a parenthesised such expression;
//! - a name the enclosing `fn` binds to one: a parameter whose type names
//!   `Owner`, or a `let` whose type names it or whose value is such a receiver.

use super::scan::{Export, Kind, fn_bodies, ident, is_ident, parens};
use std::collections::{BTreeMap, BTreeSet};

/// What test code says about the types a receiver can have.
#[derive(Debug, Default)]
pub struct Types {
    /// The return types of the functions test code can call, by name: each
    /// test helper and each exported free function.
    returns: BTreeMap<String, Vec<String>>,
    /// The types test code implements each trait for (`impl Trait for T`).
    implementors: BTreeMap<String, BTreeSet<String>>,
}

impl Types {
    /// Whether the function `name` returns a type naming `owner`.
    fn returns(&self, name: &str, owner: &str) -> bool {
        self.returns
            .get(name)
            .is_some_and(|rets| rets.iter().any(|r| names(r, owner)))
    }
}

/// Calls that hand back the receiver they are called on, as far as its type
/// goes.
const PASSTHROUGH: &[&str] = &["unwrap", "expect", "clone", "as_ref"];

/// The [`Types`] the stripped test code `code` and the free-function exports
/// define.
pub fn types(code: &[String], exports: &[Export]) -> Types {
    let mut out = Types::default();
    for file in code {
        for (at, _) in file.match_indices("impl ") {
            let header = file[at + 5..].split('{').next().unwrap_or("");
            if let Some((tr, ty)) = header.split_once(" for ") {
                out.implementors
                    .entry(ident(tr.trim()))
                    .or_default()
                    .insert(ident(ty.trim()));
            }
        }
        for (name, text) in fn_bodies(file) {
            let after_name = &text[text.find('(').unwrap_or(0)..];
            let (_, tail) = parens(after_name);
            let ret = tail
                .split('{')
                .next()
                .unwrap_or("")
                .trim()
                .strip_prefix("->")
                .unwrap_or("")
                .trim();
            out.returns.entry(name).or_default().push(ret.to_string());
        }
    }
    for e in exports.iter().filter(|e| e.kind == Kind::Function) {
        out.returns
            .entry(e.name.clone())
            .or_default()
            .push(e.ret.clone());
    }
    out
}

/// Whether the type `ty` names `owner` as one of its identifiers.
fn names(ty: &str, owner: &str) -> bool {
    ty.split(|c: char| !is_ident(c)).any(|t| t == owner)
}

/// The identifier `s` ends with.
fn trailing_ident(s: &str) -> &str {
    let start = s
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_ident(c))
        .last()
        .map_or(s.len(), |(i, _)| i);
    &s[start..]
}

/// The index of the `(` that the `)` ending `s` closes.
fn opening(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, ch) in s.char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Whether the expression `s` ends with is of type `owner`, given the names
/// `typed` as `owner` in scope.
pub fn is_owned(s: &str, owner: &str, typed: &BTreeSet<String>, types: &Types) -> bool {
    let mut s = s.trim_end();
    loop {
        if let Some(rest) = s.strip_suffix('?') {
            s = rest.trim_end();
            continue;
        }
        if s.ends_with(')') {
            let Some(open) = opening(s) else {
                return false;
            };
            let head = s[..open].trim_end();
            let name = trailing_ident(head);
            if name.is_empty() {
                return is_owned(&s[open + 1..s.len() - 1], owner, typed, types);
            }
            let before = head[..head.len() - name.len()].trim_end();
            if let Some(chained) = before.strip_suffix('.') {
                if !PASSTHROUGH.contains(&name) {
                    return false;
                }
                s = chained.trim_end();
                continue;
            }
            if let Some(path) = before.strip_suffix("::") {
                return trailing_ident(path.trim_end()) == owner;
            }
            return types.returns(name, owner);
        }
        let name = trailing_ident(s);
        let before = s[..s.len() - name.len()].trim_end();
        return !name.is_empty()
            && !before.ends_with('.')
            && !before.ends_with("::")
            && typed.contains(name);
    }
}

/// The text of `s` up to its first `;` outside brackets.
fn statement(s: &str) -> &str {
    let mut depth = 0i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ';' if depth == 0 => return &s[..i],
            _ => {}
        }
        if depth < 0 {
            return &s[..i];
        }
    }
    s
}

/// The names the `fn` text `item` binds to a value of type `owner`.
pub fn typed_names(item: &str, owner: &str, types: &Types) -> BTreeSet<String> {
    let mut typed = BTreeSet::new();
    if let Some(open) = item.find('(') {
        for param in parens(&item[open..]).0.split(',') {
            if let Some((pat, ty)) = param.split_once(':') {
                let pat = pat.trim();
                if names(ty, owner) {
                    typed.insert(ident(pat.strip_prefix("mut ").unwrap_or(pat).trim()));
                }
            }
        }
    }
    loop {
        let before = typed.len();
        for (at, _) in item.match_indices("let ") {
            if item[..at].ends_with(is_ident) {
                continue;
            }
            let rest = item[at + 4..].trim_start();
            let rest = rest.strip_prefix("mut ").unwrap_or(rest);
            let name = ident(rest);
            let stmt = statement(rest[name.len()..].trim_start());
            let bound = if let Some(typed_let) = stmt.strip_prefix(':') {
                let (ty, value) = typed_let.split_once('=').unwrap_or((typed_let, ""));
                names(ty, owner) || is_owned(value, owner, &typed, types)
            } else if let Some(value) = stmt.strip_prefix('=') {
                is_owned(value, owner, &typed, types)
            } else {
                false
            };
            if !name.is_empty() && bound {
                typed.insert(name);
            }
        }
        if typed.len() == before {
            return typed;
        }
    }
}

/// Whether some `fn` in the stripped code `code` calls `.name(` on a receiver
/// of type `owner`.
pub fn method_calls(code: &str, owner: &str, name: &str, types: &Types) -> bool {
    let needle = format!(".{name}(");
    fn_bodies(code).iter().any(|(_, item)| {
        let typed = typed_names(item, owner, types);
        item.match_indices(&needle)
            .any(|(i, _)| is_owned(&item[..i], owner, &typed, types))
    })
}

/// `code` with the text of every `impl owner for T { … }` block blanked out, so
/// that an implementor's own methods never count as using it.
fn without_impls_of(code: &str, owner: &str) -> String {
    let mut out = code.to_string();
    for (at, _) in code.match_indices("impl ") {
        if code[..at].ends_with(is_ident) {
            continue;
        }
        let Some(open) = code[at..].find('{').map(|o| at + o) else {
            continue;
        };
        let is_owner = code[at + 5..open]
            .split_once(" for ")
            .is_some_and(|(tr, _)| ident(tr.trim()) == owner);
        if !is_owner {
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
                let end = open + i + 1;
                out.replace_range(at..end, &" ".repeat(end - at));
                break;
            }
        }
    }
    out
}

/// Whether the stripped code `code` passes a value of a type test code
/// implements the trait `owner` for as a call argument: inside the argument
/// list of a call in the body of a `fn` that is not one of the implementor's
/// own methods. A foreign-trait method runs when such a value is handed to an
/// export, the way foreign code hands one over; defining the implementor (its
/// `struct`, its `impl` header, a `fn` signature naming it) does not count.
pub fn uses_implementor(code: &str, owner: &str, types: &Types) -> bool {
    let Some(tys) = types.implementors.get(owner) else {
        return false;
    };
    let code = without_impls_of(code, owner);
    fn_bodies(&code).iter().any(|(_, item)| {
        let Some(open) = item.find('{') else {
            return false;
        };
        let body = &item[open..];
        let mut depth = 0i32;
        let mut token = String::new();
        for ch in body.chars() {
            if is_ident(ch) {
                token.push(ch);
                continue;
            }
            if depth > 0 && tys.contains(&token) {
                return true;
            }
            token.clear();
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }
        false
    })
}
