pub fn escape_text(value: &str) -> String {
    escape_xml(value, false)
}

pub fn escape_attribute(value: &str) -> String {
    escape_xml(value, true)
}

fn escape_xml(value: &str, attribute: bool) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' if attribute => escaped.push_str("&quot;"),
            '\'' if attribute => escaped.push_str("&apos;"),
            character if is_xml_10_character(character) => escaped.push(character),
            _ => escaped.push('\u{FFFD}'),
        }
    }
    escaped
}

fn is_xml_10_character(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{A}' | '\u{D}')
        || ('\u{20}'..='\u{D7FF}').contains(&character)
        || ('\u{E000}'..='\u{FFFD}').contains(&character)
        || ('\u{10000}'..='\u{10FFFF}').contains(&character)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_text_and_attributes() {
        assert_eq!(escape_text("<&>"), "&lt;&amp;&gt;");
        assert_eq!(escape_attribute("'\"<&"), "&apos;&quot;&lt;&amp;");
        assert_eq!(escape_text("ok\u{000B}bad\u{FFFF}"), "ok�bad�");
    }
}
