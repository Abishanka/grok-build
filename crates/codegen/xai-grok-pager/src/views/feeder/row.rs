//! X-style Post model + display helpers.

use serde::Deserialize;
use unicode_width::UnicodeWidthStr;

use crate::views::goal_detail::truncate_to_width;

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

/// Social provenance for one feed item — peer endorsement + synthetic origin.
///
/// The feeder service attaches this **only** to items carrying a social signal;
/// most items have no `social` key at all. Every field is optional (and may
/// arrive as `null`), so this must never make an item fail to deserialise.
///
/// ```json
/// "social": {
///   "source": "cohort",
///   "peer_count": 4,
///   "endorse_weight": 2.5,
///   "synth_origin": "cohort",
///   "origin_handle": "alice"
/// }
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FeedSocial {
    /// How the item entered the feed. `"cohort"` = surfaced by nearby peers.
    #[serde(default)]
    pub source: Option<String>,
    /// How many nearby peers explicitly saved this item.
    #[serde(default)]
    pub peer_count: Option<i64>,
    /// Ranking weight the endorsement contributed (not rendered).
    #[serde(default)]
    pub endorse_weight: Option<f64>,
    /// For synthetic posts: `"cohort"` (peer's work) or `"self"` (your work).
    #[serde(default)]
    pub synth_origin: Option<String>,
    /// Handle of the peer whose session produced a `"cohort"` synthetic post.
    #[serde(default)]
    pub origin_handle: Option<String>,
}

/// Rendered social chips for one card row, already fitted to the card width.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SocialChips {
    /// "4 people near you saved this" (widest form that fits).
    pub peer: Option<String>,
    /// "from @alice's session" / "from your session".
    pub origin: Option<String>,
    /// Origin is the viewer's own session — render de-emphasised.
    pub origin_is_self: bool,
}

impl SocialChips {
    pub fn is_empty(&self) -> bool {
        self.peer.is_none() && self.origin.is_none()
    }
}

/// Separator between the two chips (matches the `·` used elsewhere in cards).
const CHIP_SEP: &str = " · ";

/// Widest form from `forms` that fits `width` columns; `None` when even the
/// shortest form cannot fit (so we paint nothing rather than a stub).
fn pick_fitting(forms: &[String], width: usize) -> Option<String> {
    forms
        .iter()
        .find(|f| UnicodeWidthStr::width(f.as_str()) <= width)
        .cloned()
}

impl FeedSocial {
    fn tag_is(field: &Option<String>, tag: &str) -> bool {
        field
            .as_deref()
            .map(|s| s.trim().eq_ignore_ascii_case(tag))
            .unwrap_or(false)
    }

    /// Item was surfaced because peers in your cohort engaged with it.
    pub fn is_cohort(&self) -> bool {
        Self::tag_is(&self.source, "cohort")
    }

    /// Peers who saved this (clamped at 0; missing counts as none).
    pub fn peers(&self) -> i64 {
        self.peer_count.unwrap_or(0).max(0)
    }

    /// Synthetic post generated from the viewer's *own* work.
    pub fn is_self_synth(&self) -> bool {
        Self::tag_is(&self.synth_origin, "self")
    }

    /// Handle of the peer this synthetic post came from, if any.
    pub fn peer_origin_handle(&self) -> Option<&str> {
        if !Self::tag_is(&self.synth_origin, "cohort") {
            return None;
        }
        self.origin_handle
            .as_deref()
            .map(|h| h.trim().trim_start_matches('@'))
            .filter(|h| !h.is_empty())
    }

    /// Peer-endorsement chip, degrading gracefully as `width` shrinks.
    ///
    /// `"4 people near you saved this"` → `"★ 4 nearby saved this"` → `"★ 4 saved"` → `"★4"`.
    pub fn peer_chip(&self, width: usize) -> Option<String> {
        if !self.is_cohort() {
            return None;
        }
        let n = self.peers();
        if n < 1 {
            return None;
        }
        let noun = if n == 1 { "person" } else { "people" };
        let forms = [
            format!("★ {n} {noun} near you saved this"),
            format!("★ {n} nearby saved this"),
            format!("★ {n} saved"),
            format!("★{n}"),
        ];
        pick_fitting(&forms, width)
    }

    /// Synthetic-origin chip: `"from @alice's session"`, or the de-emphasised
    /// `"from your session"` when the post came from your own work.
    pub fn origin_chip(&self, width: usize) -> Option<String> {
        if let Some(h) = self.peer_origin_handle() {
            // Handle itself can be long — clamp it before building the forms.
            let h = truncate_to_width(h, width.saturating_sub(4).clamp(6, 24));
            let forms = [
                format!("from @{h}'s session"),
                format!("from @{h}"),
                format!("@{h}"),
            ];
            return pick_fitting(&forms, width);
        }
        if self.is_self_synth() {
            let forms = [
                "from your session".to_string(),
                "your session".to_string(),
                "yours".to_string(),
            ];
            return pick_fitting(&forms, width);
        }
        None
    }
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
    /// Social provenance (peer endorsement / synth origin). Absent on most
    /// items — `Option` so an explicit `"social": null` also deserialises.
    #[serde(default)]
    pub social: Option<FeedSocial>,
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

    /// Social chips fitted to a card of `text_width` columns (the width the
    /// card body is painted into). Empty when the item has no social signal.
    ///
    /// The peer chip has priority: it takes as much of the row as it needs and
    /// the origin chip gets whatever is left, shrinking or dropping entirely
    /// rather than spilling past the dock edge.
    pub fn social_chips(&self, text_width: usize) -> SocialChips {
        let Some(social) = self.social.as_ref() else {
            return SocialChips::default();
        };
        let peer = social.peer_chip(text_width);
        let used = peer
            .as_deref()
            .map(|p| UnicodeWidthStr::width(p) + UnicodeWidthStr::width(CHIP_SEP))
            .unwrap_or(0);
        let origin = social.origin_chip(text_width.saturating_sub(used));
        SocialChips {
            peer,
            origin,
            origin_is_self: social.is_self_synth(),
        }
    }

    /// Text columns available to card content at a given card width — must
    /// match `paint_post`'s `w` so height and paint agree.
    pub(super) fn card_text_width(width: u16) -> usize {
        width.saturating_sub(2) as usize
    }

    /// Extra rows (0 or 1) the social chip line needs at `width`.
    pub fn social_rows(&self, width: u16) -> u16 {
        if self.social_chips(Self::card_text_width(width)).is_empty() {
            0
        } else {
            1
        }
    }

    /// Synthetic post generated from the viewer's own work — shown, but
    /// visually de-emphasised.
    pub fn is_self_synth(&self) -> bool {
        self.social.as_ref().is_some_and(|s| s.is_self_synth())
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
        // author + badge + social? + body + media? + metrics + blank gap
        1 + 1 + self.social_rows(width) + body_h + media_h + 1 + POST_GAP
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
/// First open / cold start batch (must stay ≤ API max limit, currently 25).
pub const FETCH_INITIAL: usize = 10;

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

#[cfg(test)]
mod social_tests {
    use super::*;

    /// Today's wire shape — no `social` key at all.
    const NO_SOCIAL: &str = r#"{
        "id": "a1",
        "kind": "x_post",
        "author": { "handle": "authdev", "display_name": "Auth Dev" },
        "text": "refresh tokens expiring early on mobile",
        "reason_chips": ["From X"]
    }"#;

    /// Narrowest dock the TUI ever renders (see `mod::dock_width` MIN_DOCK).
    const NARROW_CARD_W: u16 = 39; // 40-col dock minus the border column

    fn item(json: &str) -> FeedItem {
        serde_json::from_str(json).expect("feed item should deserialise")
    }

    fn with_social(social: &str) -> FeedItem {
        item(&format!(
            r#"{{ "id": "s1", "kind": "synthetic_post",
                  "author": {{ "handle": "grok" }},
                  "text": "body", "social": {social} }}"#
        ))
    }

    #[test]
    fn item_without_social_is_unchanged() {
        let it = item(NO_SOCIAL);
        assert!(it.social.is_none());
        assert!(!it.is_self_synth());
        assert!(it.social_chips(37).is_empty());
        // No extra row: author + badge + body(1) + metrics + gap.
        assert_eq!(it.social_rows(48), 0);
        assert_eq!(it.height_rows(48, false), 1 + 1 + 1 + 1 + POST_GAP);
        // Everything else still resolves the way it did before.
        assert_eq!(it.handle(), "authdev");
        assert_eq!(it.reason_line(), "From X");
    }

    #[test]
    fn embedded_fixtures_still_parse() {
        assert!(!load_mock_items().is_empty());
    }

    #[test]
    fn social_null_or_partial_never_fails() {
        assert!(with_social("null").social.is_none());
        let partial = with_social(r#"{ "peer_count": 3 }"#);
        let s = partial.social.as_ref().unwrap();
        assert_eq!(s.peers(), 3);
        assert!(!s.is_cohort()); // no source → no chip
        assert!(partial.social_chips(37).is_empty());
        // Unknown / future fields are ignored, not fatal.
        let future = with_social(r#"{ "source": "cohort", "future_field": [1,2] }"#);
        assert!(future.social.is_some());
    }

    #[test]
    fn full_contract_deserialises() {
        let it = with_social(
            r#"{ "source": "cohort", "peer_count": 4, "endorse_weight": 2.5,
                 "synth_origin": "cohort", "origin_handle": "alice" }"#,
        );
        let s = it.social.as_ref().unwrap();
        assert!(s.is_cohort());
        assert_eq!(s.peers(), 4);
        assert_eq!(s.endorse_weight, Some(2.5));
        assert_eq!(s.peer_origin_handle(), Some("alice"));
        assert!(!s.is_self_synth());
    }

    #[test]
    fn peer_chip_gets_the_plural_right() {
        let one = with_social(r#"{ "source": "cohort", "peer_count": 1 }"#);
        let many = with_social(r#"{ "source": "cohort", "peer_count": 4 }"#);
        assert_eq!(
            one.social_chips(60).peer.as_deref(),
            Some("★ 1 person near you saved this")
        );
        assert_eq!(
            many.social_chips(60).peer.as_deref(),
            Some("★ 4 people near you saved this")
        );
    }

    #[test]
    fn peer_chip_needs_cohort_source_and_a_peer() {
        let no_peers = with_social(r#"{ "source": "cohort", "peer_count": 0 }"#);
        let not_cohort = with_social(r#"{ "source": "rank", "peer_count": 9 }"#);
        assert!(no_peers.social_chips(60).is_empty());
        assert!(not_cohort.social_chips(60).is_empty());
    }

    #[test]
    fn origin_chip_names_the_peer_session() {
        let it = with_social(r#"{ "synth_origin": "cohort", "origin_handle": "@alice" }"#);
        let chips = it.social_chips(60);
        assert_eq!(chips.origin.as_deref(), Some("from @alice's session"));
        assert!(chips.peer.is_none());
        assert!(!chips.origin_is_self);
        assert_eq!(it.social_rows(48), 1);
    }

    #[test]
    fn self_origin_is_shown_but_marked_for_de_emphasis() {
        let it = with_social(r#"{ "synth_origin": "self" }"#);
        let chips = it.social_chips(60);
        assert_eq!(chips.origin.as_deref(), Some("from your session"));
        assert!(chips.origin_is_self);
        assert!(it.is_self_synth());
    }

    #[test]
    fn chips_fit_the_narrowest_dock() {
        let it = with_social(
            r#"{ "source": "cohort", "peer_count": 12,
                 "synth_origin": "cohort", "origin_handle": "alexandra_the_verbose" }"#,
        );
        for card_w in [NARROW_CARD_W, 47, 63] {
            let w = FeedItem::card_text_width(card_w);
            let chips = it.social_chips(w);
            let mut used = chips
                .peer
                .as_deref()
                .map(UnicodeWidthStr::width)
                .unwrap_or(0);
            if let Some(o) = chips.origin.as_deref() {
                if used > 0 {
                    used += UnicodeWidthStr::width(CHIP_SEP);
                }
                used += UnicodeWidthStr::width(o);
            }
            assert!(used <= w, "row {used} cols overflows {w} at card {card_w}");
            assert!(chips.peer.is_some(), "peer chip must survive at {card_w}");
        }
    }

    #[test]
    fn peer_chip_degrades_then_disappears() {
        let it = with_social(r#"{ "source": "cohort", "peer_count": 4 }"#);
        assert_eq!(
            it.social_chips(30).peer.as_deref(),
            Some("★ 4 people near you saved this")
        );
        assert_eq!(
            it.social_chips(29).peer.as_deref(),
            Some("★ 4 nearby saved this")
        );
        assert_eq!(it.social_chips(20).peer.as_deref(), Some("★ 4 saved"));
        assert_eq!(it.social_chips(5).peer.as_deref(), Some("★4"));
        // Below the shortest form we render nothing rather than a stub.
        assert!(it.social_chips(1).is_empty());
        assert_eq!(it.social_rows(2), 0);
    }

    #[test]
    fn social_row_costs_exactly_one_row_and_keeps_media_reserved() {
        let base = r#"{ "id": "m1", "kind": "x_post", "text": "one line",
            "media": [{ "kind": "image", "url": "https://pics.test/a.png" }] "#;
        let plain: FeedItem = item(&format!("{base}}}"));
        let social: FeedItem = item(&format!(
            r#"{base}, "social": {{ "source": "cohort", "peer_count": 2 }} }}"#
        ));
        assert!(plain.has_visual_media() && social.has_visual_media());
        for sel in [false, true] {
            assert_eq!(
                social.height_rows(48, sel),
                plain.height_rows(48, sel) + 1,
                "social card must grow by exactly one row (selected={sel})"
            );
        }
    }
}
