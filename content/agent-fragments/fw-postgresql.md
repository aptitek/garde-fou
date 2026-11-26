# PostgreSQL Avancé, RLS & PL/pgSQL (`GF080`–`GF082`)

## Sécurité RLS & `SECURITY DEFINER`

- Toujours appairer `ALTER TABLE <table> ENABLE ROW LEVEL SECURITY` avec au moins une `CREATE POLICY ... ON <table>` (`PG-RLS-WITHOUT-POLICY` / `PG-POLICY-WITHOUT-RLS`).
- Attacher `SET search_path = pg_catalog, pg_temp` (ou `SET search_path = ''`) directement sur toute fonction ou procédure `SECURITY DEFINER` (`PG-SECURITY-DEFINER-SEARCH-PATH`).

## Fiabilité PL/pgSQL & Tests `pgTAP`

- Préférer `SELECT ... INTO STRICT <var>` (ou un agrégat scalaire / `LIMIT 1` explicite) pour éviter les lectures silencieusement nulles ou multiples (`PLPGSQL-MISSING-INTO-STRICT`).
- Construire le SQL dynamique avec `EXECUTE format('... %I ... $1', ident) USING val` et interdire la concaténation `||` ou le spécificateur brut `%s` (`PLPGSQL-UNSAFE-DYNAMIC-EXECUTE`).
- Vérifier les fonctions et politiques RLS via des suites `pgTAP` (`SELECT plan(...)`, `SELECT ok(...)`, `SELECT is(...)`, `SELECT throws_ok(...)`, `GF076`).
