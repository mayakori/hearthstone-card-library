use html_escape::decode_html_entities;
use regex::Regex;

pub fn plain_text(markup: &str) -> String {
    let normalized = markup.replace("\r\n", "\n").replace('\r', "\n");
    let break_tags = Regex::new(r"(?i)<br\s*/?>").expect("valid static regex");
    let tags = Regex::new(r"<[^>]*>").expect("valid static regex");
    let with_breaks = break_tags.replace_all(&normalized, "\n");
    decode_html_entities(&tags.replace_all(&with_breaks, ""))
        .trim()
        .to_owned()
}
