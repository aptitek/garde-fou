# Oracle PL/SQL Industriel & `utPLSQL` (`GF080`–`GF082`)

## Sécurité, Droits d'Invocateur & SQL Dynamique

- Déclarer `AUTHID CURRENT_USER` sur les packages, procédures et fonctions applicatifs (`PLSQL-MISSING-AUTHID-CURRENT-USER`) et maintenir une parité exacte entre `CREATE PACKAGE` (`.pks`) et `CREATE PACKAGE BODY` (`.pkb`, `PLSQL-PACKAGE-SPEC-MISMATCH`).
- Utiliser des variables liées (`:1`) avec `USING` (et `DBMS_ASSERT.SIMPLE_SQL_NAME` / `ENQUOTE_NAME` pour les noms d'objets) dans `EXECUTE IMMEDIATE` / `OPEN ... FOR` (`PLSQL-EXECUTE-IMMEDIATE-INJECTION`).

## Performance, Transactions & Qualité Procédurale

- Remplacer les DML ligne-à-ligne dans `LOOP` par `FETCH ... BULK COLLECT INTO ... LIMIT <n>` + `FORALL ... SAVE EXCEPTIONS` (`PLSQL-DML-IN-LOOP`, `PLSQL-BULK-COLLECT-NO-LIMIT`).
- Interdire `COMMIT`/`ROLLBACK` dans les boucles et triggers sans `PRAGMA AUTONOMOUS_TRANSACTION`, et toujours terminer une transaction autonome par `COMMIT`/`ROLLBACK` (`PLSQL-COMMIT-IN-LOOP`, `PLSQL-AUTONOMOUS-TX-UNTERMINATED`).
- Relancer les erreurs dans `WHEN OTHERS` (`RAISE;` / `RAISE_APPLICATION_ERROR`), fermer tout curseur ouvert (`PLSQL-SWALLOWED-WHEN-OTHERS`, `PLSQL-UNCLOSED-CURSOR`) et tester chaque package avec `utPLSQL` (`--%test` + `ut.expect(...).to_*`, `GF076`).
