//! `/feeder` — toggle Feeder dock; `/feeder jam …` multiplayer.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
use crate::slash::{ModeSupport, Remedy};

/// Open the Feeder dock, or run jam subcommands.
pub struct FeederCommand;

impl SlashCommand for FeederCommand {
    fn name(&self) -> &str {
        "feeder"
    }

    fn description(&self) -> &str {
        "Feeder dock · /feeder jam start|join|leave|end|invite"
    }

    fn usage(&self) -> &str {
        "/feeder [jam start|join <url>|leave|end|invite|status]"
    }

    fn mode_support(&self) -> ModeSupport {
        ModeSupport::FullscreenOnly(Remedy::SwitchMode {
            why: "minimal is single-session",
        })
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let args = args.trim();
        if args.is_empty() {
            return CommandResult::Action(Action::OpenFeeder);
        }
        // jam …
        let lower = args.to_ascii_lowercase();
        if lower == "jam" || lower.starts_with("jam ") {
            let rest = args
                .strip_prefix("jam")
                .or_else(|| args.strip_prefix("JAM"))
                .unwrap_or(args)
                .trim()
                .to_string();
            return CommandResult::Action(Action::FeederJamCommand { args: rest });
        }
        // bare subcommands without "jam" prefix for speed
        let first = args.split_whitespace().next().unwrap_or("");
        if matches!(
            first,
            "start" | "join" | "leave" | "end" | "invite" | "status"
        ) {
            return CommandResult::Action(Action::FeederJamCommand {
                args: args.to_string(),
            });
        }
        CommandResult::Action(Action::OpenFeeder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::slash::command::{CommandExecCtx, CommandResult};

    fn ctx<'a>(
        models: &'a ModelState,
        bundle: &'a BundleState,
    ) -> CommandExecCtx<'a> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: bundle,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot {
                multiline_mode: false,
                yolo_mode: false,
                ..crate::settings::PagerLocalSnapshot::default()
            },
        }
    }

    #[test]
    fn run_returns_open_feeder_action() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut c = ctx(&models, &bundle);
        assert!(matches!(
            FeederCommand.run(&mut c, ""),
            CommandResult::Action(Action::OpenFeeder)
        ));
    }

    #[test]
    fn jam_start_returns_jam_action() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut c = ctx(&models, &bundle);
        match FeederCommand.run(&mut c, "jam start demo") {
            CommandResult::Action(Action::FeederJamCommand { args }) => {
                assert!(args.contains("start"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn name_is_feeder() {
        assert_eq!(FeederCommand.name(), "feeder");
    }
}
