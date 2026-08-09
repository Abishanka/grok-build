//! Feeder dock paint — spaced cards, color accents, clean media.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::theme::Theme;
use crate::views::goal_detail::truncate_to_width;

use super::layout::compute_layout;
use super::media_preview::{global_cache, halfblock_enabled};
use super::row::{
    wrap_text, FeedItem, SourceType, MEDIA_PREVIEW_ROWS, MEDIA_PREVIEW_ROWS_SELECTED, POST_GAP,
};
use super::state::FeederState;

/// Render the Feeder dock. Returns optional post-flush Kitty/iTerm escapes.
pub fn render_feeder(
    buf: &mut Buffer,
    area: Rect,
    state: &mut FeederState,
) -> (Option<(u16, u16)>, Option<String>) {
    let theme = Theme::current();
    fill(buf, area, &theme);

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
    prefetch_media(state);
    ensure_selection_visible(state, list.height, list.width);
    let mut escapes = String::new();
    // Always clear absolute Kitty graphics first — prior feeder frames left
    // images floating outside the dock over the agent pane.
    escapes.push_str(&crate::terminal::overlay::clear_kitty().into_string());
    if let Some(e) = render_posts(buf, list, state, &theme) {
        escapes.push_str(&e);
    }
    render_footer(buf, layout.footer, state, &theme);
    (None, Some(escapes))
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

fn prefetch_media(state: &FeederState) {
    let cache_arc = global_cache();
    let Ok(mut cache) = cache_arc.lock() else {
        return;
    };
    for item in &state.items {
        if let Some(url) = item.preview_image_url() {
            cache.ensure(&url);
        }
    }
}

fn ensure_selection_visible(state: &mut FeederState, list_h: u16, width: u16) {
    if state.items.is_empty() || list_h == 0 {
        return;
    }
    let sel = state.selected.min(state.items.len() - 1);
    state.selected = sel;

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

fn render_posts(
    buf: &mut Buffer,
    area: Rect,
    state: &FeederState,
    theme: &Theme,
) -> Option<String> {
    if area.height == 0 || area.width == 0 {
        return None;
    }
    if state.items.is_empty() {
        buf.set_string(
            area.x.saturating_add(1),
            area.y,
            "No posts — refreshing…",
            Style::default().fg(theme.text_secondary).bg(theme.bg_base),
        );
        return None;
    }

    let mut escapes = String::new();
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
        let content_h = h.saturating_sub(POST_GAP).min(end_y.saturating_sub(y));
        let block = Rect {
            x: area.x,
            y,
            width: area.width,
            height: content_h,
        };
        if let Some(esc) = paint_post(buf, block, item, selected, state.dock_focused, theme) {
            escapes.push_str(&esc);
        }
        // Dim separator in the gap
        if POST_GAP >= 2 && y + content_h + 1 < end_y {
            let sep_y = y + content_h + 1;
            let rule = "·".repeat(area.width.saturating_sub(2).max(1) as usize);
            buf.set_string(
                area.x.saturating_add(1),
                sep_y,
                truncate_to_width(&rule, area.width.saturating_sub(1) as usize),
                Style::default().fg(theme.gray_dim).bg(theme.bg_base),
            );
        }
        y = y.saturating_add(h);
        idx += 1;
    }
    if escapes.is_empty() {
        None
    } else {
        Some(escapes)
    }
}

struct CardColors {
    accent: Color,
    badge: Color,
    author: Color,
    body: Color,
    dim: Color,
    bg: Color,
}

fn card_colors(item: &FeedItem, selected: bool, dock_focused: bool, theme: &Theme) -> CardColors {
    let (accent, badge) = match item.kind() {
        SourceType::XPost => (Color::Rgb(29, 155, 240), Color::Rgb(29, 155, 240)),
        SourceType::SyntheticPost => (Color::Rgb(168, 85, 247), Color::Rgb(192, 132, 252)),
        SourceType::UserPost => (theme.accent_success, theme.accent_success),
        SourceType::Other => (theme.text_secondary, theme.text_secondary),
    };
    let bg = if selected && dock_focused {
        theme.bg_highlight
    } else if selected {
        theme.bg_light
    } else {
        theme.bg_base
    };
    CardColors {
        accent,
        badge,
        author: if selected {
            theme.text_primary
        } else {
            theme.gray_bright
        },
        body: theme.text_primary,
        dim: theme.text_secondary,
        bg,
    }
}

fn paint_post(
    buf: &mut Buffer,
    area: Rect,
    item: &FeedItem,
    selected: bool,
    dock_focused: bool,
    theme: &Theme,
) -> Option<String> {
    if area.height == 0 || area.width == 0 {
        return None;
    }
    let c = card_colors(item, selected, dock_focused, theme);
    let blank = " ".repeat(area.width as usize);
    let base = Style::default().bg(c.bg);
    for row in 0..area.height {
        buf.set_string(area.x, area.y + row, &blank, base);
    }

    for row in 0..area.height {
        buf.set_string(
            area.x,
            area.y + row,
            "▌",
            Style::default().fg(c.accent).bg(c.bg),
        );
    }

    let text_x = area.x.saturating_add(2);
    let w = area.width.saturating_sub(2) as usize;
    if w == 0 {
        return None;
    }
    let mut row = 0u16;
    let sel_mark = if selected { "›" } else { " " };

    // Author
    let handle = item.handle();
    let time = item.relative_time();
    let handle_budget = w.saturating_sub(10).max(4);
    let handle_disp = truncate_to_width(&handle, handle_budget);
    let author = if time.is_empty() {
        format!("{sel_mark}@{handle_disp}")
    } else {
        format!("{sel_mark}@{handle_disp} · {time}")
    };
    buf.set_string(
        text_x,
        area.y + row,
        truncate_to_width(&author, w),
        Style::default()
            .fg(c.author)
            .bg(c.bg)
            .add_modifier(if selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
    );
    row += 1;
    if row >= area.height {
        return None;
    }

    // Badge
    {
        let badge = source_badge(item);
        let matched = match_fragment(item);
        let why = if matched.is_empty() {
            badge
        } else {
            format!("{badge} · {matched}")
        };
        buf.set_string(
            text_x,
            area.y + row,
            truncate_to_width(&why, w),
            Style::default()
                .fg(c.badge)
                .bg(c.bg)
                .add_modifier(Modifier::BOLD),
        );
        row += 1;
        if row >= area.height {
            return None;
        }
    }

    // Hard reserve: metrics (1) + media (optional)
    let has_media = item.has_visual_media();
    let media_rows = if has_media {
        if selected {
            MEDIA_PREVIEW_ROWS_SELECTED
        } else {
            MEDIA_PREVIEW_ROWS
        }
    } else {
        0
    };
    let reserve = 1u16 + media_rows;
    let body_budget = area
        .height
        .saturating_sub(row)
        .saturating_sub(reserve)
        .max(1);
    let max_body = if has_media {
        if selected { 4 } else { 3 }
    } else if selected {
        6
    } else {
        4
    }
    .min(body_budget as usize);

    let body_w = w.saturating_sub(1).max(6);
    let wrapped = wrap_text(item.post_text(), body_w);
    for line in wrapped.iter().take(max_body) {
        if row >= area.height.saturating_sub(reserve) {
            break;
        }
        buf.set_string(
            text_x,
            area.y + row,
            truncate_to_width(line, w),
            Style::default().fg(c.body).bg(c.bg),
        );
        row += 1;
    }

    // Media slot (before metrics) — never steals metrics row
    let mut media_esc = None;
    if media_rows > 0 {
        let room_after_metrics = area.height.saturating_sub(row).saturating_sub(1);
        let avail = room_after_metrics.min(media_rows);
        if avail >= 2 {
            let media_area = Rect {
                x: text_x,
                y: area.y + row,
                width: area.width.saturating_sub(2),
                height: avail,
            };
            media_esc = paint_media(buf, media_area, item, &c);
            row = row.saturating_add(avail);
        }
    }

    // Metrics — always last content row inside the card
    if row < area.height {
        let m = &item.metrics;
        let metrics = format!(
            "r{}  rt{}  ♥{}",
            fmt_count(m.replies),
            fmt_count(m.reposts),
            fmt_count(m.likes)
        );
        buf.set_string(
            text_x,
            area.y + row,
            truncate_to_width(&metrics, w),
            Style::default().fg(c.dim).bg(c.bg),
        );
    }

    media_esc
}

fn paint_media(
    buf: &mut Buffer,
    area: Rect,
    item: &FeedItem,
    c: &CardColors,
) -> Option<String> {
    if area.width == 0 || area.height == 0 {
        return None;
    }
    let url = item.preview_image_url();

    // IMPORTANT: do NOT use Kitty/iTerm graphics here.
    // Those are absolute screen placements and float outside the scrollable
    // Feeder dock (appear "in the background"). Media must live in the
    // ratatui cell buffer so it scrolls with the panel.

    // Optional half-block only if explicitly enabled (still buffer-local).
    if halfblock_enabled() {
        if let Some(ref u) = url {
            let cache_arc = global_cache();
            if let Ok(cache) = cache_arc.lock() {
                if let Some(hb) = cache.halfblock(u) {
                    let max_rows = area.height as usize;
                    let max_cols = area.width as usize;
                    for (ri, prow) in hb.rows.iter().take(max_rows).enumerate() {
                        for (ci, (fg, bg)) in prow.cells.iter().take(max_cols).enumerate() {
                            if let Some(cell) =
                                buf.cell_mut((area.x + ci as u16, area.y + ri as u16))
                            {
                                cell.set_symbol("▀");
                                cell.set_style(Style::default().fg(*fg).bg(*bg));
                            }
                        }
                    }
                    return None;
                }
            }
        }
    }

    // Default: clean media card painted into the dock buffer (scrolls with dock).
    paint_media_card(buf, area, item, c, url.as_deref());
    None
}

fn paint_media_card(
    buf: &mut Buffer,
    area: Rect,
    item: &FeedItem,
    c: &CardColors,
    url: Option<&str>,
) {
    let label = item
        .media
        .0
        .iter()
        .find_map(|m| m.hint())
        .unwrap_or_else(|| "▣ media".into());
    let host = url
        .unwrap_or("")
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("media");
    let w = area.width as usize;
    let inner = w.saturating_sub(2);
    for r in 0..area.height {
        let line = if r == 0 {
            format!("┌{}┐", "─".repeat(inner))
        } else if r + 1 == area.height {
            format!("└{}┘", "─".repeat(inner))
        } else if r == 1 {
            let mid = format!(" {label} ");
            pad_center(&mid, inner)
        } else if r == 2 && area.height > 3 {
            let mid = format!(" {host} ");
            pad_center(&truncate_to_width(&mid, inner), inner)
        } else if r + 1 == area.height.saturating_sub(1) && area.height > 4 {
            pad_center(" o open · v media ", inner)
        } else {
            format!("│{}│", " ".repeat(inner))
        };
        buf.set_string(
            area.x,
            area.y + r,
            truncate_to_width(&line, w),
            Style::default().fg(c.dim).bg(c.bg),
        );
    }
}

fn pad_center(mid: &str, inner: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    let mw = UnicodeWidthStr::width(mid);
    if mw >= inner {
        return format!("│{}│", truncate_to_width(mid, inner));
    }
    let pad = inner - mw;
    let left = pad / 2;
    let right = pad - left;
    format!("│{}{}{}│", " ".repeat(left), mid, " ".repeat(right))
}

fn source_badge(item: &FeedItem) -> String {
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
        SourceType::XPost => "From X".into(),
        SourceType::SyntheticPost => "AI post".into(),
        SourceType::UserPost => "You".into(),
        SourceType::Other => "Feed".into(),
    }
}

fn match_fragment(item: &FeedItem) -> String {
    for chip in item.all_reason_chips() {
        let c = chip.trim();
        if let Some(rest) = c.strip_prefix("Matched:") {
            let t = rest.trim();
            if !t.is_empty() {
                return format!("Matched: {}", truncate_plain(t, 22));
            }
        }
    }
    if let Some(t) = item.feed.search_terms.first() {
        let t = t.trim();
        if !t.is_empty() {
            return format!("Matched: {}", truncate_plain(t, 22));
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
