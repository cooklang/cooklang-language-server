//! Locating Cooklang components (`@` ingredients, `#` cookware, `~` timers)
//! in a document.
//!
//! Rather than re-implementing the Cooklang grammar, this delegates to the
//! upstream parser ([`cooklang::parser::PullParser`]). Each component event
//! carries a [`Span`] covering the whole component — marker, name, `{quantity}`
//! and any `(note)` — so highlighting and hover always agree with how the
//! recipe actually parses (multi-word names, modifiers, references, aliases,
//! escapes and comments included).

use cooklang::parser::{Event, PullParser};
use cooklang::{Extensions, Span};

/// The kind of a located component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentKind {
    Ingredient,
    Cookware,
    Timer,
}

/// A component found in the source, with its byte span and parsed name.
#[derive(Debug, Clone)]
pub struct Component {
    pub kind: ComponentKind,
    /// Byte range of the whole component (marker through quantity/note).
    pub span: Span,
    /// Parsed, trimmed name. Empty for an unnamed timer (e.g. `~{10%min}`).
    pub name: String,
}

/// Scans `content` with the Cooklang parser and returns every ingredient,
/// cookware and timer in source order, each with its byte span.
pub fn scan_components(content: &str) -> Vec<Component> {
    let parser = PullParser::new(content, Extensions::all());
    let mut components = Vec::new();

    for event in parser {
        let component = match event {
            Event::Ingredient(located) => {
                let span = located.span();
                Component {
                    kind: ComponentKind::Ingredient,
                    span,
                    name: located.into_inner().name.text_trimmed().into_owned(),
                }
            }
            Event::Cookware(located) => {
                let span = located.span();
                Component {
                    kind: ComponentKind::Cookware,
                    span,
                    name: located.into_inner().name.text_trimmed().into_owned(),
                }
            }
            Event::Timer(located) => {
                let span = located.span();
                let name = located
                    .into_inner()
                    .name
                    .map(|t| t.text_trimmed().into_owned())
                    .unwrap_or_default();
                Component {
                    kind: ComponentKind::Timer,
                    span,
                    name,
                }
            }
            _ => continue,
        };
        components.push(component);
    }

    components
}

/// Returns the component whose span contains the byte `offset`, if any.
pub fn component_at(components: &[Component], offset: usize) -> Option<&Component> {
    components
        .iter()
        .find(|c| c.span.start() <= offset && offset < c.span.end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(content: &str) -> Vec<&str> {
        scan_components(content)
            .into_iter()
            .map(|c| &content[c.span.start()..c.span.end()])
            .collect()
    }

    #[test]
    fn multi_word_with_quantity() {
        let s = "Chill @heavy whipping cream{1%cup}.";
        let comps = scan_components(s);
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].kind, ComponentKind::Ingredient);
        assert_eq!(comps[0].name, "heavy whipping cream");
        assert_eq!(&s[comps[0].span.start()..comps[0].span.end()], "@heavy whipping cream{1%cup}");
    }

    #[test]
    fn names_with_punctuation() {
        assert_eq!(texts("Add @lady's fingers{20}."), vec!["@lady's fingers{20}"]);
        assert_eq!(
            texts("Use @sun-dried tomatoes{1/2%cup}."),
            vec!["@sun-dried tomatoes{1/2%cup}"]
        );
    }

    #[test]
    fn single_word_without_braces() {
        let comps = scan_components("Add @heavy whipping cream then bake.");
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].name, "heavy");
    }

    #[test]
    fn next_marker_breaks_multiword() {
        // The `{}` belongs to `#tool`, so `@multi` is single-word.
        let comps = scan_components("@multi word #tool{} end.");
        assert_eq!(comps.len(), 2);
        assert_eq!(comps[0].kind, ComponentKind::Ingredient);
        assert_eq!(comps[0].name, "multi");
        assert_eq!(comps[1].kind, ComponentKind::Cookware);
        assert_eq!(comps[1].name, "tool");
    }

    #[test]
    fn cookware_and_timer() {
        let comps = scan_components("Bake in #oven{} for ~{10%min}.");
        assert_eq!(comps[0].kind, ComponentKind::Cookware);
        assert_eq!(comps[0].name, "oven");
        assert_eq!(comps[1].kind, ComponentKind::Timer);
        assert_eq!(comps[1].name, ""); // unnamed timer
    }

    #[test]
    fn components_inside_comments_are_ignored() {
        // `@x` is inside a line comment, so the parser does not emit it.
        let comps = scan_components("Mix well -- add @secret later\n");
        assert!(comps.is_empty(), "got: {comps:?}");
    }

    #[test]
    fn component_at_offset() {
        let s = "Chill @heavy whipping cream{1%cup}.";
        let comps = scan_components(s);
        let cursor = s.find("whipping").unwrap();
        let c = component_at(&comps, cursor).unwrap();
        assert_eq!(c.name, "heavy whipping cream");
        // Outside any component.
        assert!(component_at(&comps, 0).is_none());
    }
}
