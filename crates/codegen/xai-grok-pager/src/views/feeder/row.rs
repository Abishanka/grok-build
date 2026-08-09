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
    pub fn is_visual(&self) -> bool {
        matches!(
            self.kind.as_deref().unwrap_or(""),
            "image" | "photo" | "video" | "animated_gif" | "gif"
        ) && self
            .image_url()
            .map(|u| !u.is_empty() && !u.contains("example.com"))
            .unwrap_or(false)
    }

    /// Best URL to show / open for this media object.
    pub fn image_url(&self) -> Option<&str> {
        self.poster_url
            .as_deref()
            .filter(|u| !u.is_empty())
            .or_else(|| self.url.as_deref().filter(|u| !u.is_empty()))
    }

    pub fn hint(&self) -> Option<String> {
        let kind = self.kind.as_deref().unwrap_or("none");
        if kind == "none" || kind.is_empty() {
            return None;
        }
        let glyph = match kind {
            "video" => "▶ video",
            "audio" => "♪ audio",
            "image" | "photo" | "animated_gif" | "gif" => "▣ image",
            _ => "· media",
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

    /// Prefer a real X status URL. Rejects truncated fixture-style IDs.
    pub fn open_url(&self) -> Option<String> {
        let candidates = [
            self.x.url.clone(),
            self.provenance.urls.first().cloned(),
            self.x
                .post_id
                .as_ref()
                .or(self.provenance.x_post_id.as_ref())
                .filter(|id| is_plausible_x_status_id(id))
                .map(|id| {
                    let h = self.handle();
                    format!("https://x.com/{h}/status/{id}")
                }),
            self.media
                .0
                .iter()
                .find_map(|m| m.image_url().map(|u| u.to_string())),
        ];
        candidates
            .into_iter()
            .flatten()
            .find(|u| is_plausible_open_url(u))
    }

    /// First image/video URL suitable for inline preview.
    pub fn preview_image_url(&self) -> Option<String> {
        self.media
            .0
            .iter()
            .find(|m| m.is_visual())
            .and_then(|m| m.image_url().map(|u| u.to_string()))
    }

    pub fn has_visual_media(&self) -> bool {
        self.media.0.iter().any(|m| m.is_visual())
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

    /// How many terminal rows this post needs at `width` (includes trailing gap).
    pub fn height_rows(&self, width: u16, selected: bool) -> u16 {
        // content width: leave 1 col for left accent bar + 1 pad
        let w = width.saturating_sub(3).max(8) as usize;
        let body_lines = wrap_text(self.post_text(), w);
        let has_media = self.has_visual_media();
        // Shorter body when media so image + metrics always fit.
        let max_body = if has_media {
            if selected { 4 } else { 3 }
        } else if selected {
            6
        } else {
            4
        };
        let body_h = body_lines.len().clamp(1, max_body) as u16;
        let media_h = if has_media {
            if selected {
                MEDIA_PREVIEW_ROWS_SELECTED
            } else {
                MEDIA_PREVIEW_ROWS
            }
        } else {
            0
        };
        // author + badge + body + media? + metrics + blank gap
        1 + 1 + body_h + media_h + 1 + POST_GAP
    }
}

/// Blank rows between cards (air between posts).
pub const POST_GAP: u16 = 3;
/// Media slot height (terminal rows) when not selected — compact card or Kitty.
pub const MEDIA_PREVIEW_ROWS: u16 = 4;
/// Media slot height when selected.
pub const MEDIA_PREVIEW_ROWS_SELECTED: u16 = 6;
/// Max posts held in the dock (ring buffer).
pub const DOCK_CAP: usize = 20;
/// How many new ranked posts each refresh pulls (then merge + evict tail).
pub const FETCH_BATCH: usize = 5;
/// First open / cold start may request a larger batch to fill the dock.
pub const FETCH_INITIAL: usize = 15;

/// Backward-compat alias — prefer [`DOCK_CAP`].
pub const SLATE_LIMIT: usize = DOCK_CAP;

fn is_plausible_x_status_id(id: &str) -> bool {
    let id = id.trim();
    // Real snowflake IDs are long numeric strings; reject "1", "2", "fixture_x_1".
    id.len() >= 10 && id.chars().all(|c| c.is_ascii_digit())
}

fn is_plausible_open_url(url: &str) -> bool {
    let u = url.trim();
    if u.is_empty() || u.contains("example.com") {
        return false;
    }
    if let Some(rest) = u.strip_prefix("https://x.com/") {
        if let Some(id) = rest.split("/status/").nth(1) {
            let id = id.split(['?', '#']).next().unwrap_or(id);
            return is_plausible_x_status_id(id);
        }
        // bare profile etc. ok
        return !rest.is_empty();
    }
    if let Some(rest) = u.strip_prefix("https://twitter.com/") {
        if let Some(id) = rest.split("/status/").nth(1) {
            let id = id.split(['?', '#']).next().unwrap_or(id);
            return is_plausible_x_status_id(id);
        }
        return !rest.is_empty();
    }
    u.starts_with("https://") || u.starts_with("http://")
}

/// Word-wrap by **display width** (not bytes) so CJK / bullets wrap correctly.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
            let word_w = UnicodeWidthStr::width(word);
            // Hard-break overlong tokens
            if word_w > width {
                if !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                }
                let mut chunk = String::new();
                let mut cw = 0usize;
                for ch in word.chars() {
                    let ch_w = UnicodeWidthChar::width(ch).unwrap_or(0);
                    if cw + ch_w > width && !chunk.is_empty() {
                        lines.push(std::mem::take(&mut chunk));
                        cw = 0;
                    }
                    chunk.push(ch);
                    cw += ch_w;
                }
                if !chunk.is_empty() {
                    cur = chunk;
                }
                continue;
            }
            let cur_w = UnicodeWidthStr::width(cur.as_str());
            if cur.is_empty() {
                cur = word.to_string();
            } else if cur_w + 1 + word_w <= width {
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
