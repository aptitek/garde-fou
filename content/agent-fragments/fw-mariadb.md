# MariaDB 10.11 / 11.4 & Online DDL (`GF080`–`GF082`)

## Transactions, Encodage & Moteur InnoDB

- Ne jamais exécuter de DDL (`CREATE`, `ALTER`, `DROP`, `TRUNCATE`, `RENAME`) dans une transaction explicite `START TRANSACTION` / `BEGIN`, car MariaDB déclenche un `COMMIT` implicite irréversible (`MARIADB-IMPLICIT-DDL-COMMIT`).
- Utiliser exclusivement `CHARACTER SET utf8mb4 COLLATE utf8mb4_uca1400_ai_ci` au lieu de `utf8` / `utf8mb3` (`MARIADB-UTF8MB3-TRAP`).
- Exiger `ENGINE=InnoDB`, conserver les contraintes `CHECK` actives (`ENFORCED`) et utiliser le type `JSON` ou `CHECK (JSON_VALID(col))` (`MARIADB-NON-ACID-ENGINE-AND-JSON`).

## Migrations Zero-Downtime

- Qualifier les `ALTER TABLE` sur des tables existantes avec `ALGORITHM=INSTANT` ou `ALGORITHM=INPLACE, LOCK=NONE` et interdire `ALGORITHM=COPY` / `LOCK=EXCLUSIVE` (`MARIADB-BLOCKING-ALTER-DDL`).
- Utiliser `DELIMITER //` ... `DELIMITER ;` pour les routines stockées tout en respectant les limites SRP (`<= 50` lignes, complexité cognitive `<= 10`).
