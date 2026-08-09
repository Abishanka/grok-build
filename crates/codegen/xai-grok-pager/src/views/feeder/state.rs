//! Feeder view state — selection, scroll, toast, live or mock items.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind};

use crate::actions::ActionRegistry;
use crate::app::actions::Action;
use crate::app::app_view::InputOutcome;

use super::feed_client::{
    discuss_prompt, explain_prompt, untrusted_context_block, FeedClient,
};
use super::row::{filter_timeline, load_mock_items, FeedItem, SLATE_LIMIT};
use super::work_context::WorkContext;

/// How often the dock re-queries the live API while open.
const AUTO_REFRESH: Duration = Duration::from_secs(25);

type RefreshMsg = Result<(Vec<FeedItem>, String), String>;

/// In-memory state for the Feeder dock.
#[derive(Debug)]
pub struct FeederState {
    /// Ranked feed cards (at most [`SLATE_LIMIT`]).
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
    /// Rolling user-work index (prompts) for personalized search.
    pub work: WorkContext,
    /// In-flight background feed fetch (never blocks the TUI thread).
    pending: Option<Receiver<RefreshMsg>>,
    /// Last successful (or failed) refresh attempt — drives auto-refresh.
    last_refresh_at: Option<Instant>,
    /// True while a background fetch is outstanding.
    pub loading: bool,
    /// Force next refresh even if one just finished (after new user prompt).
    force_refresh: bool,
}

impl Default for FeederState {
    fn default() -> Self {
        Self::new()
    }
}

impl FeederState {
    /// Fresh state: show fixtures immediately, kick off live fetch in background.
    pub fn new() -> Self {
        let mut s = Self {
            items: load_mock_items()
                .into_iter()
                .take(SLATE_LIMIT)
                .collect(),
            selected: 0,
            scroll: 0,
            peek_scroll: 0,
            toast: Some("Feeder · connecting…".into()),
            spinner_tick: 0,
            live: false,
            compose_title: None,
            dock_focused: true,
            work: WorkContext::new(),
            pending: None,
            last_refresh_at: None,
            loading: false,
            force_refresh: false,
        };
        s.start_refresh(None);
        s
    }

    /// Record a user prompt into the work index and schedule a refresh.
    pub fn note_user_prompt(&mut self, text: &str) {
        self.work.push(text);
        self.force_refresh = true;
        // Allow overlapping: drop stale pending so new context wins soon
        if self.pending.is_none() {
            self.start_refresh(None);
        }
    }

    /// Seed work index from agent prompt history (newest first).
    pub fn seed_work_history(&mut self, history: &[String]) {
        self.work.seed_from_history(history);
    }

    /// Kick a non-blocking live reload from current work context.
    pub fn start_refresh(&mut self, hint: Option<&str>) {
        if self.pending.is_some() && !self.force_refresh {
            return;
        }
        // If forcing while in-flight, still skip starting a second thread —
        // poll will start one when the current finishes if force_refresh set.
        if self.pending.is_some() {
            return;
        }
        self.force_refresh = false;
        if let Some(h) = hint {
            self.work.push(h);
        }
        let client = FeedClient::from_env();
        let base = client.base_url.clone();
        let prompts = self.work.prompts_for_query();
        let cwd = std::env::current_dir()
            .ok()
            .map(|p| p.display().to_string());
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.loading = true;
        if self.toast.is_none() || self.toast.as_deref().is_some_and(|t| t.contains("offline")) {
            self.toast = Some(format!("Refreshing · {base}"));
        }
        std::thread::Builder::new()
            .name("feeder-refresh".into())
            .spawn(move || {
                let result = match client.query_feed(
                    &prompts,
                    cwd.as_deref().map(|c| format!("cwd:{c}")).as_deref(),
                    None,
                    None,
                    cwd.as_deref(),
                    SLATE_LIMIT,
                ) {
                    Ok(items) if !items.is_empty() => {
                        let mut items = filter_timeline(items);
                        items.truncate(SLATE_LIMIT);
                        let n = items.len();
                        let qhint = prompts
                            .first()
                            .map(|p| {
                                let t: String = p.chars().take(28).collect();
                                format!(" · q:{t}")
                            })
                            .unwrap_or_default();
                        Ok((items, format!("live · {n}{qhint}")))
                    }
                    Ok(_) => Err(format!("empty response from {base}")),
                    Err(err) => Err(format!("{err}")),
                };
                let _ = tx.send(result);
            })
            .ok();
    }

    /// Prefer [`Self::start_refresh`] — this only enqueues background work.
    pub fn refresh_from_service(&mut self, hint: Option<&str>) {
        self.start_refresh(hint);
    }

    /// Poll background fetch + schedule auto-refresh. Returns true if UI should redraw.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        self.spinner_tick = self.spinner_tick.wrapping_add(1);

        // Half-block image downloads
        if let Ok(mut cache) = super::media_preview::global_cache().lock() {
            changed |= cache.poll();
        }

        if let Some(rx) = self.pending.take() {
            match rx.try_recv() {
                Ok(Ok((items, label))) => {
                    self.items = items;
                    self.live = true;
                    self.selected = self.selected.min(self.items.len().saturating_sub(1));
                    self.scroll = 0;
                    self.peek_scroll = 0;
                    self.toast = Some(format!("Feeder · {label}"));
                    self.loading = false;
                    self.last_refresh_at = Some(Instant::now());
                    changed = true;
                    if self.force_refresh {
                        self.start_refresh(None);
                    }
                }
                Ok(Err(err)) => {
                    if self.items.is_empty() {
                        self.items = load_mock_items()
                            .into_iter()
                            .take(SLATE_LIMIT)
                            .collect();
                    }
                    self.live = false;
                    self.toast = Some(format!("Feeder · offline ({err})"));
                    self.loading = false;
                    self.last_refresh_at = Some(Instant::now());
                    changed = true;
                    if self.force_refresh {
                        self.start_refresh(None);
                    }
                }
                Err(TryRecvError::Empty) => {
                    // Still in flight — put receiver back.
                    self.pending = Some(rx);
                    // Pulse toast so the dock feels alive while loading.
                    if self.spinner_tick % 8 == 0 {
                        let dots = match (self.spinner_tick / 8) % 3 {
                            0 => ".",
                            1 => "..",
                            _ => "...",
                        };
                        self.toast = Some(format!("Feeder · loading{dots}"));
                        changed = true;
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.loading = false;
                    self.last_refresh_at = Some(Instant::now());
                    self.toast = Some("Feeder · refresh failed".into());
                    changed = true;
                }
            }
        } else if self.force_refresh || self.dock_open_wants_auto_refresh() {
            self.start_refresh(None);
            changed = true;
        }

        changed
    }

    fn dock_open_wants_auto_refresh(&self) -> bool {
        // Caller only polls while dock is open; we just gate on interval.
        match self.last_refresh_at {
            None => !self.loading,
            Some(t) => t.elapsed() >= AUTO_REFRESH && !self.loading,
        }
    }

    /// Whether the event loop should keep ticking for this dock.
    pub fn needs_tick(&self) -> bool {
        if self.loading || self.pending.is_some() || self.last_refresh_at.is_none() {
            return true;
        }
        super::media_preview::global_cache()
            .lock()
            .map(|c| c.needs_tick())
            .unwrap_or(false)
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

    /// Route input while the feeder dock has focus.
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
                MouseEventKind::Down(_) => {
                    // Click focuses; selection change is handled by AppView hit-test.
                    InputOutcome::Changed
                }
                _ => InputOutcome::Unchanged,
            },
            Event::Resize(_, _) => InputOutcome::Changed,
            _ => InputOutcome::Unchanged,
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> InputOutcome {
        // q closes dock. Esc is handled by AppView (unfocus first).
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
            && key.modifiers == KeyModifiers::NONE
        {
            return InputOutcome::Action(Action::CloseFeeder);
        }

        // Accept keys with no mods, or only SHIFT (some terminals tag letters).
        let nav_ok = key.modifiers == KeyModifiers::NONE
            || key.modifiers == KeyModifiers::SHIFT;

        // Navigation — also accept arrows always
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') if nav_ok => {
                self.move_selection(-1, 12);
                return InputOutcome::Changed;
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') if nav_ok => {
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
            KeyCode::Char('r') | KeyCode::Char('R') if nav_ok => {
                self.toast = Some("Refreshing…".into());
                // Allow force refresh even if one is pending by dropping stale? keep simple.
                if self.pending.is_none() {
                    self.start_refresh(None);
                }
                return InputOutcome::Changed;
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                return self.handle_action_key('u');
            }
            // Swallow Esc here so it never falls through; AppView usually catches first.
            KeyCode::Esc => {
                self.dock_focused = false;
                self.toast = Some("Focus: Agent".into());
                return InputOutcome::Changed;
            }
            _ => {}
        }

        // Card actions — ignore control/alt so we don't steal chorded agent keys if focus glitches
        if key.modifiers == KeyModifiers::NONE
            && let KeyCode::Char(ch) = key.code
        {
            let lower = ch.to_ascii_lowercase();
            return self.handle_action_key(lower);
        }

        // When focused, consume leftover plain keys so they don't type into the agent.
        if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT {
            return InputOutcome::Unchanged;
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
        "j/k · u use · e explain · x dismiss · o open · r refresh · Esc agent · q close"
    }
}
