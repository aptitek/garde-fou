## Symfony 7, Doctrine ORM 3 & Twig 3 Guardrails (`GF080`)

### Dependency Injection, ORM Performance & XSS-Free Templates

- Use constructor autowiring instead of `$this->container->get()` (`SYMFONY-SERVICE-LOCATOR`), bind all SQL/DQL parameters via `setParameter()` (`SYMFONY-SQL-DQL-INJECTION`), and never call `->flush()` inside loops (`SYMFONY-FLUSH-IN-LOOP`) or unbounded `->findAll()` (`SYMFONY-UNBOUNDED-FINDALL`).
- Protect mutating `#[Route]` actions with `#[IsGranted]` / `denyAccessUnlessGranted()` (`SYMFONY-MISSING-AUTH`), keep `#[ORM\Entity]` classes non-`final` (`SYMFONY-FINAL-ENTITY`), and migrate legacy `@Route`/`@ORM\` annotations to PHP 8 attributes (`SYMFONY-LEGACY-ANNOTATION`).
- Keep `.twig` templates free of `|raw` / `{% autoescape false %}` (`TWIG-RAW-XSS`), inline `style=`/`<script>` (`TWIG-INLINE-STYLE-OR-SCRIPT`), `{{ dump() }}` (`TWIG-DUMP-LEFTOVER`), and unbalanced `{% block/if/for %}` tags (`TWIG-UNBALANCED-BLOCK`); keep XLIFF catalogs (`.xlf`) 100% key- and placeholder-symmetric (`GF062`).
