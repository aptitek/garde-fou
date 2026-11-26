# Drizzle ORM Schema & Query Safety Standards (`GF080`–`GF081`)

## Dialect Alignment, Bounded Mutations & Indexed Foreign Keys

- Match Drizzle table imports (`drizzle-orm/sqlite-core` for Cloudflare D1 / SQLite vs `drizzle-orm/pg-core` for PostgreSQL vs `drizzle-orm/mysql-core` for MySQL) strictly to the active workspace SQL dialect (`DRIZZLE-DIALECT-MISMATCH`).
- Always chain `.where(...)` onto `db.update(table).set(...)` and `db.delete(table)` queries (`DRIZZLE-UNBOUNDED-MUTATION`), and use parameterized `` sql`... ${param}` `` tagged templates instead of interpolated `sql.raw(...)` (`DRIZZLE-RAW-UNSAFE`).
- Cover every `.references(() => ...)` foreign-key column with an explicit `index(...)` or `uniqueIndex(...)` in the table definition (`DRIZZLE-FK-MISSING-INDEX`), and store monetary columns (`price`, `amount`, `balance`, `total`, `cost`, `fee`) as integer minor units (`integer` / `bigint`) or exact `numeric` rather than `real` / `doublePrecision` / `float` (`DRIZZLE-FLOAT-MONEY`).
