//! Pure Lipgloss terminal styling, Unicode box geometry, and Miette-style diagnostic cards.

pub mod ansi;
pub mod box_layout;
pub mod cache_uninstall_report;
pub mod code_frame;
pub mod human_report;
pub mod human_summary;
pub mod ops_report;
pub mod policy;
pub mod theme;

pub use ansi::{char_display_width, strip_ansi, truncate_visible, visible_width};
pub use box_layout::{
    CardStyle, TreeSymbol, gauge_bar, join_horizontal_top, kv_row, rounded_card, status_tree,
};
pub use cache_uninstall_report::{
    format_bytes_human, render_cache_clean_human, render_cache_status_human,
    render_uninstall_exec_human, render_uninstall_plan_human,
};
pub use code_frame::{CodeFrameSpec, SourceFileView, render_code_frame};
pub use human_report::{
    HumanCheckMeta, build_diagnostic_block, render_human_check, render_single_diagnostic_card,
};
pub use human_summary::{render_context_capsules, render_violation_footer};
pub use ops_report::{
    render_commit_msg_human, render_init_human, render_lock_human, render_sync_human, spinner_frame,
};
pub use policy::{
    ColorPolicy, TermCapabilities, encode_base64, osc52_clipboard_sequence, resolve_color_policy,
};
pub use theme::{PillRole, Role, paint, paint_bold, pill};
