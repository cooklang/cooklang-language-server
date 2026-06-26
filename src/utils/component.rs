//! Shared scanning helpers for Cooklang components (`@` ingredients,
//! `#` cookware, `~` timers).
//!
//! Cooklang names follow two rules depending on whether a `{...}` group is
//! attached:
//!
//! * **Single word** – when no `{` is reachable, the name is the run of
//!   alphanumeric/underscore characters right after the marker. For example
//!   `@sun-dried` is just `sun` and `@flour2` is `flour2`.
//! * **Multi word** – when a `{` is reachable before any newline or another
//!   component marker (`@`, `#`, `~`), the name spans every character from the
//!   marker up to the `{`, and the whole component runs through the matching
//!   `}`. For example `@heavy whipping cream{1%cup}` is the single ingredient
//!   `heavy whipping cream`.
//!
//! The lookahead must stop at the next marker so that `@multi word #tool{}`
//! parses as the single-word ingredient `multi` plus the cookware `tool`,
//! exactly like the Cooklang parser.

/// Returns the byte offset (exclusive) where the component starting at
/// `marker_offset` ends. `marker_offset` must point at a `@`, `#` or `~`
/// character (all single-byte ASCII).
pub fn component_end(content: &str, marker_offset: usize) -> usize {
    let name_start = marker_offset + 1; // markers are 1-byte ASCII
    let rest = &content[name_start..];

    // Is there a `{` reachable before a newline or another component marker?
    let mut brace_offset = None;
    for (i, ch) in rest.char_indices() {
        match ch {
            '{' => {
                brace_offset = Some(name_start + i);
                break;
            }
            '\n' | '\r' | '@' | '#' | '~' => break,
            _ => {}
        }
    }

    if let Some(brace_offset) = brace_offset {
        // Multi-word name (or single word with quantity): consume through the
        // matching `}`. If the brace is never closed, end at the line break.
        return match content[brace_offset..].find('}') {
            Some(rel) => brace_offset + rel + 1,
            None => content[brace_offset..]
                .find(['\n', '\r'])
                .map(|rel| brace_offset + rel)
                .unwrap_or(content.len()),
        };
    }

    // Single-word name: the run of alphanumeric/underscore characters.
    let mut end = name_start;
    for (i, ch) in rest.char_indices() {
        if ch.is_alphanumeric() || ch == '_' {
            end = name_start + i + ch.len_utf8();
        } else {
            break;
        }
    }
    end
}

#[cfg(test)]
mod tests {
    use super::component_end;

    /// Helper: returns the component substring starting at `marker_offset`.
    fn component(content: &str, marker_offset: usize) -> &str {
        &content[marker_offset..component_end(content, marker_offset)]
    }

    #[test]
    fn multi_word_with_quantity() {
        let s = "Chill @heavy whipping cream{1%cup}.";
        let at = s.find('@').unwrap();
        assert_eq!(component(s, at), "@heavy whipping cream{1%cup}");
    }

    #[test]
    fn multi_word_with_apostrophe() {
        let s = "Add @lady's fingers{20}.";
        let at = s.find('@').unwrap();
        assert_eq!(component(s, at), "@lady's fingers{20}");
    }

    #[test]
    fn multi_word_with_hyphen() {
        let s = "Use @sun-dried tomatoes{1/2%cup}.";
        let at = s.find('@').unwrap();
        assert_eq!(component(s, at), "@sun-dried tomatoes{1/2%cup}");
    }

    #[test]
    fn single_word_no_braces_stops_at_space() {
        let s = "Add @heavy whipping cream then bake.";
        let at = s.find('@').unwrap();
        assert_eq!(component(s, at), "@heavy");
    }

    #[test]
    fn single_word_stops_at_hyphen() {
        let s = "@sun-dried tomato.";
        assert_eq!(component(s, 0), "@sun");
    }

    #[test]
    fn single_word_keeps_trailing_digits() {
        let s = "@flour2 mix.";
        assert_eq!(component(s, 0), "@flour2");
    }

    #[test]
    fn lookahead_stops_at_next_marker() {
        // The `{}` belongs to `#tool`, so `@multi` is single-word.
        let s = "@multi word #tool{} end.";
        let at = s.find('@').unwrap();
        assert_eq!(component(s, at), "@multi");
        let hash = s.find('#').unwrap();
        assert_eq!(component(s, hash), "#tool{}");
    }

    #[test]
    fn two_ingredients_only_second_has_braces() {
        let s = "Add @salt and @pepper{}.";
        let first = s.find('@').unwrap();
        assert_eq!(component(s, first), "@salt");
        let second = s[first + 1..].find('@').unwrap() + first + 1;
        assert_eq!(component(s, second), "@pepper{}");
    }

    #[test]
    fn single_word_with_braces() {
        let s = "@flour{200%g} more.";
        assert_eq!(component(s, 0), "@flour{200%g}");
    }

    #[test]
    fn empty_name_with_braces() {
        // Timer style `~{10%min}`.
        let s = "~{10%min}";
        assert_eq!(component(s, 0), "~{10%min}");
    }

    #[test]
    fn unclosed_brace_stops_at_newline() {
        let s = "@heavy cream{1%cup\nnext line";
        let at = s.find('@').unwrap();
        assert_eq!(component(s, at), "@heavy cream{1%cup");
    }

    #[test]
    fn marker_at_end_of_input() {
        let s = "Add @";
        let at = s.find('@').unwrap();
        assert_eq!(component(s, at), "@");
    }
}
