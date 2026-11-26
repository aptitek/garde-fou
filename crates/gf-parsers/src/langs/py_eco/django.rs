//! Django 5.2 LTS / 6.0+ CST analyzer (`DJ-*` under `GF080`).

use gf_core::{DjangoViolation, LangRuleFinding, RepoPath, is_django_migration_path};
use tree_sitter::Node;

use crate::cst::{node_span, node_text};

#[rustfmt::skip]
const STRING_MODEL_FIELDS: &[&str] = &["CharField", "TextField", "SlugField", "EmailField", "FilePathField", "URLField"];

fn push_dj(out: &mut Vec<LangRuleFinding>, v: DjangoViolation, node: Node<'_>, msg: String) {
    out.push(LangRuleFinding::new(v, node_span(node), msg));
}

fn compact_ws(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_whitespace()).collect()
}

fn is_dynamic_sql_arg(arg: Node<'_>, source: &str) -> bool {
    let txt = node_text(arg, source).trim();
    ["f\"", "f'", "rf\"", "fr\""]
        .iter()
        .any(|p| txt.starts_with(p))
        || [".format(", "\" +", "' +", "\" %", "' %"]
            .iter()
            .any(|m| txt.contains(m))
}

fn first_call_arg(call: Node<'_>) -> Option<Node<'_>> {
    let args = call.child_by_field_name("arguments")?;
    let mut c = args.walk();
    args.named_children(&mut c).next()
}

fn check_dj_sql_and_html(
    node: Node<'_>,
    method: &str,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let Some(first) = first_call_arg(node) else {
        return;
    };
    if !is_dynamic_sql_arg(first, source) {
        return;
    }
    if matches!(method, "RawSQL" | "raw" | "execute" | "RunSQL") {
        let msg = format!("dynamic SQL in `{method}(...)`; pass static SQL and `params`");
        push_dj(out, DjangoViolation::RawSqlOrUnsafeHtml, node, msg);
    } else if method == "format_html" {
        let msg = String::from("pre-formatted `format_html(...)` bypasses escaping; pass args");
        push_dj(out, DjangoViolation::RawSqlOrUnsafeHtml, node, msg);
    }
}

fn check_dj_call(node: Node<'_>, source: &str, flags: u8, out: &mut Vec<LangRuleFinding>) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let ftxt = node_text(func, source).trim();
    let method = ftxt.rsplit('.').next().unwrap_or(ftxt);
    let args_txt = node
        .child_by_field_name("arguments")
        .map_or("", |a| node_text(a, source));
    if method == "extra" && ftxt.contains('.') {
        let msg = String::from("Django `.extra(...)` is SQL-injection prone; use ORM expressions");
        push_dj(out, DjangoViolation::RawSqlOrUnsafeHtml, node, msg);
    }
    check_dj_sql_and_html(node, method, source, out);
    if (flags & 1) != 0 && method == "mark_safe" {
        let msg = String::from("avoid `mark_safe(...)`; use `format_html(template, *args)`");
        push_dj(out, DjangoViolation::RawSqlOrUnsafeHtml, node, msg);
    }
    if method == "render" && args_txt.contains("locals()") {
        let msg = String::from("avoid passing `locals()` to Django `render(...)` (`DJ003`)");
        push_dj(out, DjangoViolation::ModelAndFormHygiene, node, msg);
    }
    if (flags & 2) != 0
        && ftxt.contains(".objects.")
        && matches!(method, "get" | "filter" | "count" | "exists" | "all")
    {
        let msg = format!("Django ORM `.objects.{method}(...)` inside loop causes N+1 queries");
        push_dj(out, DjangoViolation::OrmNPlusOneAndUnbounded, node, msg);
    }
}

fn check_dj_field_node(n: Node<'_>, source: &str, is_model: bool, out: &mut Vec<LangRuleFinding>) {
    let txt = node_text(n, source);
    if is_model && n.kind() == "call" {
        let fn_name = n
            .child_by_field_name("function")
            .map_or("", |f| node_text(f, source).trim());
        let base = fn_name.rsplit('.').next().unwrap_or(fn_name);
        let compact = compact_ws(txt);
        let unique_blank = compact.contains("unique=True") && compact.contains("blank=True");
        if STRING_MODEL_FIELDS.contains(&base) && compact.contains("null=True") && !unique_blank {
            let msg =
                format!("avoid `null=True` on `{base}` unless `unique=True, blank=True` (`DJ001`)");
            push_dj(out, DjangoViolation::ModelAndFormHygiene, n, msg);
        }
    }
    if n.kind() == "assignment"
        && (txt.contains("fields = \"__all__\"") || txt.contains("fields = '__all__'"))
    {
        let msg =
            String::from("avoid `fields = \"__all__\"` on `ModelForm`/`ModelSerializer` (`DJ007`)");
        push_dj(out, DjangoViolation::ModelAndFormHygiene, n, msg);
    }
}

fn check_dj_model_fields(
    body: Node<'_>,
    source: &str,
    is_model: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    let mut stack = vec![body];
    while let Some(n) = stack.pop() {
        check_dj_field_node(n, source, is_model, out);
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
    }
}

fn check_dj_model_class(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let txt = node_text(node, source);
    let header = txt.lines().next().unwrap_or("");
    let is_model = header.contains("models.Model") || header.contains("(Model)");
    if is_model
        && !txt.contains("abstract = True")
        && !txt.contains("abstract=True")
        && !txt.contains("def __str__(")
    {
        let msg = String::from(
            "concrete Django `Model` must define `def __str__(self) -> str:` (`DJ008`)",
        );
        push_dj(out, DjangoViolation::ModelAndFormHygiene, node, msg);
    }
    if let Some(body) = node.child_by_field_name("body") {
        check_dj_model_fields(body, source, is_model, out);
    }
}

fn check_dj_settings(
    root: Node<'_>,
    path: &RepoPath,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let raw = path.as_str();
    let is_prod = path.file_name().contains("settings")
        && !raw.contains("local")
        && !raw.contains("test")
        && !raw.contains("dev");
    if !is_prod {
        return;
    }
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "assignment" {
            let compact = compact_ws(node_text(n, source));
            if matches!(
                compact.as_str(),
                "DEBUG=True"
                    | "ALLOWED_HOSTS=[\"*\"]"
                    | "ALLOWED_HOSTS=['*']"
                    | "CORS_ALLOW_ALL_ORIGINS=True"
            ) {
                let msg = format!("insecure production Django setting `{compact}`");
                push_dj(out, DjangoViolation::SettingsAndAuthHardening, n, msg);
            }
        }
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
    }
}

fn walk_dj(node: Node<'_>, source: &str, flags: u8, out: &mut Vec<LangRuleFinding>) {
    let is_mig = (flags & 4) != 0;
    match node.kind() {
        "call" => check_dj_call(node, source, flags, out),
        "class_definition" if !is_mig => check_dj_model_class(node, source, out),
        "return_statement" if !is_mig => {
            let txt = node_text(node, source);
            if txt.contains(".objects.all()") && !txt.contains("[:") && !txt.contains(".values(") {
                let msg =
                    String::from("unbounded `Model.objects.all()` returned; paginate or slice");
                push_dj(out, DjangoViolation::OrmNPlusOneAndUnbounded, node, msg);
            }
        }
        _ => {}
    }
    let next = if matches!(node.kind(), "for_statement" | "while_statement") {
        flags | 2
    } else {
        flags
    };
    let mut c = node.walk();
    for ch in node.named_children(&mut c) {
        walk_dj(ch, source, next, out);
    }
}

/// Extracts Django 5.2/6.0 CST findings (`DJ-*`) without I/O.
pub fn extract_django_rules(
    root: Node<'_>,
    source: &str,
    in_code: bool,
    doc: &mut gf_core::UastDocument,
) {
    let is_mig = is_django_migration_path(doc.path.as_str());
    let has_dj = is_mig
        || source.contains("django")
        || source.contains("models.Model")
        || source.contains("format_html")
        || source.contains("mark_safe")
        || doc.path.file_name().contains("settings");
    if !has_dj {
        return;
    }
    let flags = u8::from(in_code) | (u8::from(is_mig) << 2);
    walk_dj(root, source, flags, &mut doc.lang_rule_findings);
    if !is_mig {
        check_dj_settings(root, &doc.path, source, &mut doc.lang_rule_findings);
    }
}
