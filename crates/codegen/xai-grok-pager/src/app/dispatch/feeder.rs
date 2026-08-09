//! Feeder dock dispatchers: toggle a right-side feed beside the agent.

use crate::app::actions::Effect;
use crate::app::app_view::{ActiveView, AppView, TrustState};
use crate::views::feeder::{feeder_enabled, FeederState};

/// Toggle the Feeder **dock** (right column). Does not replace the agent view.
pub(super) fn dispatch_open_feeder(app: &mut AppView) -> Vec<Effect> {
    if !feeder_enabled() {
        app.show_toast("Feeder is disabled (GROK_FEEDER=0)");
        return vec![];
    }
    if app.screen_mode.is_minimal() {
        app.show_toast("Feeder needs fullscreen — try /fullscreen");
        return vec![];
    }
    if !matches!(app.auth_state, crate::app::app_view::AuthState::Done) {
        app.show_toast("Sign in to open Feeder");
        return vec![];
    }
    if matches!(app.trust_state, TrustState::Pending { .. }) {
        app.show_toast("Answer the folder-trust question to open Feeder");
        return vec![];
    }

    // Already open → close dock
    if app.feeder_dock_open {
        return dispatch_close_feeder(app);
    }

    // Dock only makes sense beside an agent session
    if !matches!(app.active_view, ActiveView::Agent(_)) {
        if matches!(app.active_view, ActiveView::Feeder) {
            app.active_view = preferred_agent_view(app);
        } else {
            app.show_toast("Open a coding session first, then /feeder");
            return vec![];
        }
    }

    // Seed work context from the active agent's prompt history
    let history: Vec<String> = match app.active_view {
        ActiveView::Agent(id) => app
            .agents
            .get(&id)
            .map(|a| {
                a.combined_prompt_history()
                    .into_iter()
                    .map(|e| e.text)
                    .take(12)
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    if app.feeder.is_none() {
        app.feeder = Some(FeederState::new());
    }
    if let Some(f) = app.feeder.as_mut() {
        if !history.is_empty() {
            f.seed_work_history(&history);
        }
        f.dock_focused = true;
        f.start_refresh(None);
    }

    app.feeder_dock_open = true;
    app.feeder_focused = true;
    app.show_toast("Feeder focused · j/k · tracks your prompts · u use · q close");
    vec![]
}

fn preferred_agent_view(app: &AppView) -> ActiveView {
    if let Some(first) = app.agents.keys().next().copied() {
        ActiveView::Agent(first)
    } else {
        ActiveView::Welcome
    }
}

/// Close the Feeder dock; agent stays put.
pub(super) fn dispatch_close_feeder(app: &mut AppView) -> Vec<Effect> {
    app.feeder_dock_open = false;
    app.feeder_focused = false;
    if matches!(app.active_view, ActiveView::Feeder) {
        app.active_view = preferred_agent_view(app);
    }
    app.feeder_return = None;
    vec![]
}

/// Called when the user sends a prompt — update Feeder work index + refresh if open.
pub(super) fn feeder_on_user_prompt(app: &mut AppView, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    // Always keep work index warm if feeder state exists or dock is open
    if app.feeder.is_none() && !app.feeder_dock_open {
        // Lazy-create so context accumulates even before first /feeder
        app.feeder = Some(FeederState::new());
    }
    if let Some(f) = app.feeder.as_mut() {
        f.note_user_prompt(text);
    }
}
