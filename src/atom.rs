use html_escape::{encode_double_quoted_attribute, encode_text};

use crate::store::Entry;

pub fn build_atom_feed(tag: &str, server_host: &str, entries: &[Entry]) -> String {
    let feed_url = if server_host.is_empty() {
        format!("/feed/{tag}.atom")
    } else {
        format!("https://{server_host}/feed/{tag}.atom")
    };
    let updated = entries
        .iter()
        .map(|entry| entry.registered_at.as_str())
        .max()
        .unwrap_or("1970-01-01T00:00:00Z");

    let mut xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?><feed xmlns="http://www.w3.org/2005/Atom"><title>#{}</title><id>{}</id><link href="{}"/><updated>{}</updated>"#,
        encode_text(tag),
        encode_text(&feed_url),
        encode_double_quoted_attribute(&feed_url),
        encode_text(updated),
    );

    for entry in entries {
        let title = entry.ogp.title.as_deref().unwrap_or(&entry.url);
        xml.push_str(&format!(
            r#"<entry><title>{}</title><id>{}</id><link href="{}"/><updated>{}</updated>"#,
            encode_text(title),
            encode_text(&entry.url),
            encode_double_quoted_attribute(&entry.url),
            encode_text(&entry.registered_at),
        ));
        if let Some(description) = entry.ogp.description.as_deref() {
            xml.push_str(&format!("<summary>{}</summary>", encode_text(description)));
        }
        xml.push_str("</entry>");
    }

    xml.push_str("</feed>");
    xml
}
