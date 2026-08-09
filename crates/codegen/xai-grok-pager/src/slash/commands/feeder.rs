//! `/feeder` — open the Feeder feed view (mock shell for W6).

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
use crate::slash::{ModeSupport, Remedy};

/// Open the Feeder full-screen feed.
pub struct FeederCommand;

impl SlashCommand for FeederCommand {
    fn name(&self) -> &str {
        "feeder"
    }

    fn description(&self) -> &str {
        "Toggle Feeder dock — side feed while you code (Tab focus, u use)"
    }

    fn usage(&self) -> &str {
        "/feeder"
    }

    /// Feeder is a fullscreen multi-pane surface; refuse in minimal mode
    /// (same rationale as `/dashboard`).
    fn mode_support(&self) -> ModeSupport {
        ModeSupport::FullscreenOnly(Remedy::SwitchMode {
            why: "minimal is single-session",
        })
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::OpenFeeder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::slash::command::{CommandExecCtx, CommandResult};

    #[test]
    fn run_returns_open_feeder_action() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = CommandExecCtx {
            models: &models,
            session_id: None,
            bundle_state: &bundle,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot {
                multiline_mode: false,
                yolo_mode: false,
                ..crate::settings::PagerLocalSnapshot::default()
            },
        };
        let cmd = FeederCommand;
        assert!(matches!(
            cmd.run(&mut ctx, ""),
            CommandResult::Action(Action::OpenFeeder)
        ));
    }

    #[test]
    fn name_is_feeder() {
        assert_eq!(FeederCommand.name(), "feeder");
    }
}
