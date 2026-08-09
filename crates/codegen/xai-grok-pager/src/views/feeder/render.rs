//! Feeder dock paint — spaced cards, color accents, inline media.
//!
//! Kitty placements are absolute and survive cell redraws. Each frame we
//! place only visible media, then clear stale feeder ids (full wipe on scroll)
//! so nothing floats over the agent pane.

use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::theme::Theme;
use crate::views::goal_detail::truncate_to_width;

use super::layout::compute_layout;
use super::media_preview::{global_cache, halfblock_enabled, MediaCache};
use super::row::{wrap_text, FeedItem, SourceType, MEDIA_PREVIEW_ROWS, MEDIA_PREVIEW_ROWS_SELECTED};
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
    // One-post carousel: selection index is the only page.
    if !state.items.is_empty() {
        state.selected = state.selected.min(state.items.len() - 1);
        state.scroll = state.selected;
    }
    prefetch_media(state);

    // Order matters for Kitty:
    //   1) begin_frame — full clear on scroll/selection (reset transmitted)
    //   2) place visible media (retransmit after full clear)
    //   3) end_frame — clear ids that left the viewport
    let mut escapes = String::new();
    {
        let cache_arc = global_cache();
        if let Ok(mut cache) = cache_arc.lock() {
            escapes.push_str(&cache.begin_frame(state.scroll, state.selected));
        }
    }

    let (place_esc, this_frame_ids) = render_one_post(buf, list, state, &theme);
    escapes.push_str(&place_esc);

    {
        let cache_arc = global_cache();
        if let Ok(mut cache) = cache_arc.lock() {
            escapes.push_str(&cache.end_frame(this_frame_ids, state.scroll, state.selected));
        }
    }

    render_footer(buf, layout.footer, state, &theme);
    if escapes.is_empty() {
        (None, None)
    } else {
        (None, Some(escapes))
    }
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
    // Only current ±1 for carousel.
    let n = state.items.len();
    if n == 0 {
        return;
    }
    let i = state.selected.min(n - 1);
    for idx in [i.saturating_sub(1), i, (i + 1).min(n - 1)] {
        if let Some(url) = state.items[idx].preview_image_url() {
            cache.ensure(&url);
        }
    }
}

/// Paint exactly one post (carousel page) filling the list rect.
fn render_one_post(
    buf: &mut Buffer,
    area: Rect,
    state: &FeederState,
    theme: &Theme,
) -> (String, HashSet<u32>) {
    let mut escapes = String::new();
    let mut this_frame = HashSet::new();
    if area.height == 0 || area.width == 0 {
        return (escapes, this_frame);
    }
    if state.items.is_empty() {
        let msg = if state.loading {
            "Loading feed…"
        } else {
            "No posts — send a prompt or press r"
        };
        buf.set_string(
            area.x.saturating_add(1),
            area.y,
            msg,
            Style::default().fg(theme.text_secondary).bg(theme.bg_base),
        );
        return (escapes, this_frame);
    }

    let idx = state.selected.min(state.items.len() - 1);
    let item = &state.items[idx];
    let block = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height,
    };
    // Reuse paint_post path — single card, always "selected" chrome when dock focused.
    let (esc, ids) = paint_post_full(
        buf,
        block,
        item,
        state.dock_focused,
        theme,
        idx,
        state.items.len(),
    );
    escapes.push_str(&esc);
    this_frame.extend(ids);
    (escapes, this_frame)
}

fn paint_post_full(
    buf: &mut Buffer,
    area: Rect,
    item: &FeedItem,
    dock_focused: bool,
    theme: &Theme,
    index: usize,
    total: usize,
) -> (String, HashSet<u32>) {
    let _ = (index, total);
    // Full dock height, always highlighted when dock focused.
    paint_post(buf, area, item, true, dock_focused, theme, area)
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
    list_clip: Rect,
) -> (String, HashSet<u32>) {
    let mut esc = String::new();
    let mut ids = HashSet::new();
    if area.height == 0 || area.width == 0 {
        return (esc, ids);
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
        return (esc, ids);
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
        return (esc, ids);
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
            return (esc, ids);
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
    // One-post carousel: use the full dock height for body text.
    let max_body = (body_budget as usize).max(1);

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
            // Clip to list viewport so placements never sit outside the dock list.
            if let Some(clipped) = intersect_rect(media_area, list_clip) {
                if clipped.height >= 2 && clipped.width >= 4 {
                    let (m_esc, m_ids) = paint_media(buf, clipped, item, &c);
                    esc.push_str(&m_esc);
                    ids.extend(m_ids);
                }
            }
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

    (esc, ids)
}

/// Intersection of two rects; `None` if empty.
fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = a.x.saturating_add(a.width).min(b.x.saturating_add(b.width));
    let y1 = a.y.saturating_add(a.height).min(b.y.saturating_add(b.height));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(Rect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    })
}

fn paint_media(
    buf: &mut Buffer,
    area: Rect,
    item: &FeedItem,
    c: &CardColors,
) -> (String, HashSet<u32>) {
    let mut esc = String::new();
    let mut ids = HashSet::new();
    if area.width == 0 || area.height == 0 {
        return (esc, ids);
    }
    let url = item.preview_image_url();

    // 1) Kitty inline (safe terminals only) — absolute place + tracked ids.
    if MediaCache::kitty_inline_ok() {
        if let Some(ref u) = url {
            let cache_arc = global_cache();
            if let Ok(mut cache) = cache_arc.lock() {
                if cache.get(u).is_some() {
                    // Blank underlay so cell text does not show through the image.
                    let blank = " ".repeat(area.width as usize);
                    for r in 0..area.height {
                        buf.set_string(
                            area.x,
                            area.y + r,
                            &blank,
                            Style::default().bg(c.bg),
                        );
                    }
                    if let Some((place, id)) = cache.placement_escapes(u, area) {
                        esc.push_str(&place);
                        ids.insert(id);
                        return (esc, ids);
                    }
                }
            }
        }
    }

    // 2) Half-block buffer-local preview (scrolls with dock cells).
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
                    return (esc, ids);
                }
            }
        }
    }

    // 3) Loading / failed: framed text card in the dock buffer.
    paint_media_card(buf, area, item, c, url.as_deref());
    (esc, ids)
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
