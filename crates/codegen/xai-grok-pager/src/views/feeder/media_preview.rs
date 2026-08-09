//! Half-block image previews for Feeder cards (works without Kitty).

use std::collections::HashMap;
use std::io::Read;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

use ratatui::style::Color;

/// One row of half-block cells: glyph is always `▀` with (fg=top, bg=bottom).
#[derive(Debug, Clone)]
pub struct PreviewRow {
    pub cells: Vec<(Color, Color)>,
}

/// Cached half-block preview for a media URL.
#[derive(Debug, Clone)]
pub struct MediaPreview {
    pub rows: Vec<PreviewRow>,
    pub url: String,
}

/// Shared cache + in-flight downloads keyed by media URL.
#[derive(Debug, Default)]
pub struct MediaCache {
    ready: HashMap<String, MediaPreview>,
    /// URLs currently downloading (avoid duplicate fetches).
    inflight: HashMap<String, ()>,
    rx: Option<Receiver<(String, Option<MediaPreview>)>>,
    tx: Option<mpsc::Sender<(String, Option<MediaPreview>)>>,
}

impl MediaCache {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            ready: HashMap::new(),
            inflight: HashMap::new(),
            rx: Some(rx),
            tx: Some(tx),
        }
    }

    pub fn get(&self, url: &str) -> Option<&MediaPreview> {
        self.ready.get(url)
    }

    /// Poll finished downloads. Returns true if anything new landed.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.rx else {
            return false;
        };
        let mut changed = false;
        loop {
            match rx.try_recv() {
                Ok((url, Some(preview))) => {
                    self.inflight.remove(&url);
                    self.ready.insert(url, preview);
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

    /// Kick background fetch+decode for `url` if not already cached/in-flight.
    pub fn ensure(&mut self, url: &str, cols: u16, rows: u16) {
        if url.is_empty() || self.ready.contains_key(url) || self.inflight.contains_key(url) {
            return;
        }
        let Some(tx) = self.tx.clone() else {
            return;
        };
        let url_s = url.to_string();
        self.inflight.insert(url_s.clone(), ());
        let cols = cols.max(8) as u32;
        let rows = rows.max(3) as u32;
        std::thread::Builder::new()
            .name("feeder-img".into())
            .spawn(move || {
                let preview = fetch_halfblocks(&url_s, cols, rows);
                let _ = tx.send((url_s, preview));
            })
            .ok();
    }

    pub fn needs_tick(&self) -> bool {
        !self.inflight.is_empty()
    }
}

/// Global process-wide cache so reopening the dock reuses downloads.
pub fn global_cache() -> Arc<Mutex<MediaCache>> {
    static CACHE: std::sync::OnceLock<Arc<Mutex<MediaCache>>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| Arc::new(Mutex::new(MediaCache::new())))
        .clone()
}

fn fetch_halfblocks(url: &str, cols: u32, row_pairs: u32) -> Option<MediaPreview> {
    // Skip obviously broken / placeholder hosts
    if url.contains("example.com") {
        return None;
    }
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(6))
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
    let img = image::load_from_memory(&bytes).ok()?.to_rgb8();
    let target_w = cols.max(4);
    let target_h = (row_pairs * 2).max(4); // two pixels per terminal row
    let resized = image::imageops::resize(
        &img,
        target_w,
        target_h,
        image::imageops::FilterType::Triangle,
    );
    let mut rows = Vec::with_capacity(row_pairs as usize);
    for ry in 0..row_pairs {
        let y0 = ry * 2;
        let y1 = y0 + 1;
        let mut cells = Vec::with_capacity(target_w as usize);
        for x in 0..target_w {
            let top = resized.get_pixel(x, y0.min(target_h - 1)).0;
            let bot = resized.get_pixel(x, y1.min(target_h - 1)).0;
            cells.push((
                Color::Rgb(top[0], top[1], top[2]),
                Color::Rgb(bot[0], bot[1], bot[2]),
            ));
        }
        rows.push(PreviewRow { cells });
    }
    Some(MediaPreview {
        rows,
        url: url.to_string(),
    })
}
