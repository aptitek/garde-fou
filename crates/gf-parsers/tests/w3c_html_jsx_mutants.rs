//! Targeted mutation-killer tests for `crates/gf-parsers/src/langs/html_w3c/*`.

use gf_core::{RepoPath, UastDocument, UiCheckKind};
use gf_parsers::parse_source;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn parse_rel(rel: &str, src: &str) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(parse_source(&RepoPath::new(rel)?, src)?)
}

fn line_codes(doc: &UastDocument) -> Vec<(&'static str, u32)> {
    doc.lang_rule_findings
        .iter()
        .map(|f| (f.code(), f.span.start_line()))
        .collect()
}

#[test]
fn jsx_bridge_valid_and_ui_checks_kill_mod_mutants() -> TestResult {
    let tsx_src = concat!(
        "export function View({ dynId, open }: { dynId: string; open: boolean }) {\n",
        "  const helper = () => <button type=\"button\" />;\n",
        "  return (\n",
        "    <section aria-hidden={false} aria-level={2} aria-live={\"polite\"}>\n",
        "      <CustomWidget colon:tag=\"1\" />\n",
        "      <input id={dynId} />\n",
        "      <div role=\"checkbox\" aria-checked={open}>\n",
        "        <span>text child</span>\n",
        "      </div>\n",
        "      <ul>{[\n",
        "        <li key=\"1\">item</li>\n",
        "      ]}</ul>\n",
        "      {helper()}\n",
        "    </section>\n",
        "  );\n",
        "}\n",
    );
    let tsx_doc = parse_rel("src/View.tsx", tsx_src)?;
    assert_eq!(line_codes(&tsx_doc), vec![("A11Y-MEDIA-FORM-MOTION", 2)]);

    let html_ui = concat!(
        "<html lang=\"en\"><body>\n",
        "  <img src=\"a.png\">\n",
        "  <img src=\"b.png\" alt=\"ok\">\n",
        "  <button>bad</button>\n",
        "  <button type=\"button\">ok</button>\n",
        "</body></html>\n",
    );
    let html_doc = parse_rel("src/ui.html", html_ui)?;
    let ui_pairs: Vec<(UiCheckKind, u32)> = html_doc
        .ui_checks
        .iter()
        .map(|u| (u.kind, u.span.start_line()))
        .collect();
    assert_eq!(
        ui_pairs,
        vec![
            (UiCheckKind::ImgMissingAlt, 2),
            (UiCheckKind::ButtonMissingType, 4),
        ]
    );
    Ok(())
}

#[test]
fn jsx_bridge_invalid_elements_kill_mod_mutants() -> TestResult {
    let bad_tsx = concat!(
        "export function Bad({ xs }: { xs: string[] }) {\n",
        "  return (\n",
        "    <div aria-hidden={true} aria-level={\"bad\"} aria-live={123} tabIndex={2}>\n",
        "      <button type=\"button\">trap</button>\n",
        "      <button type=\"button\"></button>\n",
        "      <button type=\"button\">   </button>\n",
        "      <ul>{[\n",
        "        <div key=\"1\">bad ul expr child</div>\n",
        "      ]}</ul>\n",
        "      <ul>{xs.map((x) => {\n",
        "        const innerBad = () => <a href=\"/x\" />;\n",
        "        return <li key={x}>{innerBad()}</li>;\n",
        "      })}</ul>\n",
        "    </div>\n",
        "  );\n",
        "}\n",
    );
    let bad_doc = parse_rel("src/Bad.tsx", bad_tsx)?;
    let bad_lc = line_codes(&bad_doc);
    for expected in [
        ("A11Y-ARIA-INVALID", 3),
        ("A11Y-TABINDEX-AND-FOCUS-RING", 3),
        ("A11Y-HIDDEN-FOCUSABLE", 4),
        ("A11Y-MEDIA-FORM-MOTION", 5),
        ("A11Y-MEDIA-FORM-MOTION", 6),
        ("W3C-CONTENT-MODEL", 8),
        ("A11Y-MEDIA-FORM-MOTION", 11),
    ] {
        assert!(
            bad_lc.contains(&expected),
            "missing {expected:?} in {bad_lc:?}"
        );
    }
    assert!(
        !bad_lc.contains(&("A11Y-MEDIA-FORM-MOTION", 4)),
        "{bad_lc:?}"
    );
    Ok(())
}

#[test]
fn aria_table_roles_required_states_and_scalar_kinds_kill_mutants() -> TestResult {
    let aria_html = concat!(
        "<html lang=\"en\"><body>\n",
        "  <div role=\"checkbox\" aria-checked=\"mixed\" aria-expanded=\"undefined\" aria-relevant=\"additions text\" aria-level=\"3\" aria-valuenow=\"1.5\" aria-label=\"ok\">ok</div>\n",
        "  <div role=\"bogus\">x</div>\n",
        "  <nav role=\"navigation\" aria-label=\"main\">x</nav>\n",
        "  <div role=\"checkbox\">missing checked</div>\n",
        "  <div aria-unknown=\"1\">unknown</div>\n",
        "  <div aria-hidden=\"maybe\">bad bool</div>\n",
        "  <div aria-checked=\"nope\">bad tristate</div>\n",
        "  <div aria-expanded=\"nope\">bad optbool</div>\n",
        "  <div aria-live=\"loud\">bad token</div>\n",
        "  <div aria-relevant=\"additions bogus\">bad tokenlist</div>\n",
        "  <div aria-level=\"two\">bad int</div>\n",
        "  <div aria-valuenow=\"not-a-number\">bad dec</div>\n",
        "  <div aria-hidden=\"\">empty</div>\n",
        "</body></html>\n",
    );
    let doc = parse_rel("src/aria.html", aria_html)?;
    let lc = line_codes(&doc);
    assert!(!lc.iter().any(|&(_, ln)| ln == 2), "{lc:?}");
    for ln in 3..=14 {
        assert!(
            lc.contains(&("A11Y-ARIA-INVALID", ln)),
            "missing on line {ln}: {lc:?}"
        );
    }
    Ok(())
}

#[test]
fn a11y_controls_focus_hover_media_and_forms_kill_mutants() -> TestResult {
    let controls_html = concat!(
        "<html lang=\"en\"><body>\n",
        "  <div aria-hidden=\"true\">\n",
        "    <input type=\"hidden\">\n",
        "    <a>anchor without href</a>\n",
        "    <button type=\"button\" tabindex=\"-1\">neg</button>\n",
        "    <span tabindex=\"-1\">neg span</span>\n",
        "    <a href=\"/trap\">link trap</a>\n",
        "    <input type=\"text\" aria-label=\"input trap\">\n",
        "    <span tabindex=\"0\">zero span trap</span>\n",
        "  </div>\n",
        "  <div onmouseover=\"a()\" onfocus=\"b()\" onmouseout=\"c()\" onblur=\"d()\" tabindex=\"0\">paired</div>\n",
        "  <div onmouseover=\"a()\">unpaired over</div>\n",
        "  <div onmouseout=\"a()\">unpaired out</div>\n",
        "  <video muted src=\"a.mp4\"></video>\n",
        "  <video aria-hidden=\"true\" src=\"a.mp4\"></video>\n",
        "  <video src=\"a.mp4\"><track kind=\"captions\"></video>\n",
        "  <audio muted src=\"a.mp3\"></audio>\n",
        "  <video src=\"a.mp4\"><track kind=\"chapters\"></video>\n",
        "  <video src=\"a.mp4\"><source kind=\"captions\"></video>\n",
        "  <select></select>\n",
        "  <textarea></textarea>\n",
        "  <input id=\"unmatched-static\">\n",
        "</body></html>\n",
    );
    let doc = parse_rel("src/controls.html", controls_html)?;
    let lc = line_codes(&doc);
    for clean_ln in [3, 4, 5, 6, 11, 14, 15, 16] {
        assert!(
            !lc.iter().any(|&(_, ln)| ln == clean_ln),
            "unexpected on line {clean_ln}: {lc:?}"
        );
    }
    for trap_ln in [7, 8, 9] {
        assert!(
            lc.contains(&("A11Y-HIDDEN-FOCUSABLE", trap_ln)),
            "missing trap on {trap_ln}: {lc:?}"
        );
    }
    assert!(lc.contains(&("A11Y-TABINDEX-AND-FOCUS-RING", 12)), "{lc:?}");
    assert!(lc.contains(&("A11Y-TABINDEX-AND-FOCUS-RING", 13)), "{lc:?}");
    for media_form_ln in [17, 18, 19, 20, 21, 22] {
        assert!(
            lc.contains(&("A11Y-MEDIA-FORM-MOTION", media_form_ln)),
            "missing on {media_form_ln}: {lc:?}"
        );
    }
    Ok(())
}

#[test]
fn w3c_content_models_clean_landmarks_kill_mutants() -> TestResult {
    let single_landmarks = concat!(
        "<!DOCTYPE html>\n<!-- comment -->\n<html lang=\"en\"><body>\n",
        "  <nav aria-label=\"primary\">nav 1</nav>\n  <nav aria-label=\"secondary\">nav 2</nav>\n",
        "  <aside aria-label=\"a1\">aside 1</aside>\n  <aside aria-label=\"a2\">aside 2</aside>\n",
        "  <main>visible main</main>\n  <main hidden>hidden main 1</main>\n  <main hidden>hidden main 2</main>\n",
        "  <button type=\"button\"><input type=\"hidden\"></button>\n",
        "  <table><tr><td>1</td><th>2</th></tr></table>\n  <dl><dt>k</dt><dd>v</dd></dl>\n",
        "  <CustomButton role=\"bogus\" />\n  <svg:a role=\"bogus\" />\n</body></html>\n",
    );
    assert_eq!(
        parse_rel("src/clean_w3c.html", single_landmarks)?.lang_rule_findings,
        Vec::new()
    );
    let one_nav_one_aside =
        "<html lang=\"en\"><body><nav>only nav</nav><aside>only aside</aside></body></html>\n";
    assert_eq!(
        parse_rel("src/one_landmark.html", one_nav_one_aside)?.lang_rule_findings,
        Vec::new()
    );
    Ok(())
}

#[test]
fn w3c_content_models_dirty_landmarks_and_scanner_edges_kill_mutants() -> TestResult {
    let dirty_w3c = concat!(
        "<html lang=\"en\"><body>\n",
        "  <aside>aside 1</aside>\n",
        "  <aside>aside 2</aside>\n",
        "  <button type=\"button\">text<input type=\"text\" aria-label=\"nested\"></button>\n",
        "  <table><tr><div>bad tr child</div></tr></table>\n",
        "  <dl><span>bad dl child</span></dl>\n",
        "  <main>main 1</main>\n",
        "  <main>main 2</main>\n",
        "  <script>const raw = \"<marquee>ignored</marquee>\";</script>\n",
        "  <script src=\"/app.js\" />\n",
        "  <div data={fn(\"a}b>\")} nested={{ k: 1 }} aria-hidden={true} tabindex={2}>badbrace</div>\n",
        "</body></html>\n",
    );
    let dirty_doc = parse_rel("src/dirty_w3c.html", dirty_w3c)?;
    assert_eq!(
        line_codes(&dirty_doc),
        vec![
            ("W3C-LANDMARK-AND-ID-UNIQUENESS", 2),
            ("W3C-LANDMARK-AND-ID-UNIQUENESS", 3),
            ("W3C-INTERACTIVE-NESTING", 4),
            ("W3C-CONTENT-MODEL", 5),
            ("W3C-CONTENT-MODEL", 6),
            ("W3C-LANDMARK-AND-ID-UNIQUENESS", 8),
            ("A11Y-TABINDEX-AND-FOCUS-RING", 11),
            ("A11Y-HIDDEN-FOCUSABLE", 11),
        ]
    );
    Ok(())
}
