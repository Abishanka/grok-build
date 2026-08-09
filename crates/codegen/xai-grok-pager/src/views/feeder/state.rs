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
/// How often to refresh jam plane strip while jammed.
const PLANES_REFRESH: Duration = Duration::from_secs(4);

type RefreshMsg = Result<(Vec<FeedItem>, String), String>;
type PlanesMsg = Result<Vec<PlaneRow>, String>;

/// Bound jam session (one shared feed while set).
#[derive(Debug, Clone)]
pub struct JamBinding {
    pub jam_id: String,
    pub plane_id: String,
    pub token: String,
    pub join_url: String,
    pub title: String,
    pub origin: String,
}

/// Peer plane strip row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaneRow {
    pub origin: String,
    pub status: String,
    pub status_detail: Option<String>,
    pub headline: Option<String>,
}

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
    pub force_refresh: bool,
    /// Active jam — when set, feed is shared jam scope (no Personal|Jam toggle).
    pub jam: Option<JamBinding>,
    /// Live plane strip for jam.
    pub planes: Vec<PlaneRow>,
    /// Last jam brief for soft autotx inject.
    pub jam_brief: Option<String>,
    /// Stream event seq counter for uplink.
    stream_seq: u64,
    /// Background plane strip fetch.
    planes_pending: Option<Receiver<PlanesMsg>>,
    last_planes_at: Option<Instant>,
    /// Coalesce agent text chunks before uplink (ms window).
    agent_text_buf: String,
    agent_text_flushed_at: Option<Instant>,
    /// Last peer event id seen (for catch-up).
    last_event_id: i64,
    /// Background peer events fetch.
    events_pending: Option<Receiver<Result<(i64, Vec<String>), String>>>,
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
            jam: None,
            planes: Vec::new(),
            jam_brief: None,
            stream_seq: 0,
            planes_pending: None,
            last_planes_at: None,
            agent_text_buf: String::new(),
            agent_text_flushed_at: None,
            last_event_id: 0,
            events_pending: None,
        };
        // Resume jam bind if previous session left one active.
        if let Some(bind) = load_jam_bind() {
            s.jam = Some(bind);
            s.start_planes_refresh();
            s.start_events_catchup();
        }
        s.start_refresh(None);
        s
    }

    /// Whether feeder is bound to a multiplayer jam (shared feed).
    pub fn in_jam(&self) -> bool {
        self.jam.is_some()
    }

    /// Record a user prompt into the work index and schedule a refresh.
    pub fn note_user_prompt(&mut self, text: &str) {
        self.work.push(text);
        self.force_refresh = true;
        // Uplink to jam as ACP-shaped user_message (real stream, not summary).
        self.uplink_user_prompt(text);
        // Allow overlapping: drop stale pending so new context wins soon
        if self.pending.is_none() {
            self.start_refresh(None);
        }
    }

    fn uplink_user_prompt(&mut self, text: &str) {
        let Some(jam) = self.jam.clone() else {
            return;
        };
        let cleaned = text.trim();
        if cleaned.is_empty() || cleaned.starts_with('/') {
            return;
        }
        self.stream_seq = self.stream_seq.saturating_add(1);
        let seq = self.stream_seq;
        let client = FeedClient::from_env();
        let ev = serde_json::json!({
            "seq": seq,
            "kind": "user_message",
            "text": cleaned.chars().take(400).collect::<String>(),
            "title": "prompt",
        });
        std::thread::Builder::new()
            .name("jam-uplink".into())
            .spawn(move || {
                let _ = client.jam_stream_events(
                    &jam.jam_id,
                    &jam.plane_id,
                    &jam.token,
                    &[ev],
                );
            })
            .ok();
    }

    /// Snapshot current slate as seed JSON for jam create/join promote.
    pub fn seed_items_json(&self) -> Vec<serde_json::Value> {
        self.items
            .iter()
            .take(SLATE_LIMIT)
            .map(|it| {
                let text = it.post_text().to_string();
                let title = it
                    .title
                    .clone()
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| text.chars().take(80).collect());
                serde_json::json!({
                    "id": it.id,
                    "title": title,
                    "text": text,
                    "body_md": text,
                    "kind": format!("{:?}", it.kind()).to_ascii_lowercase(),
                })
            })
            .collect()
    }

    /// Persist jam slate locally then clear jam bind (demote to local feed).
    pub fn leave_jam_local(&mut self) {
        if let Some(jam) = self.jam.take() {
            self.persist_jam_snapshot(&jam);
            self.toast = Some(format!(
                "Jam ended · snapshot saved · local feed · {}",
                jam.jam_id
            ));
        }
        clear_jam_bind();
        self.planes.clear();
        self.jam_brief = None;
        self.planes_pending = None;
        self.last_planes_at = None;
        self.force_refresh = true;
        self.start_refresh(None);
    }

    /// Call after successful start/join to persist bind for resume.
    pub fn bind_jam(&mut self, jam: JamBinding) {
        save_jam_bind(&jam);
        self.jam = Some(jam);
        self.last_planes_at = None;
        self.last_event_id = 0;
        self.start_planes_refresh();
        self.start_events_catchup();
        self.force_refresh = true;
        self.start_refresh(None);
    }

    fn persist_jam_snapshot(&self, jam: &JamBinding) {
        let home = std::env::var_os("GROK_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".grok")))
            .unwrap_or_else(|| std::path::PathBuf::from(".grok"));
        let dir = home.join("feeder").join("jams");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{}-final.json", jam.jam_id));
        let payload = serde_json::json!({
            "jam_id": jam.jam_id,
            "title": jam.title,
            "join_url": jam.join_url,
            "saved_at": chrono_lite_now(),
            "items": self.seed_items_json(),
        });
        if let Ok(s) = serde_json::to_string_pretty(&payload) {
            let _ = std::fs::write(path, s);
        }
    }

    /// Refresh plane strip from server (non-blocking).
    pub fn refresh_planes(&mut self) {
        self.start_planes_refresh();
    }

    fn start_planes_refresh(&mut self) {
        if self.jam.is_none() || self.planes_pending.is_some() {
            return;
        }
        let Some(jam) = self.jam.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.planes_pending = Some(rx);
        let client = FeedClient::from_env();
        std::thread::Builder::new()
            .name("jam-planes".into())
            .spawn(move || {
                let result = client
                    .jam_planes(&jam.jam_id, Some(&jam.token))
                    .map(|raw| {
                        raw.into_iter()
                            .filter_map(|p| {
                                Some(PlaneRow {
                                    origin: p.get("origin")?.as_str()?.to_string(),
                                    status: p
                                        .get("status")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("idle")
                                        .to_string(),
                                    status_detail: p
                                        .get("status_detail")
                                        .and_then(|s| s.as_str())
                                        .map(|s| s.to_string()),
                                    headline: p
                                        .get("headline")
                                        .or_else(|| p.get("digest_md"))
                                        .and_then(|s| s.as_str())
                                        .map(|s| s.to_string()),
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .map_err(|e| e.to_string());
                let _ = tx.send(result);
            })
            .ok();
    }

    /// Uplink a tool-call shaped event (call from dispatch when tools fire).
    pub fn uplink_tool(&mut self, name: &str, title: &str, status: &str) {
        self.uplink_raw_event(
            "tool_call",
            Some(title),
            None,
            Some(serde_json::json!({ "name": name, "status": status })),
        );
    }

    /// Coalesce agent tokens; flush every ~200ms or 400 chars.
    pub fn uplink_agent_text_chunk(&mut self, text: String) {
        if text.is_empty() || self.jam.is_none() {
            return;
        }
        self.agent_text_buf.push_str(&text);
        let due = match self.agent_text_flushed_at {
            None => true,
            Some(t) => t.elapsed() >= Duration::from_millis(200),
        };
        if due || self.agent_text_buf.len() >= 400 {
            self.flush_agent_text_buf();
        }
    }

    fn flush_agent_text_buf(&mut self) {
        if self.agent_text_buf.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.agent_text_buf);
        self.agent_text_flushed_at = Some(Instant::now());
        // Only uplink meaningful chunks
        let trimmed = text.trim();
        if trimmed.len() < 12 {
            return;
        }
        self.uplink_raw_event(
            "agent_message_chunk",
            Some("agent"),
            Some(trimmed),
            None,
        );
    }

    /// Generic jam stream uplink (ACP-aligned).
    pub fn uplink_raw_event(
        &mut self,
        kind: &str,
        title: Option<&str>,
        text: Option<&str>,
        tool: Option<serde_json::Value>,
    ) {
        let Some(jam) = self.jam.clone() else {
            return;
        };
        self.stream_seq = self.stream_seq.saturating_add(1);
        let seq = self.stream_seq;
        let client = FeedClient::from_env();
        let mut ev = serde_json::json!({
            "seq": seq,
            "kind": kind,
        });
        if let Some(t) = title {
            ev["title"] = serde_json::json!(t);
        }
        if let Some(t) = text {
            ev["text"] = serde_json::json!(t.chars().take(800).collect::<String>());
        }
        if let Some(tool) = tool {
            ev["tool"] = tool;
        }
        std::thread::Builder::new()
            .name("jam-uplink".into())
            .spawn(move || {
                let _ = client.jam_stream_events(
                    &jam.jam_id,
                    &jam.plane_id,
                    &jam.token,
                    &[ev],
                );
            })
            .ok();
    }

    fn start_events_catchup(&mut self) {
        if self.jam.is_none() || self.events_pending.is_some() {
            return;
        }
        let Some(jam) = self.jam.clone() else {
            return;
        };
        let since = self.last_event_id;
        let (tx, rx) = mpsc::channel();
        self.events_pending = Some(rx);
        let client = FeedClient::from_env();
        std::thread::Builder::new()
            .name("jam-events".into())
            .spawn(move || {
                let result = client
                    .jam_events_since(&jam.jam_id, Some(&jam.token), since, 50)
                    .map(|(last, headlines)| (last, headlines))
                    .map_err(|e| e.to_string());
                let _ = tx.send(result);
            })
            .ok();
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
        let jam_id = self.jam.as_ref().map(|j| j.jam_id.clone());
        let jam_label = self
            .jam
            .as_ref()
            .map(|j| format!("jam:{}", j.title))
            .unwrap_or_else(|| "solo".into());
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.loading = true;
        if self.toast.is_none() || self.toast.as_deref().is_some_and(|t| t.contains("offline")) {
            self.toast = Some(format!("Refreshing · {jam_label} · {base}"));
        }
        std::thread::Builder::new()
            .name("feeder-refresh".into())
            .spawn(move || {
                let ws = if jam_id.is_some() {
                    jam_id.as_ref().map(|id| format!("jam:{id}"))
                } else {
                    cwd.as_deref().map(|c| format!("cwd:{c}"))
                };
                let result = match client.query_feed_ex(
                    &prompts,
                    ws.as_deref(),
                    jam_id.as_deref(),
                    None,
                    None,
                    cwd.as_deref(),
                    SLATE_LIMIT,
                ) {
                    Ok(items) if !items.is_empty() => {
                        let mut items = filter_timeline(items);
                        // jam seeds may be user_post — keep non-empty slate
                        if items.is_empty() {
                            // fall through empty
                        }
                        items.truncate(SLATE_LIMIT);
                        let n = items.len();
                        Ok((items, format!("{jam_label} · {n} · {base}")))
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

        // Jam plane strip — background poll
        if let Some(rx) = self.planes_pending.take() {
            match rx.try_recv() {
                Ok(Ok(planes)) => {
                    if planes != self.planes {
                        self.planes = planes;
                        changed = true;
                    }
                    self.last_planes_at = Some(Instant::now());
                }
                Ok(Err(_)) => {
                    self.last_planes_at = Some(Instant::now());
                }
                Err(TryRecvError::Empty) => {
                    self.planes_pending = Some(rx);
                }
                Err(TryRecvError::Disconnected) => {
                    self.last_planes_at = Some(Instant::now());
                }
            }
        } else if self.jam.is_some() {
            let due = match self.last_planes_at {
                None => true,
                Some(t) => t.elapsed() >= PLANES_REFRESH,
            };
            if due {
                self.start_planes_refresh();
            }
        }

        // While jammed, refresh feed a bit more often so activity cards appear.
        if self.jam.is_some()
            && !self.loading
            && self.pending.is_none()
            && self
                .last_refresh_at
                .is_some_and(|t| t.elapsed() >= Duration::from_secs(12))
        {
            self.start_refresh(None);
            changed = true;
        }

        // Peer event catch-up (headlines + force feed refresh on new activity)
        if let Some(rx) = self.events_pending.take() {
            match rx.try_recv() {
                Ok(Ok((last_id, headlines))) => {
                    if last_id > self.last_event_id {
                        self.last_event_id = last_id;
                        if !headlines.is_empty() {
                            // Show latest peer activity in toast
                            if let Some(h) = headlines.last() {
                                self.toast = Some(format!("peer · {h}"));
                            }
                            // Pull shared feed so activity cards appear
                            if self.pending.is_none() {
                                self.start_refresh(None);
                            }
                            changed = true;
                        }
                    }
                }
                Ok(Err(_)) => {}
                Err(TryRecvError::Empty) => {
                    self.events_pending = Some(rx);
                }
                Err(TryRecvError::Disconnected) => {}
            }
        } else if self.jam.is_some() && self.spinner_tick % 20 == 0 {
            self.start_events_catchup();
        }

        // Flush coalesced agent text periodically
        if !self.agent_text_buf.is_empty() {
            let due = self
                .agent_text_flushed_at
                .is_none_or(|t| t.elapsed() >= Duration::from_millis(250));
            if due {
                self.flush_agent_text_buf();
            }
        }

        changed
    }

    fn dock_open_wants_auto_refresh(&self) -> bool {
        // Caller only polls while dock is open; we just gate on interval.
        let interval = if self.jam.is_some() {
            Duration::from_secs(12)
        } else {
            AUTO_REFRESH
        };
        match self.last_refresh_at {
            None => !self.loading,
            Some(t) => t.elapsed() >= interval && !self.loading,
        }
    }

    /// Whether the event loop should keep ticking for this dock.
    pub fn needs_tick(&self) -> bool {
        if self.loading || self.pending.is_some() || self.last_refresh_at.is_none() {
            return true;
        }
        if self.jam.is_some() {
            // Keep ticking for plane strip + faster jam feed refresh.
            return true;
        }
        if self.planes_pending.is_some() {
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

    pub fn help_line_jam() -> &'static str {
        "JAM · j/k · u use · r refresh · /feeder jam leave · Esc agent · q close"
    }
}

fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn jam_bind_path() -> std::path::PathBuf {
    let home = std::env::var_os("GROK_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".grok")))
        .unwrap_or_else(|| std::path::PathBuf::from(".grok"));
    home.join("feeder").join("active-jam.json")
}

fn save_jam_bind(jam: &JamBinding) {
    let path = jam_bind_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let v = serde_json::json!({
        "jam_id": jam.jam_id,
        "plane_id": jam.plane_id,
        "token": jam.token,
        "join_url": jam.join_url,
        "title": jam.title,
        "origin": jam.origin,
    });
    if let Ok(s) = serde_json::to_string_pretty(&v) {
        let _ = std::fs::write(path, s);
    }
}

fn clear_jam_bind() {
    let _ = std::fs::remove_file(jam_bind_path());
}

fn load_jam_bind() -> Option<JamBinding> {
    let path = jam_bind_path();
    let s = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    Some(JamBinding {
        jam_id: v.get("jam_id")?.as_str()?.to_string(),
        plane_id: v.get("plane_id")?.as_str()?.to_string(),
        token: v.get("token")?.as_str()?.to_string(),
        join_url: v
            .get("join_url")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        title: v
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("jam")
            .to_string(),
        origin: v
            .get("origin")
            .and_then(|x| x.as_str())
            .unwrap_or("member")
            .to_string(),
    })
}
