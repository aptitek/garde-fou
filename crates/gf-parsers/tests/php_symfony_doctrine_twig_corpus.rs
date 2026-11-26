//! Symmetric corpus tests for PHP 8.4, Symfony 7, Doctrine ORM 3, Twig 3, XLIFF, and PHPStan/Psalm.

use gf_core::{ExternalToolKind, LangRuleFinding, RepoPath};
use gf_parsers::{parse_phpstan_json, parse_psalm_json, parse_source};

#[test]
fn php_symfony_bad_corpus_catches_all_rules() -> Result<(), String> {
    let bad_path =
        RepoPath::new("src/Controller/OrderController.php").map_err(|e| e.to_string())?;
    let bad_src = r#"<?php
/** @Route("/orders") */
class OrderController {
    #[Route('/orders', methods: ['POST'])]
    public function create(): void {
        $id = $_GET['id'];
        $svc = $this->container->get('app.mailer');
        $em->createQuery("SELECT u FROM User u WHERE u.id = " . $id);
        foreach ($items as $it) { $em->flush(); }
        $all = $repo->findAll();
        if ($id == 1) { var_dump($all); }
        @unlink('/tmp/x');
        eval($id);
    }
}
#[ORM\Entity]
final class BadEntity {}
"#;
    let doc = parse_source(&bad_path, bad_src).map_err(|e| e.to_string())?;
    let codes: Vec<&str> = doc
        .lang_rule_findings
        .iter()
        .map(LangRuleFinding::code)
        .collect();
    for expected in [
        "PHP-STRICT-TYPES",
        "PHP-UNSAFE-EXEC-OR-UNSERIALIZE",
        "PHP-SUPERGLOBAL-ACCESS",
        "PHP-LOOSE-COMPARISON",
        "PHP-ERROR-SUPPRESSION",
        "PHP-DEBUG-LEFTOVER",
        "SYMFONY-SERVICE-LOCATOR",
        "SYMFONY-SQL-DQL-INJECTION",
        "SYMFONY-FLUSH-IN-LOOP",
        "SYMFONY-MISSING-AUTH",
        "SYMFONY-FINAL-ENTITY",
        "SYMFONY-UNBOUNDED-FINDALL",
        "SYMFONY-LEGACY-ANNOTATION",
    ] {
        assert!(codes.contains(&expected), "missing {expected} in {codes:?}");
    }
    Ok(())
}

#[test]
fn php_symfony_clean_corpus_passes_with_zero_findings() -> Result<(), String> {
    let path = RepoPath::new("src/Controller/CleanController.php").map_err(|e| e.to_string())?;
    let good_src = r"<?php
declare(strict_types=1);
namespace App\Controller;
use App\Service\OrderService;

class CleanController {
    #[Route('/orders', methods: ['POST'])]
    #[IsGranted('ROLE_USER')]
    public function create(OrderService $svc): void {
        if ($svc === null) { return; }
    }
}
";
    let clean = parse_source(&path, good_src).map_err(|e| e.to_string())?;
    assert_eq!(clean.lang_rule_findings.len(), 0);
    Ok(())
}

#[test]
fn twig_xliff_and_php_tool_configs_corpus() -> Result<(), String> {
    let twig_p = RepoPath::new("templates/order.html.twig").map_err(|e| e.to_string())?;
    let bad_twig = "{% block body %}<div style=\"color:red\">{{ html|raw }}{{ dump(x) }}</div>";
    let tdoc = parse_source(&twig_p, bad_twig).map_err(|e| e.to_string())?;
    let tcodes: Vec<&str> = tdoc
        .lang_rule_findings
        .iter()
        .map(LangRuleFinding::code)
        .collect();
    for exp in [
        "TWIG-RAW-XSS",
        "TWIG-INLINE-STYLE-OR-SCRIPT",
        "TWIG-DUMP-LEFTOVER",
        "TWIG-UNBALANCED-BLOCK",
    ] {
        assert!(tcodes.contains(&exp), "missing {exp} in {tcodes:?}");
    }

    let ok_twig = "{% block body %}<p>{{ 'order.title'|trans }}</p>{% endblock %}";
    let ok_tdoc = parse_source(&twig_p, ok_twig).map_err(|e| e.to_string())?;
    assert_eq!(ok_tdoc.lang_rule_findings.len(), 0);
    assert!(ok_tdoc.translation_keys_used.contains("order.title"));

    let xlf_p = RepoPath::new("translations/messages.fr.xlf").map_err(|e| e.to_string())?;
    let xlf = r#"<xliff><file><body><trans-unit id="1"><source>order.title</source><target>Commande {count}</target></trans-unit></body></file></xliff>"#;
    let xdoc = parse_source(&xlf_p, xlf).map_err(|e| e.to_string())?;
    let txt = xdoc.texte.ok_or("missing TexteDocument")?;
    assert_eq!(txt.locale, "fr");
    assert!(txt.keys.contains("order.title"));

    let neon_p = RepoPath::new("phpstan.neon").map_err(|e| e.to_string())?;
    let neon = parse_source(&neon_p, "parameters:\n  level: 5\n").map_err(|e| e.to_string())?;
    assert_eq!(neon.tool_config_findings.len(), 1);

    let comp_p = RepoPath::new("composer.json").map_err(|e| e.to_string())?;
    let comp = parse_source(&comp_p, "{}").map_err(|e| e.to_string())?;
    assert_eq!(comp.tool_config_findings.len(), 1);
    Ok(())
}

#[test]
fn phpstan_and_psalm_json_parsers_work() -> Result<(), String> {
    let phpstan_out = r#"{"files":{"src/App.php":{"messages":[{"message":"Return type mismatch","line":12,"identifier":"return.type"}]}}}"#;
    let f1 = parse_phpstan_json(phpstan_out).map_err(|e| e.to_string())?;
    assert_eq!(f1.len(), 1);
    assert_eq!(f1[0].tool, ExternalToolKind::Phpstan);
    assert_eq!(f1[0].native_code, "return.type");

    let psalm_out = r#"[{"file_name":"src/App.php","line_from":8,"column_from":4,"type":"InvalidReturnType","message":"Bad return"}]"#;
    let f2 = parse_psalm_json(psalm_out).map_err(|e| e.to_string())?;
    assert_eq!(f2.len(), 1);
    assert_eq!(f2[0].tool, ExternalToolKind::Psalm);
    assert_eq!(f2[0].native_code, "InvalidReturnType");
    Ok(())
}
