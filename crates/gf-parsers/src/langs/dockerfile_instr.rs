//! Semantic `Dockerfile` instruction parser (`FROM`, `USER`, `RUN`, `ADD`, `ENV`, `ARG`, etc.).

use gf_core::{DockerDocument, DockerInstruction, DockerInstructionKind, FromStage, Span};

const OTHER_VALID_KEYWORDS: &[&str] = &[
    "CMD",
    "ENTRYPOINT",
    "EXPOSE",
    "HEALTHCHECK",
    "LABEL",
    "MAINTAINER",
    "ONBUILD",
    "SHELL",
    "STOPSIGNAL",
    "VOLUME",
];

fn split_image_ref(spec: &str) -> (String, Option<String>, Option<String>) {
    let (without_digest, digest) = spec.split_once('@').map_or_else(
        || (spec, None),
        |(img, dig)| (img, (!dig.is_empty()).then(|| String::from(dig))),
    );
    let last_slash = without_digest.rfind('/').unwrap_or(0);
    let tail = &without_digest[last_slash..];
    if let Some(colon_rel) = tail.find(':') {
        let colon_idx = last_slash + colon_rel;
        let image = String::from(&without_digest[..colon_idx]);
        let tag_str = &without_digest[colon_idx + 1..];
        let tag = (!tag_str.is_empty()).then(|| String::from(tag_str));
        (image, tag, digest)
    } else {
        (String::from(without_digest), None, digest)
    }
}

fn parse_from(
    args: &str,
    span: Span,
    doc: &mut DockerDocument,
) -> Result<DockerInstructionKind, String> {
    let tokens: Vec<&str> = args
        .split_whitespace()
        .filter(|tok| !tok.starts_with("--"))
        .collect();
    let Some(spec) = tokens.first() else {
        return Err(String::from("FROM instruction requires a base image"));
    };
    let alias = if tokens.len() >= 3 && tokens[1].eq_ignore_ascii_case("AS") {
        Some(String::from(tokens[2]))
    } else if tokens.len() > 1 {
        return Err(String::from(
            "malformed FROM clause (expected `FROM <image> [AS <name>]`)",
        ));
    } else {
        None
    };

    let (image, tag, digest) = split_image_ref(spec);
    let inherited_user = doc
        .stages
        .iter()
        .find(|s| {
            s.alias
                .as_deref()
                .is_some_and(|a| a.eq_ignore_ascii_case(&image))
        })
        .and_then(|s| s.effective_user.clone());

    let stage_index = doc.stages.len();
    doc.stages.push(FromStage {
        image,
        tag,
        digest,
        alias,
        effective_user: inherited_user,
        span,
    });
    Ok(DockerInstructionKind::From { stage_index })
}

fn parse_env(args: &str) -> Result<DockerInstructionKind, String> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return Err(String::from(
            "ENV instruction requires at least one key-value pair",
        ));
    }
    let mut bindings = Vec::new();
    if trimmed.contains('=') {
        for token in trimmed.split_whitespace() {
            if let Some((k, v)) = token.split_once('=') {
                bindings.push((String::from(k), String::from(v.trim_matches(['"', '\'']))));
            }
        }
    } else if let Some((k, v)) = trimmed.split_once(char::is_whitespace) {
        bindings.push((
            String::from(k.trim()),
            String::from(v.trim().trim_matches(['"', '\''])),
        ));
    } else {
        return Err(String::from("ENV instruction missing value"));
    }
    Ok(DockerInstructionKind::Env { bindings })
}

fn parse_arg(args: &str) -> Result<DockerInstructionKind, String> {
    let first = args
        .split_whitespace()
        .next()
        .ok_or_else(|| String::from("ARG instruction requires a variable name"))?;
    let (name, default_value) = first.split_once('=').map_or_else(
        || (String::from(first), None),
        |(k, v)| {
            (
                String::from(k),
                Some(String::from(v.trim_matches(['"', '\'']))),
            )
        },
    );
    if name.is_empty() {
        return Err(String::from("ARG variable name cannot be empty"));
    }
    Ok(DockerInstructionKind::Arg {
        name,
        default_value,
    })
}

fn parse_non_empty(label: &str, args: &str) -> Result<String, String> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        Err(format!("{label} instruction requires arguments"))
    } else {
        Ok(String::from(trimmed))
    }
}

/// Parses a single logical `Dockerfile` instruction line and appends it to `doc`.
pub fn parse_instruction(raw: &str, span: Span, doc: &mut DockerDocument) -> Result<(), String> {
    let trimmed = raw.trim();
    let (kw_raw, rest) = trimmed
        .split_once(char::is_whitespace)
        .map_or((trimmed, ""), |(k, r)| (k, r.trim()));
    let keyword = kw_raw.to_ascii_uppercase();

    if doc.stages.is_empty() && keyword != "FROM" && keyword != "ARG" {
        return Err(format!(
            "instruction `{keyword}` cannot appear before the first `FROM` stage"
        ));
    }

    let kind = dispatch_keyword(&keyword, rest, span, doc)?;
    doc.instructions.push(DockerInstruction { kind, span });
    Ok(())
}

fn parse_add(rest: &str) -> Result<DockerInstructionKind, String> {
    let operands: Vec<String> = rest
        .split_whitespace()
        .filter(|t| !t.starts_with("--"))
        .map(String::from)
        .collect();
    if operands.len() < 2 {
        return Err(String::from("ADD requires source and destination operands"));
    }
    Ok(DockerInstructionKind::Add { operands })
}

fn parse_other_or_error(keyword: &str, rest: &str) -> Result<DockerInstructionKind, String> {
    if OTHER_VALID_KEYWORDS.contains(&keyword) {
        Ok(DockerInstructionKind::Other {
            keyword: String::from(keyword),
            arguments: String::from(rest),
        })
    } else {
        Err(format!("unknown Dockerfile instruction `{keyword}`"))
    }
}

fn dispatch_keyword(
    keyword: &str,
    rest: &str,
    span: Span,
    doc: &mut DockerDocument,
) -> Result<DockerInstructionKind, String> {
    match keyword {
        "FROM" => parse_from(rest, span, doc),
        "USER" => {
            let user = parse_non_empty("USER", rest)?;
            if let Some(stage) = doc.stages.last_mut() {
                stage.effective_user = Some(user.clone());
            }
            Ok(DockerInstructionKind::User { user })
        }
        "RUN" => Ok(DockerInstructionKind::Run {
            command: parse_non_empty("RUN", rest)?,
        }),
        "COPY" => Ok(DockerInstructionKind::Copy {
            arguments: parse_non_empty("COPY", rest)?,
        }),
        "ADD" => parse_add(rest),
        "WORKDIR" => Ok(DockerInstructionKind::Workdir {
            path: parse_non_empty("WORKDIR", rest)?,
        }),
        "ENV" => parse_env(rest),
        "ARG" => parse_arg(rest),
        other => parse_other_or_error(other, rest),
    }
}
