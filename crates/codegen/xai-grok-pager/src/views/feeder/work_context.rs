//! Rolling work index for Feeder — recent user prompts drive search.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Max prompts kept in memory / file.
const RING: usize = 12;
/// Max prompts sent to the API per query.
const QUERY_PROMPTS: usize = 8;
/// Truncate each prompt for the API.
const PROMPT_CHARS: usize = 200;

/// Session work context that personalizes the Feeder slate.
#[derive(Debug, Clone, Default)]
pub struct WorkContext {
    /// Newest first.
    prompts: Vec<String>,
    path: Option<PathBuf>,
}

impl WorkContext {
    pub fn new() -> Self {
        let mut wc = Self {
            prompts: Vec::new(),
            path: default_path(),
        };
        wc.load_from_disk();
        wc
    }

    /// Bind persistence path (e.g. after cwd known). Reloads if path changes.
    pub fn set_path(&mut self, path: PathBuf) {
        if self.path.as_ref() != Some(&path) {
            self.path = Some(path);
            self.load_from_disk();
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Push a new user prompt (newest). No-op for empty / slash-only.
    pub fn push(&mut self, text: &str) {
        let cleaned = clean_prompt(text);
        if cleaned.is_empty() {
            return;
        }
        // Dedupe consecutive identical
        if self.prompts.first().is_some_and(|p| p == &cleaned) {
            return;
        }
        self.prompts.retain(|p| p != &cleaned);
        self.prompts.insert(0, cleaned);
        if self.prompts.len() > RING {
            self.prompts.truncate(RING);
        }
        self.persist();
    }

    /// Seed from agent history (newest first), without clobbering newer local entries.
    pub fn seed_from_history(&mut self, history: &[String]) {
        for h in history.iter().rev() {
            let cleaned = clean_prompt(h);
            if cleaned.is_empty() {
                continue;
            }
            if !self.prompts.iter().any(|p| p == &cleaned) {
                self.prompts.push(cleaned);
            }
        }
        // Re-sort: keep existing order preference — rebuild newest-first from
        // history order (history is already newest first).
        let mut seen = std::collections::HashSet::new();
        let mut ordered = Vec::new();
        for h in history {
            let c = clean_prompt(h);
            if !c.is_empty() && seen.insert(c.clone()) {
                ordered.push(c);
            }
        }
        for p in &self.prompts {
            if seen.insert(p.clone()) {
                ordered.push(p.clone());
            }
        }
        ordered.truncate(RING);
        self.prompts = ordered;
        self.persist();
    }

    /// Prompts for `/v1/feed/query` context (newest first).
    pub fn prompts_for_query(&self) -> Vec<String> {
        if self.prompts.is_empty() {
            return default_bootstrap_prompts();
        }
        self.prompts
            .iter()
            .take(QUERY_PROMPTS)
            .cloned()
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.prompts.is_empty()
    }

    fn persist(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".into());
        let mut body = String::new();
        body.push_str("# Feeder work context\n");
        body.push_str(&format!("updated: {ts}\n"));
        body.push_str(&format!("cwd: {cwd}\n\n"));
        body.push_str("## Recent prompts (newest first)\n");
        for (i, p) in self.prompts.iter().enumerate() {
            let one_line = p.replace('\n', " ");
            body.push_str(&format!("{}. {}\n", i + 1, one_line));
        }
        if let Ok(mut f) = fs::File::create(path) {
            let _ = f.write_all(body.as_bytes());
        }
    }

    fn load_from_disk(&mut self) {
        let Some(path) = &self.path else {
            return;
        };
        let Ok(text) = fs::read_to_string(path) else {
            return;
        };
        let mut loaded = Vec::new();
        let mut in_section = false;
        for line in text.lines() {
            if line.starts_with("## Recent prompts") {
                in_section = true;
                continue;
            }
            if !in_section {
                continue;
            }
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            // "1. prompt text"
            let content = t
                .split_once('.')
                .map(|(_, rest)| rest.trim())
                .unwrap_or(t);
            let c = clean_prompt(content);
            if !c.is_empty() {
                loaded.push(c);
            }
        }
        if !loaded.is_empty() {
            self.prompts = loaded;
            if self.prompts.len() > RING {
                self.prompts.truncate(RING);
            }
        }
    }
}

fn default_path() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    Some(cwd.join(".grok").join("feeder-session.md"))
}

fn clean_prompt(text: &str) -> String {
    let t = text.trim();
    if t.is_empty() {
        return String::new();
    }
    // Skip pure slash commands
    if t.starts_with('/') && !t.contains(' ') {
        return String::new();
    }
    // Skip feeder untrusted blocks injected as prompts
    if t.contains("<untrusted-feed-item") {
        return String::new();
    }
    let mut s: String = t.chars().take(PROMPT_CHARS).collect();
    // Light secret scrub
    if s.to_ascii_lowercase().contains("bearer ")
        || s.contains("sk-")
        || s.contains("api_key")
    {
        s = "[redacted prompt]".into();
    }
    s
}

fn default_bootstrap_prompts() -> Vec<String> {
    vec![
        "coding agent tooling".into(),
        "developer productivity".into(),
    ]
}
