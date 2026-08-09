//! X-style Post model + display helpers.

use serde::Deserialize;

/// Embedded fixture payload (matches grokathon-shared/fixtures/feed_items.json).
const MOCK_FEED_JSON: &str = include_str!("fixtures/feed_items.json");

/// Only these kinds appear in the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    XPost,
    #[serde(alias = "synthetic_post")]
    SyntheticPost,
    UserPost,
    // Legacy kinds — filtered out of the default feed but deserializable
    #[serde(other)]
    Other,
}

impl SourceType {
    pub fn is_timeline_kind(self) -> bool {
        matches!(self, Self::XPost | Self::SyntheticPost | Self::UserPost)
    }

    pub fn badge(self) -> &'static str {
        match self {
            Self::XPost => "X",
            Self::SyntheticPost => "AI",
            Self::UserPost => "YOU",
            Self::Other => "·",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FeedAuthor {
    #[serde(default)]
    pub handle: String,
    #[serde(default)]
    pub display_name: String,
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub verified: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FeedMedia {
    pub kind: Option<String>,
    pub url: Option<String>,
    pub poster_url: Option<String>,
    pub duration_s: Option<f64>,
    pub mime: Option<String>,
}

impl FeedMedia {
    pub fn hint(&self) -> Option<String> {
        let kind = self.kind.as_deref().unwrap_or("none");
        if kind == "none" || kind.is_empty() {
            return None;
        }
        let glyph = match kind {
            "video" => "▶",
            "audio" => "♪",
            "image" => "▣",
            _ => "·",
        };
        match self.duration_s {
            Some(s) if s > 0.0 => Some(format!("{glyph} {s:.0}s")),
            _ => Some(glyph.to_string()),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FeedMetrics {
    #[serde(default)]
    pub replies: i64,
    #[serde(default)]
    pub reposts: i64,
    #[serde(default)]
    pub likes: i64,
    pub views: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FeedXMeta {
    pub post_id: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FeedAiMeta {
    #[serde(default)]
    pub generated: bool,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FeedMeta {
    pub topic_key: Option<String>,
    #[serde(default)]
    pub search_terms: Vec<String>,
    #[serde(default)]
    pub reason_chips: Vec<String>,
    pub score: Option<f64>,
    pub workspace_key: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FeedProvenance {
    #[serde(default)]
    pub ai_generated: bool,
    #[serde(default)]
    pub urls: Vec<String>,
    pub model: Option<String>,
    pub x_post_id: Option<String>,
    pub session_origin: Option<String>,
    pub author_handle: Option<String>,
}

/// One X-style post in the Feeder dock.
#[derive(Debug, Clone, Deserialize)]
pub struct FeedItem {
    pub id: String,
    /// Preferred kind field; falls back to source_type.
    #[serde(default)]
    pub kind: Option<SourceType>,
    #[serde(default)]
    pub source_type: Option<SourceType>,
    #[serde(default)]
    pub author: FeedAuthor,
    /// Primary body.
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body_md: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    /// New shape: array of media; legacy: single object.
    #[serde(default)]
    pub media: FeedMediaField,
    #[serde(default)]
    pub metrics: FeedMetrics,
    #[serde(default)]
    pub x: FeedXMeta,
    #[serde(default)]
    pub ai: FeedAiMeta,
    #[serde(default)]
    pub feed: FeedMeta,
    #[serde(default)]
    pub ai_generated: bool,
    pub topic_key: Option<String>,
    pub workspace_key: Option<String>,
    pub user_id: Option<String>,
    pub author_id: Option<String>,
    pub quality_score: Option<f64>,
    pub score: Option<f64>,
    #[serde(default)]
    pub reason_chips: Vec<String>,
    #[serde(default)]
    pub provenance: FeedProvenance,
}

/// Accept media as array (new) or single object (legacy).
#[derive(Debug, Clone, Default)]
pub struct FeedMediaField(pub Vec<FeedMedia>);

impl<'de> Deserialize<'de> for FeedMediaField {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{self, Visitor};
        use std::fmt;
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = FeedMediaField;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("media array or object")
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut v = Vec::new();
                while let Some(m) = seq.next_element::<FeedMedia>()? {
                    v.push(m);
                }
                Ok(FeedMediaField(v))
            }
            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let m = FeedMedia::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(FeedMediaField(vec![m]))
            }
            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(FeedMediaField(vec![]))
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(FeedMediaField(vec![]))
            }
        }
        deserializer.deserialize_any(V)
    }
}

impl FeedItem {
    pub fn kind(&self) -> SourceType {
        self.kind
            .or(self.source_type)
            .unwrap_or(SourceType::Other)
    }

    pub fn post_text(&self) -> &str {
        if let Some(t) = self.text.as_deref().filter(|s| !s.is_empty()) {
            return t;
        }
        if let Some(t) = self.body_md.as_deref().filter(|s| !s.is_empty()) {
            return t;
        }
        self.title.as_deref().unwrap_or("")
    }

    pub fn is_ai_generated(&self) -> bool {
        self.ai.generated || self.ai_generated || self.provenance.ai_generated
    }

    pub fn handle(&self) -> String {
        let h = self.author.handle.trim().trim_start_matches('@');
        if !h.is_empty() {
            return h.to_string();
        }
        if let Some(p) = self.provenance.author_handle.as_deref() {
            return p.trim().trim_start_matches('@').to_string();
        }
        self.author_id
            .as_deref()
            .unwrap_or("user")
            .trim_start_matches("x:")
            .to_string()
    }

    pub fn display_name(&self) -> String {
        let n = self.author.display_name.trim();
        if !n.is_empty() {
            return n.to_string();
        }
        self.handle()
    }

    pub fn monogram(&self) -> char {
        self.display_name()
            .chars()
            .next()
            .unwrap_or('·')
            .to_ascii_uppercase()
    }

    /// All reason chips from top-level or nested `feed` (API may send either).
    pub fn all_reason_chips(&self) -> Vec<String> {
        if !self.reason_chips.is_empty() {
            return self.reason_chips.clone();
        }
        if !self.feed.reason_chips.is_empty() {
            return self.feed.reason_chips.clone();
        }
        Vec::new()
    }

    pub fn reason_line(&self) -> String {
        let chips = self.all_reason_chips();
        if chips.is_empty() {
            String::new()
        } else {
            chips.join(" · ")
        }
    }

    pub fn open_url(&self) -> Option<String> {
        self.x
            .url
            .clone()
            .filter(|u| !u.is_empty())
            .or_else(|| self.provenance.urls.first().cloned())
            .or_else(|| {
                self.x
                    .post_id
                    .as_ref()
                    .or(self.provenance.x_post_id.as_ref())
                    .map(|id| format!("https://x.com/i/status/{id}"))
            })
            .or_else(|| {
                self.media
                    .0
                    .first()
                    .and_then(|m| m.url.clone())
                    .filter(|u| !u.is_empty())
            })
    }

    pub fn relative_time(&self) -> String {
        // Keep simple — show clock fragment from ISO if present
        let Some(raw) = self.created_at.as_deref() else {
            return String::new();
        };
        // "2026-08-08T10:00:00Z" → "10:00"
        if let Some(t) = raw.split('T').nth(1) {
            return t.trim_end_matches('Z').chars().take(5).collect();
        }
        raw.chars().take(10).collect()
    }

    /// How many terminal rows this post needs at `width`.
    pub fn height_rows(&self, width: u16, selected: bool) -> u16 {
        let w = width.saturating_sub(2).max(8) as usize;
        let body_lines = wrap_text(self.post_text(), w);
        // Keep cards shorter so "From X" + metrics stay on-screen.
        let max_body = if selected { 5 } else { 3 };
        let body_h = body_lines.len().clamp(1, max_body) as u16;
        let media_h = u16::from(self.media.0.iter().any(|m| m.hint().is_some()));
        // author + source badge ("From X") + body + media? + metrics
        1 + 1 + body_h + media_h + 1
    }
}

/// Word-wrap helper for dock width.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![];
    }
    let mut lines = Vec::new();
    for para in text.split('\n') {
        if para.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut cur = String::new();
        for word in para.split_whitespace() {
            if cur.is_empty() {
                cur = word.to_string();
            } else if cur.len() + 1 + word.len() <= width {
                cur.push(' ');
                cur.push_str(word);
            } else {
                lines.push(std::mem::take(&mut cur));
                cur = word.to_string();
            }
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Load mock cards; drop legacy kinds.
pub fn load_mock_items() -> Vec<FeedItem> {
    match serde_json::from_str::<Vec<FeedItem>>(MOCK_FEED_JSON) {
        Ok(items) => items
            .into_iter()
            .filter(|i| i.kind().is_timeline_kind())
            .collect(),
        Err(err) => {
            tracing::error!(%err, "feeder: failed to parse embedded mock fixtures");
            Vec::new()
        }
    }
}

/// Filter API items to timeline kinds only.
pub fn filter_timeline(items: Vec<FeedItem>) -> Vec<FeedItem> {
    items
        .into_iter()
        .filter(|i| i.kind().is_timeline_kind())
        .collect()
}
