use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct OgpData {
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
}

#[async_trait]
pub trait ArticleVerifier: Send + Sync {
    async fn verify_backlink(&self, article_url: &str, tag_url: &str) -> bool;
    async fn fetch_ogp(&self, article_url: &str) -> OgpData;
}

#[derive(Clone)]
pub struct HttpArticleVerifier {
    client: reqwest::Client,
}

impl HttpArticleVerifier {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("WebHashtag-Bot/0.1")
                .build()
                .expect("reqwest client"),
        }
    }
}

impl Default for HttpArticleVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ArticleVerifier for HttpArticleVerifier {
    async fn verify_backlink(&self, article_url: &str, tag_url: &str) -> bool {
        let Ok(response) = self.client.get(article_url).send().await else {
            return false;
        };
        if !response.status().is_success() {
            return false;
        }
        response
            .text()
            .await
            .is_ok_and(|html| html.contains(tag_url))
    }

    async fn fetch_ogp(&self, article_url: &str) -> OgpData {
        let Ok(response) = self.client.get(article_url).send().await else {
            return OgpData::default();
        };
        let Ok(html) = response.text().await else {
            return OgpData::default();
        };

        OgpData {
            title: meta_content(&html, "og:title").or_else(|| title_content(&html)),
            description: meta_content(&html, "og:description"),
            image: meta_content(&html, "og:image"),
        }
    }
}

fn meta_content(html: &str, property: &str) -> Option<String> {
    for part in html.split("<meta").skip(1) {
        let tag = part.split('>').next().unwrap_or_default();
        let lower = tag.to_ascii_lowercase();
        let property_match = format!(r#"property="{property}""#);
        let name_match = format!(r#"name="{property}""#);
        if lower.contains(&property_match) || lower.contains(&name_match) {
            return attr_value(tag, "content");
        }
    }
    None
}

fn title_content(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    Some(html[start..end].trim().to_string()).filter(|title| !title.is_empty())
}

fn attr_value(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!(r#"{attr}=""#);
    let value_start = lower.find(&needle)? + needle.len();
    let value_end = tag[value_start..].find('"')? + value_start;
    Some(tag[value_start..value_end].to_string()).filter(|value| !value.is_empty())
}
