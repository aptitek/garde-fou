//! Symmetric corpus tests for Dart 3.6+, Flutter 3.x, nested comments, configs, and `dart analyze`.

use gf_core::{ExternalToolKind, LangRuleFinding, RepoPath};
use gf_parsers::{parse_dart_analyze_json, parse_source};

#[test]
fn dart_and_flutter_bad_corpus_catches_all_rules() -> Result<(), String> {
    let p = RepoPath::new("lib/screens/home.dart").map_err(|e| e.to_string())?;
    let bad = r"
// ignore_for_file: avoid_print
import 'dart:html';
class HomeState extends State<Home> {
  final TextEditingController ctrl = TextEditingController();
  Future<void> submit(BuildContext context, dynamic payload) async {
    print(payload);
    try {
      await send(payload);
      Navigator.of(context).pop();
    } catch (e) {}
  }
  Widget build(BuildContext context) {
    setState(() {});
    return IconButton(color: Colors.red, onPressed: () {});
  }
}
";
    let doc = parse_source(&p, bad).map_err(|e| e.to_string())?;
    let codes: Vec<&str> = doc
        .lang_rule_findings
        .iter()
        .map(LangRuleFinding::code)
        .collect();
    for exp in [
        "DART-IGNORE-FOR-FILE",
        "DART-DEPRECATED-HTML",
        "DART-EXPLICIT-DYNAMIC",
        "DART-AVOID-PRINT",
        "DART-EMPTY-CATCH",
        "FLUTTER-BUILD-CONTEXT-ASYNC",
        "FLUTTER-MISSING-DISPOSE",
        "FLUTTER-SETSTATE-IN-BUILD",
        "FLUTTER-HARDCODED-COLOR",
        "FLUTTER-MISSING-A11Y",
    ] {
        assert!(codes.contains(&exp), "missing {exp} in {codes:?}");
    }
    Ok(())
}

#[test]
fn dart_nested_comments_generated_and_clean_flutter_pass() -> Result<(), String> {
    let p = RepoPath::new("lib/theme/tokens.dart").map_err(|e| e.to_string())?;
    let clean = r"
/* outer /* nested print('ignored'); dynamic x; */ still comment */
import 'package:web/web.dart';
class CleanState extends State<Clean> {
  final TextEditingController ctrl = TextEditingController();
  @override
  void dispose() {
    ctrl.dispose();
    super.dispose();
  }
  Future<void> submit(BuildContext context) async {
    await Future<void>.value();
    if (!context.mounted) return;
    Navigator.of(context).pop();
  }
  Widget build(BuildContext context) {
    return IconButton(color: Colors.blue, tooltip: 'Save', onPressed: () {});
  }
}
";
    let doc = parse_source(&p, clean).map_err(|e| e.to_string())?;
    assert_eq!(doc.lang_rule_findings.len(), 0);

    let gen_p = RepoPath::new("lib/model.g.dart").map_err(|e| e.to_string())?;
    let gen_doc = parse_source(&gen_p, "dynamic x = print(1);").map_err(|e| e.to_string())?;
    assert_eq!(gen_doc.lang_rule_findings.len(), 0);
    Ok(())
}

#[test]
fn dart_configs_and_analyze_json_work() -> Result<(), String> {
    let ao_p = RepoPath::new("analysis_options.yaml").map_err(|e| e.to_string())?;
    let ao = parse_source(
        &ao_p,
        "linter:\n  rules:\n    use_build_context_synchronously: false\n",
    )
    .map_err(|e| e.to_string())?;
    assert_eq!(ao.tool_config_findings.len(), 1);

    let pub_p = RepoPath::new("pubspec.yaml").map_err(|e| e.to_string())?;
    let pubdoc = parse_source(&pub_p, "name: app\ndependencies:\n  http: any\n")
        .map_err(|e| e.to_string())?;
    assert_eq!(pubdoc.tool_config_findings.len(), 1);

    let json = r#"{"diagnostics":[{"code":"use_build_context_synchronously","problemMessage":"Don't use BuildContext across async gaps","location":{"file":"lib/home.dart","range":{"start":{"line":14,"column":5}}}}]}"#;
    let findings = parse_dart_analyze_json(json).map_err(|e| e.to_string())?;
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool, ExternalToolKind::DartAnalyze);
    assert_eq!(findings[0].native_code, "use_build_context_synchronously");
    Ok(())
}
