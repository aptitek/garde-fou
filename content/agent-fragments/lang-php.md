## PHP 8.4 Quality Standards (`GF080`–`GF082`)

### Strict Types, Safe Execution & PSR-4 Hygiene

- Start every `.php` file with `declare(strict_types=1);` (`PHP-STRICT-TYPES`) and use strict identity operators `===` / `!==` (`PHP-LOOSE-COMPARISON`).
- Forbid `eval()`, `exec()`, `shell_exec()`, `system()`, `passthru()`, `unserialize()`, `extract()` (`PHP-UNSAFE-EXEC-OR-UNSERIALIZE`), direct `$_GET`/`$_POST`/`$_REQUEST` superglobals (`PHP-SUPERGLOBAL-ACCESS`), `@` error suppression (`PHP-ERROR-SUPPRESSION`), and `var_dump`/`dd`/`dump`/`print_r` leftovers (`PHP-DEBUG-LEFTOVER`).
- Declare `require.php` (`>=8.4`) and `autoload.psr-4` in `composer.json`, keep `phpstan.neon` at `level: 8` or `max` (`GF081`), and remediate `PHPStan`/`Psalm` diagnostics (`GF082`).
