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
/// terminated. Every `<!` construct must be a comment (`<!--`), a CDATA
/// section (`<![CDATA[`), or a named `<!DOCTYPE` (any case): `quick-xml`
/// errors on any other, such as `<!x>` or `<!ELEMENT x>`, and the `Data` /
/// `SchemaData` readers swallow that error and spin on the end of input.
///
/// Every construct ends exactly where `quick-xml` (the `kml` parser's
/// tokenizer) ends it, so no tag the parser reads is skipped or invented:
/// a `>` inside a quoted attribute value ends neither a start nor an end tag,
/// a comment ends at the first `-->` after `<!--` (so `<!-->` and `<!--->`
/// do not end one), a processing instruction or `<?xml …?>` declaration ends
/// at the first `?>` after `<?` (so `<?>` is not terminated), and a `<!DOCTYPE …>` ends at the `>` that balances every `<`
/// inside it.
///
/// **Points.** Every `Point` element (any prefix, not self-closing) must hold
/// a coordinate, or the scan returns a `Syntax` error: `kml` 0.13 builds a
/// `Point` from the first position its geometry reader collected and panics
/// when there is none. That reader runs from the `<Point>` to the first end
/// tag named `Point` (any prefix), so a nested `<Point>…</Point>` ends the
/// outer one early, and keeps only the last `<coordinates>` start tag (any
/// prefix, at any depth) inside that span, taking the event after it: text,
/// which holds a coordinate when anything but whitespace remains once its
/// references are unescaped as `quick-xml` unescapes them (where that fails,
/// `kml` keeps the raw text, `&` included), or a CDATA section, which holds
/// one when anything but
/// whitespace lies in it. Each `Point` is held to its own span, whatever the
/// element around it, so a coordinate-less `Point` the parser would skip is
/// rejected too.
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
    /// A tag name without its namespace prefix.
    fn local(name: &str) -> &str {
        name.split_once(':').map_or(name, |(_, local)| local)
    }
    /// The character `quick-xml` 0.37's `unescape` puts in place of the
    /// numeric reference named `name` (between `&` and `;`): `#` then
    /// decimal, or `#x` then hex. `None` for any other name, or where
    /// unescaping fails. Only a whitespace result matters to the caller, so
    /// a named entity (one of the five predefined ones, none whitespace, as
    /// `kml` builds no `escape-html` table; any other fails) and a failed
    /// reference to U+0000 (not whitespace either) need no case of their
    /// own. A leading `+`, which `quick-xml` rejects and `from_str_radix`
    /// accepts, does.
    fn numeric_reference(name: &str) -> Option<char> {
        let number = name.strip_prefix('#')?;
        let (digits, radix) = number
            .strip_prefix('x')
            .map_or((number, 10), |hex| (hex, 16));
        if digits.starts_with('+') {
            return None;
        }
        u32::from_str_radix(digits, radix)
            .ok()
            .and_then(char::from_u32)
    }
    /// Whether the text `kml`'s `read_str` returns for `text` holds a
    /// character that is not Unicode whitespace, on which `kml` splits it
    /// into positions. `read_str` unescapes the text as `quick-xml` does,
    /// ending each reference at the next `&` or `;` after its `&` (it must
    /// be a `;`); where unescaping fails it returns the raw text, whose `&`
    /// is not whitespace.
    fn unescapes_to_position(text: &str) -> bool {
        let mut rest = text;
        while let Some(amp) = rest.find('&') {
            if rest[..amp].chars().any(|c| !c.is_whitespace()) {
                return true;
            }
            let reference = &rest[amp + 1..];
            let Some(end) = reference
                .find(['&', ';'])
                .filter(|&i| reference.as_bytes()[i] == b';')
            else {
                return true;
            };
            if numeric_reference(&reference[..end]).is_none_or(|c| !c.is_whitespace()) {
                return true;
            }
            rest = &reference[end + 1..];
        }
        rest.chars().any(|c| !c.is_whitespace())
    }
    /// Whether the content `after` a `<coordinates>` start tag holds a
    /// position for `kml`'s `read_str`, which reads only the next event and
    /// trims `quick-xml` whitespace from text. Text holds one when its
    /// unescaped form has a character that is not whitespace (so `&#49;`
    /// does, and `&#160;` does not). Any other next event yields no
    /// position, or an error, which rejecting here also turns into an error.
    fn holds_position(after: &str) -> bool {
        let text = &after[..after.find('<').unwrap_or(after.len())];
        if !text.trim_matches(is_space).is_empty() {
            return unescapes_to_position(text);
        }
        after[text.len()..]
            .strip_prefix("<![CDATA[")
            .and_then(|cdata| cdata.find("]]>").map(|end| &cdata[..end]))
            .is_some_and(|cdata| cdata.chars().any(|c| !c.is_whitespace()))
    }

    let mut depth = 0usize;
    // The names of the open elements, innermost last. Never longer than
    // `depth`, which only `</kml>` keeps from falling with it, so it holds at
    // most `MAX_KML_DEPTH` names.
    let mut open: Vec<&str> = Vec::new();
    // The byte offset of each `<Point>` whose geometry reader is still
    // running, and whether the last `<coordinates>` in its span so far holds
    // a position. Every entry is an open element, so it too holds at most
    // `MAX_KML_DEPTH`.
    let mut points: Vec<(usize, bool)> = Vec::new();
    let mut rest = input;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        let at = input.len() - rest.len();
        // Resume at the construct's terminator, which holds no `<`, so the
        // next search starts past it.
        let end = if rest.starts_with("<!--") {
            find_from(rest, 4, "-->")
        } else if rest.starts_with("<![CDATA[") {
            rest.find("]]>")
        } else if rest
            .get(..9)
            .is_some_and(|s| s.eq_ignore_ascii_case("<!DOCTYPE"))
        {
            let end = doctype_end(rest);
            // `quick-xml` rejects a DOCTYPE with nothing but whitespace
            // before its `>`.
            if end.is_some_and(|i| rest[9..i].trim_start_matches(is_space).is_empty()) {
                return Err(syntax(format!("kml: DOCTYPE at byte {at} has no name")));
            }
            end
        } else if rest.starts_with("<?") {
            // `quick-xml` reports `<?>` as an unclosed processing
            // instruction: its opening `?` never doubles as the one in `?>`.
            // Rejecting it outright, rather than reading on to a later `?>`,
            // also catches `<?>…?>`, which `quick-xml` errors on at the `<?>`.
            if rest.starts_with("<?>") {
                None
            } else {
                find_from(rest, 2, "?>")
            }
        } else if rest.starts_with("<!") {
            // `quick-xml` errors on every other `<!` construct, and the
            // `Data` / `SchemaData` readers swallow that error and then spin
            // on `Eof`, so it is rejected here whether or not it is closed.
            return Err(syntax(format!(
                "kml: markup at byte {at} is not a comment, CDATA section, or DOCTYPE"
            )));
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
                match local(name) {
                    "kml" => {}
                    "Point" => {
                        // The first `Point` end tag ends every running reader.
                        if let Some(&(point, _)) = points.iter().find(|&&(_, held)| !held) {
                            return Err(syntax(format!(
                                "kml: <Point> at byte {point} has no coordinates"
                            )));
                        }
                        points.clear();
                        depth = depth.saturating_sub(1);
                    }
                    _ => depth = depth.saturating_sub(1),
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
                let name = rest[1..i].split(is_space).next().unwrap_or_default();
                match local(name) {
                    "Point" => points.push((at, false)),
                    "coordinates" => {
                        let held = holds_position(&rest[i + 1..]);
                        points.iter_mut().for_each(|(_, h)| *h = held);
                    }
                    _ => {}
                }
                open.push(name);
            }
            end
        };
        let Some(end) = end else {
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
