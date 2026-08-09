//! Feeder dock paint — X-style multi-line posts.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::theme::Theme;
use crate::views::goal_detail::truncate_to_width;

use super::layout::compute_layout;
use super::row::{wrap_text, FeedItem};
use super::state::FeederState;

/// Render the Feeder dock into `buf`.
pub fn render_feeder(buf: &mut Buffer, area: Rect, state: &mut FeederState) -> Option<(u16, u16)> {
    let theme = Theme::current();
    fill(buf, area, &theme);

    // Vertical border on the left edge of the dock
    let border = Style::default().fg(theme.text_secondary).bg(theme.bg_base);
    for row in 0..area.height {
        buf.set_string(area.x, area.y + row, "│", border);
    }

    let inner = Rect {
        x: area.x.saturating_add(1),
        y: area.y,
        width: area.width.saturating_sub(1),
        height: area.height,
    };
    let layout = compute_layout(inner);
    render_header(buf, layout.header, state, &theme);

    let list = layout.list;
    // Only nudge scroll if selection is off-screen — don't clobber every frame
    ensure_selection_visible(state, list.height, list.width);
    render_posts(buf, list, state, &theme);
    render_footer(buf, layout.footer, state, &theme);
    None
}

fn fill(buf: &mut Buffer, area: Rect, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let blank = " ".repeat(area.width as usize);
    let style = Style::default().bg(theme.bg_base);
    for row in 0..area.height {
        buf.set_string(area.x, area.y + row, &blank, style);
    }
}

fn render_header(buf: &mut Buffer, area: Rect, state: &FeederState, theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let n = state.items.len();
    let sel = if n == 0 { 0 } else { state.selected + 1 };
    let src = if state.loading {
        "…"
    } else if state.live {
        "live"
    } else {
        "off"
    };
    let focus = if state.dock_focused { "●" } else { "○" };
    let title = format!(" {focus} Feeder {sel}/{n} {src}");
    let style = if state.dock_focused {
        Style::default()
            .fg(theme.text_primary)
            .bg(theme.bg_base)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.text_secondary)
            .bg(theme.bg_base)
    };
    buf.set_string(
        area.x,
        area.y,
        truncate_to_width(&title, area.width as usize),
        style,
    );
}

fn ensure_selection_visible(state: &mut FeederState, list_h: u16, width: u16) {
    if state.items.is_empty() || list_h == 0 {
        return;
    }
    let sel = state.selected.min(state.items.len() - 1);
    state.selected = sel;

    // If current scroll window already contains selection, keep it.
    let mut y = 0u16;
    let mut end = state.scroll;
    while end < state.items.len() {
        let h = state.items[end].height_rows(width, end == sel);
        if y + h > list_h {
            break;
        }
        y += h;
        end += 1;
    }
    if sel >= state.scroll && sel < end {
        return;
    }

    // Rebuild window ending at selection (or starting at selection).
    let mut start = sel;
    let mut h = state.items[sel].height_rows(width, true);
    while start > 0 {
        let prev = state.items[start - 1].height_rows(width, false);
        if h + prev > list_h {
            break;
        }
        start -= 1;
        h += prev;
    }
    state.scroll = start;
}

fn render_posts(buf: &mut Buffer, area: Rect, state: &FeederState, theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    if state.items.is_empty() {
        buf.set_string(
            area.x.saturating_add(1),
            area.y,
            "No posts — refreshing…",
            Style::default().fg(theme.text_secondary).bg(theme.bg_base),
        );
        return;
    }

    let mut y = area.y;
    let end_y = area.y.saturating_add(area.height);
    let mut idx = state.scroll;
    while idx < state.items.len() && y < end_y {
        let item = &state.items[idx];
        let selected = idx == state.selected;
        let h = item.height_rows(area.width, selected);
        if y + h > end_y && idx != state.scroll {
            break;
        }
        let block = Rect {
            x: area.x,
            y,
            width: area.width,
            height: h.min(end_y.saturating_sub(y)),
        };
        paint_post(buf, block, item, selected, state.dock_focused, theme);
        y = y.saturating_add(h);
        idx += 1;
    }
}

fn paint_post(
    buf: &mut Buffer,
    area: Rect,
    item: &FeedItem,
    selected: bool,
    dock_focused: bool,
    theme: &Theme,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let bg = if selected && dock_focused {
        theme.bg_highlight
    } else if selected {
        theme.bg_base
    } else {
        theme.bg_base
    };
    let fg = theme.text_primary;
    let dim = theme.text_secondary;
    let blank = " ".repeat(area.width as usize);
    let base = Style::default().bg(bg);
    for row in 0..area.height {
        buf.set_string(area.x, area.y + row, &blank, base);
    }

    let w = area.width as usize;
    let mut row = 0u16;
    let sel_mark = if selected { "›" } else { " " };

    // Author line — keep short so source badge always fits
    let handle = item.handle();
    let time = item.relative_time();
    let handle_budget = w.saturating_sub(14).max(4); // room for " · From X" / time
    let handle_disp = truncate_to_width(&handle, handle_budget);
    let author = if time.is_empty() {
        format!("{sel_mark}@{handle_disp}")
    } else {
        format!("{sel_mark}@{handle_disp} · {time}")
    };
    buf.set_string(
        area.x,
        area.y + row,
        truncate_to_width(&author, w),
        Style::default()
            .fg(if selected { fg } else { dim })
            .bg(bg)
            .add_modifier(if selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
    );
    row += 1;
    if row >= area.height {
        return;
    }

    // FROM X / AI badge — painted SECOND so it is never clipped by tall bodies
    {
        let badge = source_badge(item);
        let matched = match_fragment(item);
        let why = if matched.is_empty() {
            badge
        } else {
            format!("{badge} · {matched}")
        };
        buf.set_string(
            area.x,
            area.y + row,
            truncate_to_width(&format!(" {why}"), w),
            Style::default()
                .fg(if selected { fg } else { dim })
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        );
        row += 1;
        if row >= area.height {
            return;
        }
    }

    // Body — leave room for metrics (1 row) at the bottom when possible
    let reserve_metrics: u16 = 1;
    let body_budget = area
        .height
        .saturating_sub(row)
        .saturating_sub(reserve_metrics)
        .max(1);
    let body_w = w.saturating_sub(2).max(6);
    let wrapped = wrap_text(item.post_text(), body_w);
    let max_body = (if selected { 5 } else { 3 }).min(body_budget as usize);
    for line in wrapped.iter().take(max_body) {
        if row >= area.height {
            break;
        }
        buf.set_string(
            area.x,
            area.y + row,
            truncate_to_width(&format!(" {line}"), w),
            Style::default().fg(fg).bg(bg),
        );
        row += 1;
    }

    // Media hint (optional, only if room before metrics)
    if let Some(hint) = item.media.0.iter().find_map(|m| m.hint()) {
        if row + 1 < area.height {
            buf.set_string(
                area.x,
                area.y + row,
                truncate_to_width(&format!(" {hint}"), w),
                Style::default().fg(dim).bg(bg),
            );
            row += 1;
        }
    }

    // Metrics — ASCII so width is predictable in narrow docks
    if row < area.height {
        let m = &item.metrics;
        let metrics = format!(
            " r{}  rt{}  <3{}",
            fmt_count(m.replies),
            fmt_count(m.reposts),
            fmt_count(m.likes)
        );
        buf.set_string(
            area.x,
            area.y + row,
            truncate_to_width(&metrics, w),
            Style::default().fg(dim).bg(bg),
        );
    }
}

/// Always-visible source label ("From X" / "AI post" / "You").
fn source_badge(item: &FeedItem) -> String {
    // Prefer explicit chips that start with From / AI / You
    for chip in item.all_reason_chips() {
        let c = chip.trim();
        if c.eq_ignore_ascii_case("From X")
            || c.eq_ignore_ascii_case("AI")
            || c.eq_ignore_ascii_case("AI post")
            || c.eq_ignore_ascii_case("You")
            || c.eq_ignore_ascii_case("Shared post")
        {
            return if c.eq_ignore_ascii_case("AI") {
                "AI post".into()
            } else if c.eq_ignore_ascii_case("Shared post") {
                "You".into()
            } else {
                c.to_string()
            };
        }
    }
    match item.kind() {
        super::row::SourceType::XPost => "From X".into(),
        super::row::SourceType::SyntheticPost => "AI post".into(),
        super::row::SourceType::UserPost => "You".into(),
        super::row::SourceType::Other => "Feed".into(),
    }
}

fn match_fragment(item: &FeedItem) -> String {
    for chip in item.all_reason_chips() {
        let c = chip.trim();
        if let Some(rest) = c.strip_prefix("Matched:") {
            let t = rest.trim();
            if !t.is_empty() {
                // Keep short so "From X · Matched: …" fits a ~40-col dock
                return format!("Matched: {}", truncate_plain(t, 18));
            }
        }
    }
    // Fall back to first search term
    if let Some(t) = item.feed.search_terms.first() {
        let t = t.trim();
        if !t.is_empty() {
            return format!("Matched: {}", truncate_plain(t, 18));
        }
    }
    String::new()
}

fn truncate_plain(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
}

fn fmt_count(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn render_footer(buf: &mut Buffer, area: Rect, state: &FeederState, theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let msg = state
        .toast
        .as_deref()
        .unwrap_or_else(|| FeederState::help_line());
    buf.set_string(
        area.x,
        area.y,
        truncate_to_width(msg, area.width as usize),
        Style::default()
            .fg(theme.text_secondary)
            .bg(theme.bg_base),
    );
}
