//! Flutter 3.x async `BuildContext`, `dispose()` lifecycle, Material 3 theme, and a11y analyzer (`FLUTTER-*`).

use gf_core::{FlutterViolation, LangRuleFinding, RepoPath, Span, UastDocument};

const DISPOSABLE_TYPES: &[&str] = &[
    "TextEditingController",
    "AnimationController",
    "ScrollController",
    "FocusNode",
    "StreamSubscription",
];

const CONTEXT_USES: &[&str] = &[
    "Navigator.of(context)",
    "ScaffoldMessenger.of(context)",
    "Theme.of(context)",
    "showDialog(",
];

fn push_fl(doc: &mut UastDocument, v: FlutterViolation, ln: u32, msg: &str) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    doc.lang_rule_findings
        .push(LangRuleFinding::new(v, sp, msg));
}

fn is_fn_boundary(trimmed: &str) -> bool {
    trimmed.contains("async {") || trimmed.starts_with("void ") || trimmed.starts_with("Future")
}

fn check_async_build_context(lines: &[&str], doc: &mut UastDocument) {
    let (mut saw_await, mut guarded) = (false, false);
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if is_fn_boundary(trimmed) {
            (saw_await, guarded) = (false, false);
        } else if trimmed.contains("await ") {
            (saw_await, guarded) = (true, false);
        }
        guarded |= trimmed.contains("mounted");
        if saw_await && !guarded && CONTEXT_USES.iter().any(|u| trimmed.contains(u)) {
            let ln = u32::try_from(idx + 1).unwrap_or(1);
            push_fl(
                doc,
                FlutterViolation::BuildContextAcrossAsyncGap,
                ln,
                "`BuildContext` used after `await` without `if (!context.mounted) return;` guard",
            );
        }
    }
}

fn check_missing_dispose(source: &str, lines: &[&str], doc: &mut UastDocument) {
    if !source.contains("extends State<") {
        return;
    }
    let has_dispose_call = source.contains(".dispose()") || source.contains(".cancel()");
    if has_dispose_call {
        return;
    }
    for (idx, line) in lines.iter().enumerate() {
        if DISPOSABLE_TYPES.iter().any(|t| line.contains(t)) {
            let ln = u32::try_from(idx + 1).unwrap_or(1);
            push_fl(
                doc,
                FlutterViolation::MissingDispose,
                ln,
                "Flutter `State` allocates disposable controller/subscription without `.dispose()`/`.cancel()`",
            );
            return;
        }
    }
}

fn check_setstate_in_build(lines: &[&str], doc: &mut UastDocument) {
    let mut in_build = false;
    let mut depth = 0i32;
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.contains("Widget build(") {
            in_build = true;
            depth = 0;
        }
        if !in_build {
            continue;
        }
        let opens = i32::try_from(trimmed.bytes().filter(|&b| b == b'{').count()).unwrap_or(0);
        let closes = i32::try_from(trimmed.bytes().filter(|&b| b == b'}').count()).unwrap_or(0);
        depth += opens - closes;
        if trimmed.starts_with("setState(") {
            let ln = u32::try_from(idx + 1).unwrap_or(1);
            push_fl(
                doc,
                FlutterViolation::SetStateInBuild,
                ln,
                "calling `setState()` directly inside `Widget build()` triggers infinite rebuilds",
            );
        }
        if depth <= 0 && closes > 0 {
            in_build = false;
        }
    }
}

fn check_icon_button_tooltip(lines: &[&str], idx: usize, ln: u32, doc: &mut UastDocument) {
    let end = (idx + 6).min(lines.len());
    if !lines[idx..end].iter().any(|l| l.contains("tooltip:")) {
        push_fl(
            doc,
            FlutterViolation::MissingIconButtonTooltip,
            ln,
            "`IconButton` is missing a localized `tooltip:` for accessibility",
        );
    }
}

fn check_theme_and_a11y(path: &RepoPath, lines: &[&str], doc: &mut UastDocument) {
    let in_theme = path.as_str().contains("/theme/") || path.as_str().starts_with("lib/theme/");
    for (idx, line) in lines.iter().enumerate() {
        let ln = u32::try_from(idx + 1).unwrap_or(1);
        let trimmed = line.trim();
        if !in_theme && (trimmed.contains("Color(0x") || trimmed.contains("Colors.")) {
            push_fl(
                doc,
                FlutterViolation::HardcodedColorOutsideTheme,
                ln,
                "hardcoded `Color(0x...)`/`Colors.*` outside `lib/theme/`; use `Theme.of(context).colorScheme`",
            );
        }
        if trimmed.contains("IconButton(") {
            check_icon_button_tooltip(lines, idx, ln, doc);
        }
    }
}

/// Runs Flutter 3.x static checks on Dart source.
pub fn analyze_flutter(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    let lines: Vec<&str> = source.lines().collect();
    check_async_build_context(&lines, doc);
    check_missing_dispose(source, &lines, doc);
    check_setstate_in_build(&lines, doc);
    check_theme_and_a11y(path, &lines, doc);
}
