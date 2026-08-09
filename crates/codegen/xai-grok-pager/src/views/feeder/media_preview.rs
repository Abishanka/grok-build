//! Media download + optional Kitty/iTerm placement for Feeder cards.
//!
//! Primary path: download bytes for Kitty/iTerm inline graphics.
//! Fallback: caller paints a clean text media card (no muddy half-blocks).
//! Half-blocks only if `FEEDER_HALFBLOCK=1`.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::terminal::image::{
    detect_graphics_protocol, fit_image_to_cells, place_inline_image, prepare_overlay_image_bytes,
    transmit_inline_image, GraphicsProtocol,
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

/// Optional half-block rows (env-gated only).
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
                    // Optional halfblock decode for FEEDER_HALFBLOCK=1
                    if halfblock_enabled() {
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

    /// Build Kitty/iTerm place escapes for a media rect. Marks transmitted.
    pub fn placement_escapes(&mut self, url: &str, area: Rect) -> Option<String> {
        if detect_graphics_protocol() == GraphicsProtocol::None {
            return None;
        }
        let entry = self.ready.get_mut(url)?;
        if area.width < 4 || area.height < 2 {
            return None;
        }
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
        Some(esc)
    }
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

pub fn halfblock_enabled() -> bool {
    std::env::var_os("FEEDER_HALFBLOCK").is_some_and(|v| v == "1")
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
    let (width, height) = crate::prompt_images::decode_image_dimensions(&prepared)
        .or_else(|| {
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
