//! Direct unit tests in `gf-rules` for `docker_checks.rs` and `docker_instr_checks.rs` (`GF030`–`GF034`).

use std::collections::BTreeSet;
use std::slice;

use gf_core::{
    Config, DockerDocument, DockerInstruction, DockerInstructionKind, FeatureId, FromStage,
    Language, LanguageId, ProjectContext, RepoPath, Span, UastDocument,
};
use gf_rules::evaluate_workspace_sorted;

type Res = Result<(), Box<dyn std::error::Error>>;

fn make_df(doc: DockerDocument) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut d = UastDocument::new(RepoPath::new("Dockerfile")?, Language::Dockerfile, 10, 8)?;
    d.docker = Some(doc);
    Ok(d)
}

fn stage(image: &str, tag: Option<&str>, digest: Option<&str>, user: Option<&str>) -> FromStage {
    FromStage {
        image: String::from(image),
        tag: tag.map(String::from),
        digest: digest.map(String::from),
        alias: None,
        effective_user: user.map(String::from),
        span: Span::FIRST,
    }
}

#[test]
fn docker_stages_pinning_and_dockerignore_boundaries() -> Res {
    let mut builder = stage("rust", Some("1.88"), None, None);
    builder.alias = Some(String::from("Builder"));
    let df = make_df(DockerDocument {
        stages: vec![builder, stage("builder", None, None, Some("appuser"))],
        instructions: Vec::new(),
    })?;

    let mut ctx = ProjectContext::default();
    assert_eq!(
        evaluate_workspace_sorted(slice::from_ref(&df), &ctx, &Config::default()),
        Vec::new()
    );

    ctx.features.insert(FeatureId::Docker);
    let missing_ignore = evaluate_workspace_sorted(slice::from_ref(&df), &ctx, &Config::default());
    assert!(
        missing_ignore
            .iter()
            .any(|d| d.rule_id == "GF034" && d.message.contains("missing"))
    );

    ctx.languages
        .extend([LanguageId::Rust, LanguageId::TypeScript, LanguageId::Python]);
    ctx.dockerignore_entries = Some(BTreeSet::from([String::from(".git")]));
    let incomplete = evaluate_workspace_sorted(slice::from_ref(&df), &ctx, &Config::default());
    assert!(
        incomplete
            .iter()
            .any(|d| d.rule_id == "GF034" && d.message.contains("target"))
    );

    ctx.dockerignore_entries = Some(
        [".git", "target", "node_modules", ".venv", "__pycache__"]
            .into_iter()
            .map(String::from)
            .collect(),
    );
    assert_eq!(
        evaluate_workspace_sorted(slice::from_ref(&df), &ctx, &Config::default()),
        Vec::new()
    );
    verify_root_users_and_pinning(&ctx)
}

fn verify_root_users_and_pinning(ctx: &ProjectContext) -> Res {
    for root_u in [
        None,
        Some(""),
        Some("ROOT"),
        Some("0"),
        Some("root:0"),
        Some("0:0"),
    ] {
        let bad = make_df(DockerDocument {
            stages: vec![stage("debian", Some("12-slim"), None, root_u)],
            instructions: Vec::new(),
        })?;
        assert!(
            evaluate_workspace_sorted(&[bad], ctx, &Config::default())
                .iter()
                .any(|d| d.rule_id == "GF031")
        );
    }
    for unpinned in [
        stage("ubuntu", None, None, Some("u")),
        stage("ubuntu", Some("LATEST"), None, Some("u")),
    ] {
        let bad = make_df(DockerDocument {
            stages: vec![unpinned],
            instructions: Vec::new(),
        })?;
        assert!(
            evaluate_workspace_sorted(&[bad], ctx, &Config::default())
                .iter()
                .any(|d| d.rule_id == "GF030")
        );
    }
    let ok_special = make_df(DockerDocument {
        stages: vec![
            stage("alpine", None, Some("sha256:abc"), Some("1000")),
            stage("scratch", None, None, None),
        ],
        instructions: Vec::new(),
    })?;
    assert_eq!(
        evaluate_workspace_sorted(&[ok_special], ctx, &Config::default()),
        Vec::new()
    );
    Ok(())
}

#[test]
fn docker_instructions_enforce_gf032_and_gf033_exact_guards() -> Res {
    let mut ctx = ProjectContext::default();
    ctx.features.insert(FeatureId::Docker);
    ctx.dockerignore_entries = Some(BTreeSet::from([String::from(".git")]));
    let mk = |kind: DockerInstructionKind| DockerInstruction {
        kind,
        span: Span::FIRST,
    };

    let bad = make_df(DockerDocument {
        stages: vec![stage("alpine", Some("3.20"), None, Some("app"))],
        instructions: vec![
            mk(DockerInstructionKind::Add {
                operands: vec!["local.txt".into(), "/app/".into()],
            }),
            mk(DockerInstructionKind::Run {
                command: "sudo apt-get install -y curl && pip install flask".into(),
            }),
            mk(DockerInstructionKind::Env {
                bindings: vec![("API_KEY".into(), "hardcoded".into())],
            }),
            mk(DockerInstructionKind::Arg {
                name: "DB_PASSWORD".into(),
                default_value: Some("sec".into()),
            }),
            mk(DockerInstructionKind::Workdir {
                path: "rel_dir".into(),
            }),
        ],
    })?;
    let diags = evaluate_workspace_sorted(&[bad], &ctx, &Config::default());
    assert_eq!(diags.iter().filter(|d| d.rule_id == "GF032").count(), 4);
    assert_eq!(diags.iter().filter(|d| d.rule_id == "GF033").count(), 3);

    let clean = make_df(DockerDocument {
        stages: vec![stage("alpine", Some("3.20"), None, Some("app"))],
        instructions: vec![
            mk(DockerInstructionKind::Add { operands: vec!["https://x.io/a".into(), "http://x.io/b".into(), "p.tar.gz".into(), "/app/".into()] }),
            mk(DockerInstructionKind::Run { command: "my-sudo my_sudo apt-get install -y x && rm -rf /var/lib/apt/lists/* && pip install --no-cache-dir y".into() }),
            mk(DockerInstructionKind::Run { command: "--mount=type=cache apt-get install -y x && pip install y".into() }),
            mk(DockerInstructionKind::Env { bindings: vec![("API_KEY".into(), String::new()), ("API_KEY".into(), "$ENV_KEY".into()), ("PORT".into(), "8080".into())] }),
            mk(DockerInstructionKind::Arg { name: "SECRET_TOKEN".into(), default_value: Some("$TOK".into()) }),
            mk(DockerInstructionKind::Workdir { path: "/app".into() }),
            mk(DockerInstructionKind::Workdir { path: "$APP_HOME".into() }),
        ],
    })?;
    assert_eq!(
        evaluate_workspace_sorted(&[clean], &ctx, &Config::default()),
        Vec::new()
    );
    Ok(())
}
