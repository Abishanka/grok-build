//! Feeder view state — selection, scroll, toast TTL, ~20-post ring.

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind};

use crate::actions::ActionRegistry;
use crate::app::actions::Action;
use crate::app::app_view::InputOutcome;

use super::feed_client::{
    discuss_prompt, explain_prompt, untrusted_context_block, FeedClient, SessionInfo,
};
use super::row::{
    filter_timeline, load_mock_items, FeedItem, DOCK_CAP, FETCH_BATCH, FETCH_INITIAL,
};
use super::work_context::WorkContext;

/// How often the dock re-queries the live API while open.
const AUTO_REFRESH: Duration = Duration::from_secs(25);
/// Footer toast auto-clear (Dismissed, Stored, etc.).
const TOAST_TTL: Duration = Duration::from_millis(2500);
/// Live status line lasts a bit longer.
const TOAST_TTL_STATUS: Duration = Duration::from_secs(6);

type RefreshMsg = Result<(Vec<FeedItem>, String), String>;

/// In-memory state for the Feeder dock.
#[derive(Debug)]
pub struct FeederState {
    /// Ranked feed cards (at most [`DOCK_CAP`]).
    pub items: Vec<FeedItem>,
    /// Selected row index into `items` (0 when empty).
    pub selected: usize,
    /// First visible row index (vertical scroll).
    pub scroll: usize,
    /// Scroll offset inside the peek body (lines).
    pub peek_scroll: usize,
    /// Transient status / toast (footer).
    pub toast: Option<String>,
    /// When `toast` should clear (None = sticky until replaced).
    toast_until: Option<Instant>,
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
    /// Ids dismissed this session — skipped on merge so they don't bounce back.
    dismissed: HashSet<String>,
    /// Current Feeder session (ensured on first refresh).
    session: Option<SessionInfo>,
    /// In-flight background feed fetch (never blocks the TUI thread).
    pending: Option<Receiver<RefreshMsg>>,
    /// Last successful (or failed) refresh attempt — drives auto-refresh.
    last_refresh_at: Option<Instant>,
    /// True while a background fetch is outstanding.
    pub loading: bool,
    /// Force next refresh even if one just finished (after new user prompt).
    force_refresh: bool,
    /// True until the first successful live merge (use larger fetch).
    cold_start: bool,
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
            items: load_mock_items().into_iter().take(DOCK_CAP).collect(),
            selected: 0,
            scroll: 0,
            peek_scroll: 0,
            toast: Some("Feeder · connecting…".into()),
            toast_until: Some(Instant::now() + TOAST_TTL_STATUS),
            spinner_tick: 0,
            live: false,
            compose_title: None,
            dock_focused: true,
            work: WorkContext::new(),
            dismissed: HashSet::new(),
            session: None,
            pending: None,
            last_refresh_at: None,
            loading: false,
            force_refresh: false,
            cold_start: true,
        };
        s.start_refresh(None);
        s
    }

    fn set_toast(&mut self, msg: impl Into<String>, ttl: Duration) {
        self.toast = Some(msg.into());
        self.toast_until = Some(Instant::now() + ttl);
    }

    /// Record a user prompt into the work index and schedule a refresh.
    pub fn note_user_prompt(&mut self, text: &str) {
        self.work.push(text);
        self.force_refresh = true;
        // Post moment in background (queued with prompts).
        if let Some(sid) = self.session.as_ref().map(|s| s.session_id.clone()) {
            let prompts = self.work.prompts_for_query();
            let client = FeedClient::from_env();
            std::thread::spawn(move || {
                let _ = client.post_moment(&sid, &prompts);
            });
        }
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
        let workspace_key = super::work_context::workspace_key_from_cwd();
        // Ensure session once. Failure is non-fatal — still query feed (degraded).
        let session_id = if self.session.is_none() {
            match client.ensure_session(&workspace_key) {
                Ok(sess) => {
                    let sid = sess.session_id.clone();
                    self.session = Some(sess);
                    Some(sid)
                }
                Err(err) => {
                    tracing::warn!(%err, "feeder: ensure_session failed — querying without session");
                    None
                }
            }
        } else {
            self.session.as_ref().map(|s| s.session_id.clone())
        };
        // Cold start: fill dock; later refreshes pull a small batch to merge.
        let fetch_n = if self.cold_start || self.items.is_empty() {
            FETCH_INITIAL
        } else {
            FETCH_BATCH
        };
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.loading = true;
        if self.toast.is_none()
            || self
                .toast
                .as_deref()
                .is_some_and(|t| t.contains("offline") || t.contains("Dismissed"))
        {
            self.set_toast(format!("Refreshing · {base}"), TOAST_TTL_STATUS);
        }
        let session_id_for_thread = session_id.clone();
        std::thread::Builder::new()
            .name("feeder-refresh".into())
            .spawn(move || {
                let result = match client.query_feed(
                    &prompts,
                    Some(&workspace_key),
                    None,
                    None,
                    cwd.as_deref(),
                    fetch_n,
                    session_id_for_thread.as_deref(),
                ) {
                    Ok(items) if !items.is_empty() => {
                        let mut items = filter_timeline(items);
                        // Don't pre-truncate to DOCK_CAP here — merge handles cap.
                        items.truncate(fetch_n.max(FETCH_BATCH));
                        let n = items.len();
                        let qhint = prompts
                            .first()
                            .map(|p| {
                                let t: String = p.chars().take(28).collect();
                                format!(" · q:{t}")
                            })
                            .unwrap_or_default();
                        Ok((items, format!("+{n}{qhint}")))
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

    /// Prepend `incoming`, keep existing (minus dismissed/dupes), cap at [`DOCK_CAP`].
    fn merge_slate(&mut self, incoming: Vec<FeedItem>) {
        let mut seen = HashSet::new();
        let mut out = Vec::with_capacity(DOCK_CAP);
        for it in incoming {
            if self.dismissed.contains(&it.id) {
                continue;
            }
            if seen.insert(it.id.clone()) {
                out.push(it);
            }
        }
        for it in self.items.drain(..) {
            if self.dismissed.contains(&it.id) {
                continue;
            }
            if seen.insert(it.id.clone()) {
                out.push(it);
            }
        }
        // Evict from the tail when over cap (oldest / previously lower-ranked).
        out.truncate(DOCK_CAP);
        self.items = out;
        if self.selected >= self.items.len() {
            self.selected = self.items.len().saturating_sub(1);
        }
    }

    /// Poll background fetch + schedule auto-refresh. Returns true if UI should redraw.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        self.spinner_tick = self.spinner_tick.wrapping_add(1);

        // Toast TTL
        if let Some(until) = self.toast_until {
            if Instant::now() >= until {
                self.toast = None;
                self.toast_until = None;
                changed = true;
            }
        }

        if let Ok(mut cache) = super::media_preview::global_cache().lock() {
            changed |= cache.poll();
        }

        if let Some(rx) = self.pending.take() {
            match rx.try_recv() {
                Ok(Ok((items, label))) => {
                    let added = items.len();
                    self.merge_slate(items);
                    self.live = true;
                    self.cold_start = false;
                    self.peek_scroll = 0;
                    // Don't jump scroll to 0 on every merge — keep place unless empty before
                    if self.scroll >= self.items.len() {
                        self.scroll = 0;
                    }
                    let total = self.items.len();
                    self.set_toast(
                        format!("Feeder · live · {total} · {label}"),
                        TOAST_TTL_STATUS,
                    );
                    self.loading = false;
                    self.last_refresh_at = Some(Instant::now());
                    let _ = added;
                    changed = true;
                    // Post moment in background after successful refresh.
                    if let Some(sid) = self.session.as_ref().map(|s| s.session_id.clone()) {
                        let prompts = self.work.prompts_for_query();
                        let client = FeedClient::from_env();
                        std::thread::spawn(move || {
                            let _ = client.post_moment(&sid, &prompts);
                        });
                    }
                    if self.force_refresh {
                        self.start_refresh(None);
                    }
                }
                Ok(Err(err)) => {
                    if self.items.is_empty() {
                        self.items = load_mock_items().into_iter().take(DOCK_CAP).collect();
                    }
                    self.live = false;
                    self.set_toast(format!("Feeder · offline ({err})"), TOAST_TTL_STATUS);
                    self.loading = false;
                    self.last_refresh_at = Some(Instant::now());
                    changed = true;
                    if self.force_refresh {
                        self.start_refresh(None);
                    }
                }
                Err(TryRecvError::Empty) => {
                    self.pending = Some(rx);
                    if self.spinner_tick % 8 == 0 {
                        let dots = match (self.spinner_tick / 8) % 3 {
                            0 => ".",
                            1 => "..",
                            _ => "...",
                        };
                        // Loading pulse — short sticky while in flight
                        self.toast = Some(format!("Feeder · loading{dots}"));
                        self.toast_until = Some(Instant::now() + Duration::from_secs(2));
                        changed = true;
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.loading = false;
                    self.last_refresh_at = Some(Instant::now());
                    self.set_toast("Feeder · refresh failed", TOAST_TTL);
                    changed = true;
                }
            }
        } else if self.force_refresh || self.dock_open_wants_auto_refresh() {
            self.start_refresh(None);
            changed = true;
        }

        // Heartbeat if we have a session (background, fire-and-forget; clone to 'static).
        if let Some(sess) = &self.session {
            let sid = sess.session_id.clone();
            let client = FeedClient::from_env();
            let _ = std::thread::spawn(move || {
                let _ = client.heartbeat(&sid);
            });
        }

        changed
    }

    fn dock_open_wants_auto_refresh(&self) -> bool {
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
        if self.toast_until.is_some() {
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
                MouseEventKind::Down(_) => InputOutcome::Changed,
                _ => InputOutcome::Unchanged,
            },
            Event::Resize(_, _) => InputOutcome::Changed,
            _ => InputOutcome::Unchanged,
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> InputOutcome {
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
            && key.modifiers == KeyModifiers::NONE
        {
            return InputOutcome::Action(Action::CloseFeeder);
        }

        let nav_ok =
            key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT;

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
                self.set_toast("Refreshing…", TOAST_TTL);
                if self.pending.is_none() {
                    self.start_refresh(None);
                }
                return InputOutcome::Changed;
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                return self.handle_action_key('u');
            }
            KeyCode::Esc => {
                self.dock_focused = false;
                self.set_toast("Focus: Agent", TOAST_TTL);
                return InputOutcome::Changed;
            }
            _ => {}
        }

        if key.modifiers == KeyModifiers::NONE
            && let KeyCode::Char(ch) = key.code
        {
            let lower = ch.to_ascii_lowercase();
            return self.handle_action_key(lower);
        }

        if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT {
            return InputOutcome::Unchanged;
        }
        InputOutcome::Unchanged
    }

    fn handle_action_key(&mut self, ch: char) -> InputOutcome {
        let client = FeedClient::from_env();
        let Some(item) = self.selected_item().cloned() else {
            self.set_toast("No card selected", TOAST_TTL);
            return InputOutcome::Changed;
        };

        match ch {
            'u' | 'c' => {
                let _ = client.post_feedback(&item.id, "context");
                let prompt = untrusted_context_block(&item);
                self.set_toast("Attached to agent · dock stays open", TOAST_TTL);
                self.dock_focused = false;
                InputOutcome::Action(Action::SendPrompt(prompt))
            }
            'e' => {
                let _ = client.post_feedback(&item.id, "explain");
                let prompt = explain_prompt(&item);
                self.set_toast("Explain → agent", TOAST_TTL);
                self.dock_focused = false;
                InputOutcome::Action(Action::SendPrompt(prompt))
            }
            'd' => {
                let _ = client.post_feedback(&item.id, "discuss");
                let prompt = discuss_prompt(&item);
                self.set_toast("Discuss → agent", TOAST_TTL);
                self.dock_focused = false;
                InputOutcome::Action(Action::SendPrompt(prompt))
            }
            's' => {
                match client.post_feedback(&item.id, "store") {
                    Ok(()) => self.set_toast("Stored ✓", TOAST_TTL),
                    Err(err) => self.set_toast(format!("Store failed: {err}"), TOAST_TTL),
                }
                InputOutcome::Changed
            }
            'o' => {
                if let Some(url) = item.open_url() {
                    let _ = client.post_feedback(&item.id, "open");
                    return InputOutcome::Action(Action::OpenUrl(url));
                }
                self.set_toast("No link on this card", TOAST_TTL);
                InputOutcome::Changed
            }
            'x' => {
                let _ = client.post_feedback(&item.id, "dismiss");  // server mutes
                self.dismissed.insert(item.id.clone());
                if let Some(idx) = self.items.iter().position(|i| i.id == item.id) {
                    self.items.remove(idx);
                    if self.selected >= self.items.len() {
                        self.selected = self.items.len().saturating_sub(1);
                    }
                }
                self.set_toast("Dismissed (muted)", TOAST_TTL);
                InputOutcome::Changed
            }
            'p' => {
                let title = format!("Update from {}", client.user_id);
                let body = "Posted from Feeder.".to_string();
                match client.post_item(&title, &body, Some("repo:demo")) {
                    Ok(posted) => {
                        self.items.insert(0, posted);
                        if self.items.len() > DOCK_CAP {
                            self.items.truncate(DOCK_CAP);
                        }
                        self.selected = 0;
                        self.set_toast("Posted ✓", TOAST_TTL);
                    }
                    Err(err) => self.set_toast(format!("Post failed: {err}"), TOAST_TTL),
                }
                InputOutcome::Changed
            }
            'v' => {
                if let Some(url) = item
                    .media
                    .0
                    .iter()
                    .find_map(|m| m.url.clone().filter(|u| !u.is_empty()))
                {
                    return InputOutcome::Action(Action::OpenUrl(url));
                }
                self.set_toast("No media URL", TOAST_TTL);
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
