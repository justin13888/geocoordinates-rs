//! The markup pre-scan [`from_kml`](super::from_kml) runs before handing a
//! document to the `kml` parser, which neither bounds its recursion nor
//! returns on every malformed document.

use super::{syntax, too_deep};
use crate::error::Result;

/// The deepest element nesting a KML document may have. The `kml` parser
/// recurses once per element with no limit of its own and spends tens of
/// kilobytes of stack on each `<Folder>` level in an unoptimized build (about
/// 40 levels exhaust a 2 MiB thread stack), so the limit is 32 rather than
/// `MAX_WKT_DEPTH`'s 128. Real KML (`kml` / `Document` / a few `Folder`s /
/// `Placemark` / `MultiGeometry` / `Polygon` / boundary / `LinearRing` /
/// `coordinates`) stays well inside it.
const MAX_KML_DEPTH: usize = 32;

/// Reject KML markup the `kml` 0.13 parser cannot be trusted with: elements
/// nested deeper than [`MAX_KML_DEPTH`] (a `TooDeep` error), and markup that
/// is not closed (a `Syntax` error). The parser's placemark, geometry, and
/// `Data` / `SchemaData` readers loop until their own end tag and never stop at
/// the end of input, so `<kml><Point>` would spin forever; requiring every
/// element and construct to be closed before the parser runs means each of
/// those readers meets its end tag.
///
/// **Depth.** A start tag opens a level and an end tag closes one, except
/// `</kml>` (any prefix): `kml` 0.13 ignores it and keeps the `<kml>` reader
/// on its stack, so `<kml></kml>` repeated nests the parser one level per
/// repeat and each one counts as a level here. Self-closing tags, comments,
/// CDATA sections, processing instructions, and declarations open none.
///
/// **Closure.** Every end tag must name the innermost open element, exactly
/// as written (prefix included, trailing whitespace ignored), every element
/// must be closed by the end of input, and every construct must be
/// terminated.
///
/// Every construct ends exactly where `quick-xml` (the `kml` parser's
/// tokenizer) ends it, so no tag the parser reads is skipped or invented:
/// a `>` inside a quoted attribute value ends neither a start nor an end tag,
/// a comment ends at the first `-->` after `<!--` (so `<!-->` and `<!--->`
/// do not end one), a processing instruction or `<?xml …?>` declaration ends
/// at `?>`, and a `<!DOCTYPE …>` ends at the `>` that balances every `<`
/// inside it.
pub(in crate::parse) fn check_kml_markup(input: &str) -> Result<()> {
    /// The offset of the `>` ending the tag `tag` starts, skipping quotes.
    fn tag_end(tag: &str) -> Option<usize> {
        let mut quote = None;
        for (i, b) in tag.bytes().enumerate() {
            match (quote, b) {
                (None, b'"' | b'\'') => quote = Some(b),
                (Some(q), b) if b == q => quote = None,
                (None, b'>') => return Some(i),
                _ => {}
            }
        }
        None
    }
    /// The offset of the `>` ending the `<!DOCTYPE` that `decl` starts: the
    /// first `>` not balanced by a `<` inside it. Like `quick-xml` 0.37, it
    /// tracks neither quotes nor `[`: skipping a quoted `>` the parser counts
    /// would hide the start tags after it.
    fn doctype_end(decl: &str) -> Option<usize> {
        let mut open = 0usize;
        for (i, b) in decl.bytes().enumerate().skip(1) {
            match b {
                b'<' => open += 1,
                b'>' if open == 0 => return Some(i),
                b'>' => open -= 1,
                _ => {}
            }
        }
        None
    }
    /// The offset of `needle` in `s` at or after `from`.
    fn find_from(s: &str, from: usize, needle: &str) -> Option<usize> {
        s.get(from..)?.find(needle).map(|i| i + from)
    }
    /// `quick-xml`'s whitespace, which ends a start tag's name.
    fn is_space(c: char) -> bool {
        matches!(c, ' ' | '\t' | '\r' | '\n')
    }

    let mut depth = 0usize;
    // The names of the open elements, innermost last. Never longer than
    // `depth`, which only `</kml>` keeps from falling with it, so it holds at
    // most `MAX_KML_DEPTH` names.
    let mut open: Vec<&str> = Vec::new();
    let mut rest = input;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        // Resume at the construct's terminator, which holds no `<`, so the
        // next search starts past it.
        let end = if rest.starts_with("<!--") {
            find_from(rest, 4, "-->")
        } else if rest.starts_with("<![CDATA[") {
            rest.find("]]>")
        } else if rest.starts_with("<!D") || rest.starts_with("<!d") {
            doctype_end(rest)
        } else if rest.starts_with("<?") {
            find_from(rest, 1, "?>")
        } else if rest.starts_with("<!") {
            // Any other `<!` construct is a parse error at this point.
            rest.find('>')
        } else if rest.starts_with("</") {
            let end = tag_end(rest);
            if let Some(i) = end {
                let name = rest[2..i].trim_end_matches(is_space);
                match open.pop() {
                    Some(innermost) if innermost == name => {}
                    Some(innermost) => {
                        return Err(syntax(format!(
                            "kml: end tag </{name}> does not close <{innermost}>"
                        )));
                    }
                    None => {
                        return Err(syntax(format!("kml: end tag </{name}> closes nothing")));
                    }
                }
                // `kml`'s element-list reader returns only at `</Folder>`,
                // `</Document>`, or the end of input, so it ignores `</kml>`
                // and the reader the `<kml>` opened stays on the stack: that
                // level never closes.
                let local = name.split_once(':').map_or(name, |(_, local)| local);
                if local != "kml" {
                    depth = depth.saturating_sub(1);
                }
            }
            end
        } else {
            let end = tag_end(rest);
            if let Some(i) = end.filter(|&i| rest.as_bytes()[i - 1] != b'/') {
                depth += 1;
                if depth > MAX_KML_DEPTH {
                    return Err(too_deep("kml", MAX_KML_DEPTH));
                }
                open.push(rest[1..i].split(is_space).next().unwrap_or_default());
            }
            end
        };
        let Some(end) = end else {
            let at = input.len() - rest.len();
            return Err(syntax(format!(
                "kml: markup at byte {at} is not terminated"
            )));
        };
        // Every terminator lies past the `<` at offset 0; stepping at least
        // one byte keeps the scan advancing whatever a helper returns.
        rest = &rest[end.max(1)..];
    }
    match open.last() {
        Some(innermost) => Err(syntax(format!("kml: element <{innermost}> is not closed"))),
        None => Ok(()),
    }
}
