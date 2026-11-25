//! Supply-chain dependency pinning checks (`XML-UNPINNED-DEPENDENCY` -> `GF080`) for
//! `NuGet`/`MSBuild` (`.csproj`, `Directory.Packages.props`) and Maven (`pom.xml`) manifests.

use gf_core::{LangRuleFinding, RepoPath, Span, XmlSvgViolation};
use roxmltree::{Document, Node};

fn span_at(doc: &Document<'_>, byte_offset: usize) -> Span {
    let row = doc.text_pos_at(byte_offset).row.max(1);
    Span::new(row, row).unwrap_or(Span::FIRST)
}

fn is_unpinned_nuget_version(ver: &str) -> bool {
    let t = ver.trim();
    t.is_empty() || t.contains('*') || t.eq_ignore_ascii_case("latest")
}

fn is_unpinned_maven_version(ver: &str) -> bool {
    let t = ver.trim();
    t.contains('*') || t.eq_ignore_ascii_case("latest") || t.eq_ignore_ascii_case("release")
}

fn child_element<'a, 'input>(node: Node<'a, 'input>, tag: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name().eq_ignore_ascii_case(tag))
}

fn check_msbuild_package_node(
    doc: &Document<'_>,
    node: Node<'_, '_>,
    out: &mut Vec<LangRuleFinding>,
) {
    let pkg = node
        .attributes()
        .find(|a| {
            a.name().eq_ignore_ascii_case("Include") || a.name().eq_ignore_ascii_case("Update")
        })
        .map_or("<pkg>", |a| a.value());
    if let Some(ver_attr) = node
        .attributes()
        .find(|a| a.name().eq_ignore_ascii_case("Version"))
    {
        if is_unpinned_nuget_version(ver_attr.value()) {
            out.push(LangRuleFinding::new(
                XmlSvgViolation::UnpinnedDependency,
                span_at(doc, ver_attr.range().start),
                format!(
                    "NuGet package `{pkg}` uses floating version `{}`",
                    ver_attr.value()
                ),
            ));
        }
    } else if let Some(ver_el) = child_element(node, "Version")
        && let Some(txt) = ver_el.text()
        && is_unpinned_nuget_version(txt)
    {
        out.push(LangRuleFinding::new(
            XmlSvgViolation::UnpinnedDependency,
            span_at(doc, ver_el.range().start),
            format!(
                "NuGet package `{pkg}` uses floating `<Version>{}</Version>`",
                txt.trim()
            ),
        ));
    }
}

fn check_msbuild_manifest(doc: &Document<'_>, out: &mut Vec<LangRuleFinding>) {
    for node in doc.descendants().filter(Node::is_element) {
        let tag = node.tag_name().name();
        if tag.eq_ignore_ascii_case("PackageReference")
            || tag.eq_ignore_ascii_case("PackageVersion")
        {
            check_msbuild_package_node(doc, node, out);
        }
    }
}

fn check_pom_manifest(doc: &Document<'_>, out: &mut Vec<LangRuleFinding>) {
    for node in doc.descendants().filter(Node::is_element) {
        let tag = node.tag_name().name();
        if (tag == "dependency" || tag == "plugin")
            && let Some(ver_el) = child_element(node, "version")
            && let Some(txt) = ver_el.text()
            && is_unpinned_maven_version(txt)
        {
            let artifact = child_element(node, "artifactId")
                .and_then(|a| a.text())
                .map_or("<artifact>", str::trim);
            out.push(LangRuleFinding::new(
                XmlSvgViolation::UnpinnedDependency,
                span_at(doc, ver_el.range().start),
                format!(
                    "Maven `{artifact}` uses floating `<version>{}</version>`",
                    txt.trim()
                ),
            ));
        } else if (tag == "repository" || tag == "pluginRepository")
            && let Some(url_el) = child_element(node, "url")
            && let Some(url_txt) = url_el.text()
            && url_txt.trim().to_ascii_lowercase().starts_with("http://")
        {
            out.push(LangRuleFinding::new(
                XmlSvgViolation::UnpinnedDependency,
                span_at(doc, url_el.range().start),
                format!(
                    "unencrypted HTTP Maven repository `{}` in `<{tag}>`",
                    url_txt.trim()
                ),
            ));
        }
    }
}

/// Evaluates `XML-UNPINNED-DEPENDENCY` when `path` is `.csproj`, `*.props`, or `pom.xml`.
pub fn check_xml_package_manifest(
    path: &RepoPath,
    dom: &Document<'_>,
    out: &mut Vec<LangRuleFinding>,
) {
    if path.file_name().eq_ignore_ascii_case("pom.xml") {
        check_pom_manifest(dom, out);
    } else if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csproj") || e.eq_ignore_ascii_case("props"))
    {
        check_msbuild_manifest(dom, out);
    }
}
