//! Feeder — ranked multimodal feed dock inside Grok Build.
//!
//! Phone-width **right dock** beside the live agent (not a full-screen mode).
//! State lives on `AppView::feeder`; visibility is `AppView::feeder_dock_open`.
//!
//! ## Module layout
//!
//! - [`state`] — `FeederState` (selection, scroll, toast, items)
//! - [`row`] — `FeedItem` model + display helpers
//! - [`layout`] — pure rect computation (list + peek + footer)
//! - [`render`] — paint routine
//! - [`peek`] — peek panel body
//! - [`feed_client`] — HTTP client to feeder-service (live + fixture fallback)
//!
//! Toggle with `/feeder`. Tab focuses agent ↔ dock. Prefer live API
//! (`FEEDER_BASE_URL`); falls back to embedded fixtures when offline.

pub mod feed_client;
pub mod layout;
pub mod peek;
pub mod render;
pub mod row;
pub mod state;

pub use render::render_feeder;
pub use row::{
    filter_timeline, load_mock_items, FeedAuthor, FeedItem, FeedMedia, FeedProvenance, SourceType,
};
pub use state::FeederState;

/// Whether the feeder surface is available.
///
/// Order: env override (`GROK_FEEDER=0` → off) wins; default on.
pub fn feeder_enabled() -> bool {
    if std::env::var_os("GROK_FEEDER")
        .as_deref()
        .is_some_and(|v| v == std::ffi::OsStr::new("0"))
    {
        return false;
    }
    true
}

/// Preferred dock width (phone column). Returns 0 if the terminal is too narrow.
pub fn dock_width(total_width: u16) -> u16 {
    // Need room for a usable agent (~56) + dock + gap.
    const MIN_TOTAL: u16 = 96;
    const DOCK: u16 = 40;
    const MIN_AGENT: u16 = 52;
    if total_width < MIN_TOTAL {
        return 0;
    }
    let max_dock = total_width.saturating_sub(MIN_AGENT + 1);
    DOCK.min(max_dock).max(32).min(max_dock)
}

/// Split `area` into `(agent_area, feeder_dock)` when dock is open.
/// Returns `None` for feeder when dock closed or width insufficient.
pub fn split_agent_feeder(
    area: ratatui::layout::Rect,
    dock_open: bool,
) -> (ratatui::layout::Rect, Option<ratatui::layout::Rect>) {
    if !dock_open || area.width == 0 {
        return (area, None);
    }
    let w = dock_width(area.width);
    if w == 0 {
        return (area, None);
    }
    let gap: u16 = 1;
    let agent_w = area.width.saturating_sub(w + gap);
    if agent_w < 40 {
        return (area, None);
    }
    let agent = ratatui::layout::Rect {
        x: area.x,
        y: area.y,
        width: agent_w,
        height: area.height,
    };
    let feeder = ratatui::layout::Rect {
        x: area.x.saturating_add(agent_w + gap),
        y: area.y,
        width: w,
        height: area.height,
    };
    (agent, Some(feeder))
}
