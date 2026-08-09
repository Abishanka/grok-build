//! Feeder dock dispatchers: toggle a right-side feed beside the agent.

use crate::app::actions::Effect;
use crate::app::app_view::{ActiveView, AppView, TrustState};
use crate::views::feeder::{FeederState, feeder_enabled};

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
        // If stuck on legacy full-screen Feeder enum, leave it
        if matches!(app.active_view, ActiveView::Feeder) {
            app.active_view = preferred_agent_view(app);
        } else {
            app.show_toast("Open a coding session first, then /feeder");
            return vec![];
        }
    }

    if app.feeder.is_none() {
        app.feeder = Some(FeederState::new());
    } else if let Some(f) = app.feeder.as_mut() {
        // Soft refresh when reopening
        f.refresh_from_service(None);
    }

    app.feeder_dock_open = true;
    app.feeder_focused = true;
    app.show_toast("Feeder dock · Tab agent↔feed · u use · e explain · x dismiss · q close");
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
    // Leave legacy full-screen mode if somehow active
    if matches!(app.active_view, ActiveView::Feeder) {
        app.active_view = preferred_agent_view(app);
    }
    app.feeder_return = None;
    vec![]
}
