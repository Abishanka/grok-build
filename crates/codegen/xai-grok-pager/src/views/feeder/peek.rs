//! Peek panel — shows the selected card's `body_md`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::theme::Theme;
use crate::views::goal_detail::truncate_to_width;

use super::row::FeedItem;
use super::state::FeederState;

/// Paint the peek panel (title strip + body_md lines).
pub fn render_peek(buf: &mut Buffer, area: Rect, state: &FeederState, theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // Clear
    let blank = " ".repeat(area.width as usize);
    let bg = Style::default().bg(theme.bg_base);
    for row in 0..area.height {
        buf.set_string(area.x, area.y + row, &blank, bg);
    }

    let Some(item) = state.selected_item() else {
        let msg = "No feed item selected";
        buf.set_string(
            area.x.saturating_add(1),
            area.y,
            truncate_to_width(msg, area.width.saturating_sub(2) as usize),
            Style::default().fg(theme.text_secondary).bg(theme.bg_base),
        );
        return;
    };

    // Top rule / title
    let title = format!(" Peek · @{}", item.handle());
    buf.set_string(
        area.x,
        area.y,
        truncate_to_width(&title, area.width as usize),
        Style::default()
            .fg(theme.text_secondary)
            .bg(theme.bg_base)
            .add_modifier(Modifier::BOLD),
    );

    if area.height < 2 {
        return;
    }

    let body_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height - 1,
    };
    render_body_md(buf, body_area, item, state.peek_scroll, theme);
}

fn render_body_md(
    buf: &mut Buffer,
    area: Rect,
    item: &FeedItem,
    scroll: usize,
    theme: &Theme,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let width = area.width as usize;
    let lines = wrap_md_lines(item.post_text(), width.saturating_sub(2).max(1));
    let style = Style::default().fg(theme.text_primary).bg(theme.bg_base);
    let pad_x = area.x.saturating_add(1);

    for (i, row) in (0..area.height).enumerate() {
        let idx = scroll + i;
        if let Some(line) = lines.get(idx) {
            buf.set_string(
                pad_x,
                area.y + row,
                truncate_to_width(line, width.saturating_sub(2)),
                style,
            );
        }
    }
}

/// Naive wrap on whitespace for markdown body preview (no full md render).
fn wrap_md_lines(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    if width == 0 {
        return out;
    }
    for raw in text.lines() {
        if raw.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut rest = raw;
        while !rest.is_empty() {
            if rest.chars().count() <= width {
                out.push(rest.to_string());
                break;
            }
            // Prefer break at last space within width
            let mut end = width;
            let prefix: String = rest.chars().take(width).collect();
            if let Some(sp) = prefix.rfind(' ') {
                if sp > 0 {
                    end = sp;
                }
            }
            let (chunk, next) = split_at_chars(rest, end);
            out.push(chunk.trim_end().to_string());
            rest = next.trim_start();
        }
    }
    out
}

fn split_at_chars(s: &str, n: usize) -> (String, &str) {
    let mut idx = 0;
    for (i, _) in s.char_indices().take(n) {
        idx = i;
    }
    // advance past the nth char
    let mut chars = s.char_indices().skip(n);
    let split = chars.next().map(|(i, _)| i).unwrap_or(s.len());
    let _ = idx;
    (s[..split].to_string(), &s[split..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_preserves_blank_lines() {
        let lines = wrap_md_lines("a\n\nb", 40);
        assert_eq!(lines, vec!["a", "", "b"]);
    }
}
