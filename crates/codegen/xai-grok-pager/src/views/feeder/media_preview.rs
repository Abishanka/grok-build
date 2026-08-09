//! Media download + Kitty inline placement for Feeder cards.
//!
//! Kitty graphics are absolute screen placements and survive cell redraws.
//! Feeder therefore tracks every id placed last frame and:
//! - on **scroll change**: full-refresh — delete *all* previous feeder ids, then re-place
//! - otherwise: delete only ids that left the visible set
//!
//! Fallback (no safe Kitty overlay, or image not ready): half-block cells in the
//! ratatui buffer (always scroll-correct). Text card only while loading / failed.
//!
//! `FEEDER_HALFBLOCK=0` disables half-blocks. Default is on when Kitty path is off.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::terminal::image::{
    clear_kitty_image, detect_graphics_protocol, fit_image_to_cells, place_inline_image,
    prepare_overlay_image_bytes, scrollback_inline_overlay_active, transmit_inline_image,
    GraphicsProtocol,
};

/// Downloaded + prepared image ready for terminal graphics.
#[derive(Debug, Clone)]
pub struct MediaBytes {
    pub url: String,
    pub prepared: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub image_id: u32,
    pub transmitted: bool,
}

/// Half-block row (buffer-local inline fallback).
#[derive(Debug, Clone)]
pub struct PreviewRow {
    pub cells: Vec<(Color, Color)>,
}

#[derive(Debug, Clone)]
pub struct HalfblockPreview {
    pub rows: Vec<PreviewRow>,
}

#[derive(Debug, Default)]
pub struct MediaCache {
    ready: HashMap<String, MediaBytes>,
    halfblocks: HashMap<String, HalfblockPreview>,
    inflight: HashMap<String, ()>,
    rx: Option<Receiver<(String, Option<MediaBytes>)>>,
    tx: Option<mpsc::Sender<(String, Option<MediaBytes>)>>,
    next_id: u32,
    /// Kitty image ids placed on the previous feeder frame.
    last_placed_ids: HashSet<u32>,
    /// `(scroll, selected)` from the previous feeder frame.
    /// Full refresh when either changes (selection changes card heights → media Y).
    last_layout_key: Option<(usize, usize)>,
}

impl MediaCache {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            ready: HashMap::new(),
            halfblocks: HashMap::new(),
            inflight: HashMap::new(),
            rx: Some(rx),
            tx: Some(tx),
            next_id: 9000,
            last_placed_ids: HashSet::new(),
            last_layout_key: None,
        }
    }

    pub fn get(&self, url: &str) -> Option<&MediaBytes> {
        self.ready.get(url)
    }

    pub fn get_mut(&mut self, url: &str) -> Option<&mut MediaBytes> {
        self.ready.get_mut(url)
    }

    pub fn halfblock(&self, url: &str) -> Option<&HalfblockPreview> {
        self.halfblocks.get(url)
    }

    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.rx else {
            return false;
        };
        let mut changed = false;
        loop {
            match rx.try_recv() {
                Ok((url, Some(mut bytes))) => {
                    self.inflight.remove(&url);
                    if bytes.image_id == 0 {
                        self.next_id = self.next_id.wrapping_add(1).max(9001);
                        bytes.image_id = self.next_id;
                    }
                    // Always decode halfblocks when Kitty overlay is unsafe, or
                    // when halfblock is not explicitly disabled — keeps buffer-
                    // local fallback ready without a second download.
                    if should_decode_halfblock() {
                        if let Some(hb) = decode_halfblocks(&bytes.prepared, 40, 4) {
                            self.halfblocks.insert(url.clone(), hb);
                        }
                    }
                    self.ready.insert(url, bytes);
                    changed = true;
                }
                Ok((url, None)) => {
                    self.inflight.remove(&url);
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.rx = None;
                    break;
                }
            }
        }
        changed
    }

    pub fn ensure(&mut self, url: &str) {
        if url.is_empty() || self.ready.contains_key(url) || self.inflight.contains_key(url) {
            return;
        }
        if url.contains("example.com") {
            return;
        }
        let Some(tx) = self.tx.clone() else {
            return;
        };
        let url_s = url.to_string();
        self.inflight.insert(url_s.clone(), ());
        std::thread::Builder::new()
            .name("feeder-img".into())
            .spawn(move || {
                let got = fetch_media(&url_s);
                let _ = tx.send((url_s, got));
            })
            .ok();
    }

    pub fn needs_tick(&self) -> bool {
        !self.inflight.is_empty()
    }

    /// Whether Kitty scrollback-style placement is safe for Feeder.
    pub fn kitty_inline_ok() -> bool {
        scrollback_inline_overlay_active()
    }

    /// Build Kitty place escapes for a media rect. Marks transmitted.
    /// Returns `(escapes, image_id)` so the caller can track this-frame ids.
    pub fn placement_escapes(&mut self, url: &str, area: Rect) -> Option<(String, u32)> {
        if !Self::kitty_inline_ok() {
            return None;
        }
        let entry = self.ready.get_mut(url)?;
        if area.width < 4 || area.height < 2 {
            return None;
        }
        let image_id = entry.image_id;
        let needs_tx = !entry.transmitted;
        let mut esc = String::new();
        if needs_tx {
            if let Some(t) = transmit_inline_image(&entry.prepared, entry.image_id) {
                esc.push_str(&t);
                entry.transmitted = true;
            } else {
                return None;
            }
        }
        // Re-borrow after possible mutation
        let entry = self.ready.get(url)?;
        let place = place_inline_image(
            &entry.prepared,
            entry.width,
            entry.height,
            area,
            area.height,
            0,
            entry.image_id,
            true, // iTerm may need data each place for feeder
        )?;
        esc.push_str(&place);
        Some((esc, image_id))
    }

    /// Call **before** placing this frame.
    ///
    /// On scroll/selection change: full-refresh — delete every feeder id from last
    /// frame and reset `transmitted` so subsequent places re-upload.
    ///
    /// When layout is unchanged, returns empty (stale ids cleared in [`end_frame`]).
    pub fn begin_frame(&mut self, scroll: usize, selected: usize) -> String {
        let key = (scroll, selected);
        let layout_changed = self.last_layout_key.map(|k| k != key).unwrap_or(false);
        if !layout_changed {
            return String::new();
        }
        // FULL refresh on scroll/selection: wipe every feeder placement so nothing
        // is left floating over the agent pane at the old Y.
        let to_clear: HashSet<u32> = self.last_placed_ids.iter().copied().collect();
        let clears = self.emit_clears(&to_clear);
        self.last_placed_ids.clear();
        // Keep last_layout_key until end_frame commits the new key.
        clears
    }

    /// Call **after** placing this frame.
    ///
    /// Clears ids that were on screen last frame but not this frame (scrolled off).
    /// Commits `this_frame` + layout key.
    pub fn end_frame(
        &mut self,
        this_frame: HashSet<u32>,
        scroll: usize,
        selected: usize,
    ) -> String {
        let mut to_clear: HashSet<u32> = HashSet::new();
        for &id in &self.last_placed_ids {
            if !this_frame.contains(&id) {
                to_clear.insert(id);
            }
        }
        let clears = self.emit_clears(&to_clear);
        self.last_placed_ids = this_frame;
        self.last_layout_key = Some((scroll, selected));
        clears
    }

    /// Delete every feeder placement (dock close / hide). Resets frame tracking.
    pub fn clear_all_placed(&mut self) -> String {
        let mut to_clear: HashSet<u32> = self.last_placed_ids.iter().copied().collect();
        // Belt and suspenders: any transmitted ready entry may still be on GPU.
        for entry in self.ready.values() {
            if entry.transmitted {
                to_clear.insert(entry.image_id);
            }
        }
        let clears = self.emit_clears(&to_clear);
        self.last_placed_ids.clear();
        self.last_layout_key = None;
        clears
    }

    fn emit_clears(&mut self, ids: &HashSet<u32>) -> String {
        if ids.is_empty() {
            return String::new();
        }
        let mut clears = String::new();
        for &id in ids {
            clears.push_str(&clear_kitty_image(id));
        }
        // d=i removed GPU data — force retransmit on next place.
        for entry in self.ready.values_mut() {
            if ids.contains(&entry.image_id) {
                entry.transmitted = false;
            }
        }
        clears
    }

    /// Snapshot of last-placed ids (tests).
    #[cfg(test)]
    pub fn last_placed_ids(&self) -> &HashSet<u32> {
        &self.last_placed_ids
    }
}

/// Take clear escapes for all feeder placements (dock close path).
pub fn take_clear_all_escapes() -> String {
    let cache = global_cache();
    let Ok(mut cache) = cache.lock() else {
        return String::new();
    };
    cache.clear_all_placed()
}

pub fn global_cache() -> Arc<Mutex<MediaCache>> {
    static CACHE: std::sync::OnceLock<Arc<Mutex<MediaCache>>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| Arc::new(Mutex::new(MediaCache::new())))
        .clone()
}

pub fn graphics_available() -> bool {
    detect_graphics_protocol() != GraphicsProtocol::None
}

/// Half-block fallback: on by default; `FEEDER_HALFBLOCK=0` forces off;
/// `FEEDER_HALFBLOCK=1` forces on even when Kitty is active.
pub fn halfblock_enabled() -> bool {
    match std::env::var_os("FEEDER_HALFBLOCK") {
        Some(v) if v == "0" => false,
        Some(v) if v == "1" => true,
        // Default: use halfblocks when Kitty inline is not safe.
        _ => !MediaCache::kitty_inline_ok(),
    }
}

fn should_decode_halfblock() -> bool {
    match std::env::var_os("FEEDER_HALFBLOCK") {
        Some(v) if v == "0" => false,
        Some(v) if v == "1" => true,
        // Decode whenever Kitty might not paint — keeps fallback ready.
        _ => true,
    }
}

fn fetch_media(url: &str) -> Option<MediaBytes> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(8))
        .timeout_connect(std::time::Duration::from_secs(2))
        .user_agent("FeederTUI/0.1")
        .build();
    let resp = agent.get(url).call().ok()?;
    let mut bytes = Vec::new();
    resp.into_reader()
        .take(4 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() < 32 {
        return None;
    }
    let prepared = prepare_overlay_image_bytes(&bytes)?;
    let (width, height) = crate::prompt_images::decode_image_dimensions(&prepared).or_else(|| {
        image::load_from_memory(&prepared)
            .ok()
            .map(|i| (i.width(), i.height()))
    })?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    let image_id = 9000 + (hasher.finish() as u32 % 50_000);
    // Touch fit helper so aspect path stays linked
    let _ = fit_image_to_cells(width, height, 40, 6);
    Some(MediaBytes {
        url: url.to_string(),
        prepared,
        width,
        height,
        image_id,
        transmitted: false,
    })
}

fn decode_halfblocks(png_bytes: &[u8], cols: u32, row_pairs: u32) -> Option<HalfblockPreview> {
    let img = image::load_from_memory(png_bytes).ok()?.to_rgb8();
    let (iw, ih) = (img.width(), img.height());
    if iw == 0 || ih == 0 {
        return None;
    }
    // Preserve aspect: fit inside cols x (row_pairs*2)
    let target_h_px = (row_pairs * 2).max(4);
    let target_w = cols.max(4);
    let scale = (target_w as f32 / iw as f32).min(target_h_px as f32 / ih as f32);
    let rw = ((iw as f32 * scale).round() as u32).max(1).min(target_w);
    let rh = ((ih as f32 * scale).round() as u32).max(1).min(target_h_px);
    // even height for halfblocks
    let rh = (rh / 2 * 2).max(2);
    let resized = image::imageops::resize(&img, rw, rh, image::imageops::FilterType::Lanczos3);
    let pairs = rh / 2;
    let mut rows = Vec::with_capacity(pairs as usize);
    for ry in 0..pairs {
        let y0 = ry * 2;
        let y1 = y0 + 1;
        let mut cells = Vec::with_capacity(rw as usize);
        for x in 0..rw {
            let top = resized.get_pixel(x, y0).0;
            let bot = resized.get_pixel(x, y1.min(rh - 1)).0;
            cells.push((
                Color::Rgb(top[0], top[1], top[2]),
                Color::Rgb(bot[0], bot[1], bot[2]),
            ));
        }
        rows.push(PreviewRow { cells });
    }
    Some(HalfblockPreview { rows })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_frame_clears_ids_not_in_this_frame() {
        let mut cache = MediaCache::new();
        cache.last_placed_ids = [9001, 9002, 9003].into_iter().collect();
        cache.last_layout_key = Some((0, 0));

        let this: HashSet<u32> = [9001, 9003].into_iter().collect();
        let _ = cache.begin_frame(0, 0); // no layout change
        let clears = cache.end_frame(this.clone(), 0, 0);

        assert!(clears.contains(&clear_kitty_image(9002)));
        assert!(!clears.contains(&clear_kitty_image(9001)));
        assert!(!clears.contains(&clear_kitty_image(9003)));
        assert_eq!(cache.last_placed_ids(), &this);
    }

    #[test]
    fn begin_frame_full_refresh_on_scroll_clears_all_previous() {
        let mut cache = MediaCache::new();
        cache.last_placed_ids = [9001, 9002].into_iter().collect();
        cache.last_layout_key = Some((0, 0));
        // Mark transmitted so we can assert reset after clear.
        cache.ready.insert(
            "u".into(),
            MediaBytes {
                url: "u".into(),
                prepared: vec![0; 64],
                width: 10,
                height: 10,
                image_id: 9001,
                transmitted: true,
            },
        );

        let clears = cache.begin_frame(3, 0);
        assert!(clears.contains(&clear_kitty_image(9001)));
        assert!(clears.contains(&clear_kitty_image(9002)));
        assert!(cache.last_placed_ids.is_empty());
        assert!(!cache.ready.get("u").unwrap().transmitted);

        // Place path after scroll would retransmit; end_frame commits new set.
        let this: HashSet<u32> = [9001, 9002].into_iter().collect();
        let stale = cache.end_frame(this.clone(), 3, 0);
        assert!(stale.is_empty(), "nothing stale after full clear");
        assert_eq!(cache.last_placed_ids(), &this);
        assert_eq!(cache.last_layout_key, Some((3, 0)));
    }

    #[test]
    fn begin_frame_full_refresh_on_selection_change() {
        let mut cache = MediaCache::new();
        cache.last_placed_ids = [9001].into_iter().collect();
        cache.last_layout_key = Some((0, 0));
        let clears = cache.begin_frame(0, 2);
        assert!(clears.contains(&clear_kitty_image(9001)));
    }

    #[test]
    fn clear_all_placed_empties_tracking() {
        let mut cache = MediaCache::new();
        cache.last_placed_ids = [9100].into_iter().collect();
        cache.last_layout_key = Some((2, 1));
        let clears = cache.clear_all_placed();
        assert!(clears.contains(&clear_kitty_image(9100)));
        assert!(cache.last_placed_ids.is_empty());
        assert!(cache.last_layout_key.is_none());
    }

    #[test]
    fn clears_do_not_emit_overlay_id_one() {
        let mut cache = MediaCache::new();
        cache.last_placed_ids = [9005].into_iter().collect();
        cache.last_layout_key = Some((0, 0));
        let clears = cache.end_frame(HashSet::new(), 0, 0);
        assert!(!clears.contains(&clear_kitty_image(1)));
        assert!(clears.contains(&clear_kitty_image(9005)));
    }
}
