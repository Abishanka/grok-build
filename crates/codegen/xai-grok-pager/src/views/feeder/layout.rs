//! Pure rect layout for the Feeder dock (posts carry their own body).

use ratatui::layout::Rect;

/// Computed regions for one Feeder frame.
#[derive(Debug, Clone, Copy)]
pub struct FeederLayout {
    pub header: Rect,
    pub list: Rect,
    pub peek: Rect,
    pub footer: Rect,
    pub visible_rows: usize,
}

/// Header + footer only; list is the post stream (peek collapsed — body is in-card).
pub fn compute_layout(area: Rect) -> FeederLayout {
    if area.height == 0 || area.width == 0 {
        let empty = Rect::new(area.x, area.y, 0, 0);
        return FeederLayout {
            header: empty,
            list: empty,
            peek: empty,
            footer: empty,
            visible_rows: 0,
        };
    }

    let header_h: u16 = 1.min(area.height);
    let footer_h: u16 = if area.height > 2 { 1 } else { 0 };
    let list_h = area.height.saturating_sub(header_h + footer_h);

    let header = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: header_h,
    };
    let list = Rect {
        x: area.x,
        y: area.y.saturating_add(header_h),
        width: area.width,
        height: list_h,
    };
    let peek = Rect {
        x: area.x,
        y: list.y.saturating_add(list_h),
        width: area.width,
        height: 0,
    };
    let footer = Rect {
        x: area.x,
        y: area.y.saturating_add(area.height.saturating_sub(footer_h)),
        width: area.width,
        height: footer_h,
    };

    FeederLayout {
        header,
        list,
        peek,
        footer,
        visible_rows: list_h as usize,
    }
}
