//! The Markdown templates, embedded in the binary at compile time.

/// The skeleton of a new dossier.
pub const DOSSIER: &str = include_str!("../templates/dossier.md");

/// The skeleton of a new daily entry.
pub const DAILY_ENTRY: &str = include_str!("../templates/daily-entry.md");

/// Fill in a template's `$NAME` placeholders.
///
/// Substitution is a single pass, so a value that itself looks like a
/// placeholder is written out as the user typed it. A placeholder with no
/// matching value is left alone, which is why the tests below check that the
/// embedded templates only use placeholders we know about.
#[must_use]
pub fn render(template: &str, values: &[(&str, &str)]) -> String {
    let mut rendered = String::with_capacity(template.len());
    let mut characters = template.chars().peekable();

    while let Some(character) = characters.next() {
        if character != '$' {
            rendered.push(character);
            continue;
        }

        let mut name = String::new();
        while let Some(&next) = characters.peek() {
            if next.is_ascii_alphanumeric() || next == '_' {
                name.push(next);
                characters.next();
            } else {
                break;
            }
        }

        if let Some((_, value)) = values.iter().find(|(key, _)| *key == name.as_str()) {
            rendered.push_str(value);
        } else {
            rendered.push('$');
            rendered.push_str(&name);
        }
    }

    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_in_every_placeholder() {
        let rendered = render(
            "$A and $B, but $C",
            &[("A", "one"), ("B", "two"), ("C", "three")],
        );
        assert_eq!(rendered, "one and two, but three");
    }

    #[test]
    fn does_not_expand_placeholders_inside_values() {
        let rendered = render("$A", &[("A", "$B"), ("B", "expanded")]);
        assert_eq!(rendered, "$B");
    }

    #[test]
    fn keeps_non_recognized_placeholders() {
        let rendered = render("$A $B", &[("A", "aaa")]);
        assert_eq!(rendered, "aaa $B");
    }

    #[test]
    fn dossier_template_only_uses_known_placeholders() {
        let rendered = render(
            DOSSIER,
            &[
                ("DOSSIER_NAME", "name"),
                ("CREATION_DATE", "date"),
                ("DESCRIPTION", "description"),
                ("DOSSIER_LINK", "link\n"),
            ],
        );
        assert!(!rendered.contains('$'), "left a placeholder in: {rendered}");
    }

    #[test]
    fn daily_entry_template_only_uses_known_placeholders() {
        let rendered = render(DAILY_ENTRY, &[("DATE", "2026-03-23")]);
        assert!(!rendered.contains('$'), "left a placeholder in: {rendered}");
    }
}
