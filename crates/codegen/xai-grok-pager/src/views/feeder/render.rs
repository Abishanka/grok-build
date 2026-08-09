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
    let layout = compute_layout(area);
    fill(buf, area, &theme);
    render_header(buf, layout.header, state, &theme);

    // Layout posts by variable height within list rect
    let list = layout.list;
    let (first, _used) = layout_visible_posts(state, list.height, list.width);
    state.scroll = first;
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
    let src = if state.live { "live" } else { "offline" };
    let focus = if state.dock_focused { "●" } else { "○" };
    let title = format!(" {focus} Feeder · {sel}/{n} · {src} ");
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

/// Returns (scroll_first_index, total_height_used_hint).
fn layout_visible_posts(state: &FeederState, list_h: u16, width: u16) -> (usize, u16) {
    if state.items.is_empty() || list_h == 0 {
        return (0, 0);
    }
    // Ensure selected is reachable: walk from selected backward to fill height
    let sel = state.selected.min(state.items.len() - 1);
    let mut start = sel;
    let mut h = state.items[sel].height_rows(width, true);
    while start > 0 {
        let prev_h = state.items[start - 1].height_rows(width, false);
        if h + prev_h > list_h {
            break;
        }
        start -= 1;
        h += prev_h;
    }
    // Prefer keeping state.scroll if it still shows selection
    let scroll = state.scroll.min(sel);
    let mut acc = 0u16;
    let mut ok = true;
    for i in scroll..=sel {
        let ih = state.items[i].height_rows(width, i == sel);
        if acc + ih > list_h {
            ok = false;
            break;
        }
        acc += ih;
    }
    if ok {
        (scroll, acc)
    } else {
        (start, h)
    }
}

fn render_posts(buf: &mut Buffer, area: Rect, state: &FeederState, theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    if state.items.is_empty() {
        buf.set_string(
            area.x.saturating_add(1),
            area.y,
            "No posts yet — worker warming…",
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
        paint_post(buf, block, item, selected, theme);
        y = y.saturating_add(h);
        idx += 1;
    }
}

fn paint_post(buf: &mut Buffer, area: Rect, item: &FeedItem, selected: bool, theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let bg = if selected {
        theme.bg_highlight
    } else {
        theme.bg_base
    };
    let fg = theme.text_primary;
    let dim = theme.text_secondary;
    let blank = " ".repeat(area.width as usize);
    for row in 0..area.height {
        buf.set_string(area.x, area.y + row, &blank, Style::default().bg(bg));
    }

    let w = area.width as usize;
    let mut row = 0u16;

    // Author line: ◉ Name  @handle · time  [AI]
    let mono = item.monogram();
    let name = item.display_name();
    let handle = item.handle();
    let time = item.relative_time();
    let ai = if item.is_ai_generated() { " AI" } else { "" };
    let author = format!(" {mono} {name}  @{handle} · {time}{ai}");
    buf.set_string(
        area.x,
        area.y + row,
        truncate_to_width(&author, w),
        Style::default()
            .fg(fg)
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

    // Body lines
    let body_w = w.saturating_sub(2).max(6);
    let wrapped = wrap_text(item.post_text(), body_w);
    let max_body = if selected { 8 } else { 5 };
    for line in wrapped.iter().take(max_body) {
        if row >= area.height {
            break;
        }
        let content = format!("  {line}");
        buf.set_string(
            area.x,
            area.y + row,
            truncate_to_width(&content, w),
            Style::default().fg(fg).bg(bg),
        );
        row += 1;
    }

    // Media hint
    if let Some(hint) = item.media.0.iter().find_map(|m| m.hint()) {
        if row < area.height {
            buf.set_string(
                area.x,
                area.y + row,
                truncate_to_width(&format!("  {hint}"), w),
                Style::default().fg(dim).bg(bg),
            );
            row += 1;
        }
    }

    // Metrics
    if row < area.height {
        let m = &item.metrics;
        let metrics = format!(
            "  💬 {}   ↻ {}   ♥ {}",
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
        row += 1;
    }

    // Reason
    let reason = item.reason_line();
    if !reason.is_empty() && row < area.height {
        buf.set_string(
            area.x,
            area.y + row,
            truncate_to_width(&format!("  {reason}"), w),
            Style::default().fg(dim).bg(bg),
        );
    }
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
