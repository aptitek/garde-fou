//! Surgical project detachment (`garde uninstall --project` / `garde eject`)
//! and safe self-binary uninstallation (`garde uninstall --self`).

pub mod project_eject;
pub mod self_remove;
pub mod types;

pub use project_eject::{execute_project_eject, plan_project_eject};
pub use self_remove::{
    detect_mise_tool_slug, execute_self_uninstall, find_secondary_garde_binaries,
    plan_self_uninstall,
};
pub use types::{UninstallAction, UninstallExecReport, UninstallPlan};
