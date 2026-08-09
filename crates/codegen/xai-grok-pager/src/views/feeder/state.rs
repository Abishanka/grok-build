//! Feeder view state — selection, scroll, toast, live or mock items.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind};

use crate::actions::ActionRegistry;
use crate::app::actions::Action;
use crate::app::app_view::InputOutcome;

use super::feed_client::{
    discuss_prompt, explain_prompt, untrusted_context_block, FeedClient,
};
use super::row::{filter_timeline, load_mock_items, FeedItem};

/// In-memory state for the Feeder full-screen view.
#[derive(Debug)]
pub struct FeederState {
    /// Ranked feed cards.
    pub items: Vec<FeedItem>,
    /// Selected row index into `items` (0 when empty).
    pub selected: usize,
    /// First visible row index (vertical scroll).
    pub scroll: usize,
    /// Scroll offset inside the peek body (lines).
    pub peek_scroll: usize,
    /// Transient status / toast (footer).
    pub toast: Option<String>,
    /// Tick counter for spinner/blink if needed later.
    pub spinner_tick: u64,
    /// Whether the last load came from the live API.
    pub live: bool,
    /// Compose draft (title) when in simple post mode — unused for multi-line; one-shot post uses defaults.
    pub compose_title: Option<String>,
    /// Dock has keyboard focus (set by AppView each frame / on toggle).
    pub dock_focused: bool,
}

impl Default for FeederState {
    fn default() -> Self {
        Self::new()
    }
}

impl FeederState {
    /// Fresh state: try live API, fall back to embedded fixtures.
    pub fn new() -> Self {
        let mut s = Self {
            items: Vec::new(),
            selected: 0,
            scroll: 0,
            peek_scroll: 0,
            toast: None,
            spinner_tick: 0,
            live: false,
            compose_title: None,
            dock_focused: false,
        };
        s.refresh_from_service(None);
        s
    }

    /// Reload from feeder-service. `hint` is optional recent prompt for ranking context.
    pub fn refresh_from_service(&mut self, hint: Option<&str>) {
        let client = FeedClient::from_env();
        let prompts: Vec<String> = hint
            .map(|h| vec![h.to_string()])
            .unwrap_or_else(|| {
                vec![
                    "fix oauth refresh token expiry".into(),
                    "coding agent feed ranking".into(),
                ]
            });
        match client.query_feed(
            &prompts,
            Some("repo:demo"),
            Some("TokenExpired"),
            None,
            std::env::current_dir()
                .ok()
                .map(|p| p.display().to_string())
                .as_deref(),
            15,
        ) {
            Ok(items) if !items.is_empty() => {
                self.items = filter_timeline(items);
                self.live = true;
                self.selected = 0;
                self.scroll = 0;
                self.peek_scroll = 0;
                self.toast = Some(format!(
                    "Feeder · {} cards · {}",
                    self.items.len(),
                    client.base_url
                ));
            }
            Ok(_) => {
                self.items = load_mock_items();
                self.live = false;
                self.toast = Some("Feeder · empty live response — showing fixtures".into());
            }
            Err(err) => {
                self.items = load_mock_items();
                self.live = false;
                self.toast = Some(format!("Feeder · offline ({err}) — fixtures"));
            }
        }
    }

    /// Currently selected item, if any.
    pub fn selected_item(&self) -> Option<&FeedItem> {
        self.items.get(self.selected)
    }

    /// Move selection by `delta` rows, clamping and keeping selection in view.
    pub fn move_selection(&mut self, delta: isize, visible_rows: usize) {
        if self.items.is_empty() {
            return;
        }
        let len = self.items.len() as isize;
        let next = (self.selected as isize + delta).clamp(0, len - 1) as usize;
        if next != self.selected {
            self.selected = next;
            self.peek_scroll = 0;
            self.ensure_visible(visible_rows);
        }
    }

    /// Keep `selected` inside the visible window of `visible_rows` list rows.
    pub fn ensure_visible(&mut self, visible_rows: usize) {
        if visible_rows == 0 || self.items.is_empty() {
            return;
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + visible_rows {
            self.scroll = self.selected + 1 - visible_rows;
        }
        let max_scroll = self.items.len().saturating_sub(visible_rows);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
    }

    /// Route input while the feeder view is active.
    pub fn handle_input(&mut self, ev: &Event, _registry: &ActionRegistry) -> InputOutcome {
        match ev {
            Event::Key(key) if key.kind != KeyEventKind::Release => self.handle_key(key),
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => {
                    self.move_selection(-1, 12);
                    InputOutcome::Changed
                }
                MouseEventKind::ScrollDown => {
                    self.move_selection(1, 12);
                    InputOutcome::Changed
                }
                _ => InputOutcome::Unchanged,
            },
            Event::Resize(_, _) => InputOutcome::Changed,
            _ => InputOutcome::Unchanged,
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> InputOutcome {
        // Esc: unfocus dock first (AppView may close on second Esc); q closes dock
        if matches!(key.code, KeyCode::Esc) {
            return InputOutcome::Action(Action::CloseFeeder);
        }
        if matches!(key.code, KeyCode::Char('q')) && key.modifiers == KeyModifiers::NONE {
            return InputOutcome::Action(Action::CloseFeeder);
        }
        // Tab handled at AppView level when dock is open

        // Navigation
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                self.move_selection(-1, 12);
                return InputOutcome::Changed;
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                self.move_selection(1, 12);
                return InputOutcome::Changed;
            }
            KeyCode::PageUp => {
                self.move_selection(-8, 12);
                return InputOutcome::Changed;
            }
            KeyCode::PageDown => {
                self.move_selection(8, 12);
                return InputOutcome::Changed;
            }
            KeyCode::Home => {
                if !self.items.is_empty() {
                    self.selected = 0;
                    self.scroll = 0;
                    self.peek_scroll = 0;
                }
                return InputOutcome::Changed;
            }
            KeyCode::End => {
                if !self.items.is_empty() {
                    self.selected = self.items.len() - 1;
                    self.peek_scroll = 0;
                    self.ensure_visible(12);
                }
                return InputOutcome::Changed;
            }
            KeyCode::Char('[') if key.modifiers == KeyModifiers::NONE => {
                self.peek_scroll = self.peek_scroll.saturating_sub(1);
                return InputOutcome::Changed;
            }
            KeyCode::Char(']') if key.modifiers == KeyModifiers::NONE => {
                self.peek_scroll = self.peek_scroll.saturating_add(1);
                return InputOutcome::Changed;
            }
            KeyCode::Char('r') if key.modifiers == KeyModifiers::NONE => {
                self.refresh_from_service(None);
                return InputOutcome::Changed;
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                return self.handle_action_key('u');
            }
            _ => {}
        }

        // Card actions
        if key.modifiers == KeyModifiers::NONE
            && let KeyCode::Char(ch) = key.code
        {
            let lower = ch.to_ascii_lowercase();
            return self.handle_action_key(lower);
        }

        InputOutcome::Unchanged
    }

    fn handle_action_key(&mut self, ch: char) -> InputOutcome {
        let client = FeedClient::from_env();
        let Some(item) = self.selected_item().cloned() else {
            self.toast = Some("No card selected".into());
            return InputOutcome::Changed;
        };

        match ch {
            // Use as context — dock stays open; focus returns to agent via AppView
            'u' | 'c' => {
                let _ = client.post_feedback(&item.id, "context");
                let prompt = untrusted_context_block(&item);
                self.toast = Some("Attached to agent · dock stays open".into());
                self.dock_focused = false;
                InputOutcome::Action(Action::SendPrompt(prompt))
            }
            // Explain
            'e' => {
                let _ = client.post_feedback(&item.id, "explain");
                let prompt = explain_prompt(&item);
                self.toast = Some("Explain → agent".into());
                self.dock_focused = false;
                InputOutcome::Action(Action::SendPrompt(prompt))
            }
            // Discuss (alias)
            'd' => {
                let _ = client.post_feedback(&item.id, "discuss");
                let prompt = discuss_prompt(&item);
                self.toast = Some("Discuss → agent".into());
                self.dock_focused = false;
                InputOutcome::Action(Action::SendPrompt(prompt))
            }
            // store
            's' => {
                match client.post_feedback(&item.id, "store") {
                    Ok(()) => self.toast = Some("Stored ✓".into()),
                    Err(err) => self.toast = Some(format!("Store failed: {err}")),
                }
                InputOutcome::Changed
            }
            // open link
            'o' => {
                if let Some(url) = item.open_url() {
                    let _ = client.post_feedback(&item.id, "open");
                    return InputOutcome::Action(Action::OpenUrl(url));
                }
                self.toast = Some("No link on this card".into());
                InputOutcome::Changed
            }
            // dismiss
            'x' => {
                let _ = client.post_feedback(&item.id, "dismiss");
                if let Some(idx) = self.items.iter().position(|i| i.id == item.id) {
                    self.items.remove(idx);
                    if self.selected >= self.items.len() {
                        self.selected = self.items.len().saturating_sub(1);
                    }
                }
                self.toast = Some("Dismissed".into());
                InputOutcome::Changed
            }
            // Enter = use
            _ if ch == '\n' => unreachable!(),
            // post
            'p' => {
                let title = format!("Update from {}", client.user_id);
                let body = "Posted from Feeder.".to_string();
                match client.post_item(&title, &body, Some("repo:demo")) {
                    Ok(posted) => {
                        self.items.insert(0, posted);
                        self.selected = 0;
                        self.toast = Some("Posted ✓".into());
                    }
                    Err(err) => self.toast = Some(format!("Post failed: {err}")),
                }
                InputOutcome::Changed
            }
            // media
            'v' => {
                if let Some(url) = item
                    .media
                    .0
                    .iter()
                    .find_map(|m| m.url.clone().filter(|u| !u.is_empty()))
                {
                    return InputOutcome::Action(Action::OpenUrl(url));
                }
                self.toast = Some("No media URL".into());
                InputOutcome::Changed
            }
            _ => InputOutcome::Unchanged,
        }
    }

    /// Footer help line.
    pub fn help_line() -> &'static str {
        "Tab focus · j/k · u use · e explain · x dismiss · o open · r refresh · q close"
    }
}
