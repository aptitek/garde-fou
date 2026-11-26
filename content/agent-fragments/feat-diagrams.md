# Diagrammes-as-Code Mermaid v11 & PlantUML (`GF063` / `GF080`)

## Mermaid v11

- Toujours commencer par un en-tête officiel Mermaid v11 (`flowchart`, `sequenceDiagram`, `classDiagram`, `erDiagram`, `C4Context`, `architecture-beta`, etc.) (`MERMAID-UNKNOWN-OR-EMPTY-DIAGRAM`).
- Refermer chaque bloc (`subgraph`, `alt`, `opt`, `loop`, `par`, `rect`, `box`) par `end`, équilibrer les formes de nœuds et ne jamais nommer un nœud `end` en minuscule nu (`MERMAID-UNCLOSED-BLOCK-OR-SHAPE`, `MERMAID-RESERVED-END-NODE`).
- Conserver `securityLevel: 'strict'`, interdire `click ... javascript:` / `call` et `<script>`/`on*=` dans les labels (`MERMAID-UNSAFE-SECURITY-OR-XSS`), et maintenir les diagrammes sous `max_nodes` (`MERMAID-SPAGHETTI-COMPLEXITY`).

## PlantUML Sécurisé

- Appairer chaque `@start<kind>` avec `@end<kind>` et fermer tous les blocs `{ ... }`, `alt`..`end` et `note`..`end note` (`PLANTUML-UNBALANCED-STRUCTURE`).
- Interdire les inclusions distantes (`!include http(s)://...`, `!includeurl`, `!import`), les traversées `..`/chemins absolus et `%getenv`/`%file_exists`/`!log` (`PLANTUML-SSRF-OR-LOCAL-LEAK`), tout en autorisant les bibliothèques standard `<C4/...>` et les `!include ./local.iuml` existants (`PLANTUML-BROKEN-INCLUDE`).
- Interdire les hyperliens `[[javascript:...]]`, `[[vbscript:...]]` et `[[data:...]]` (`PLANTUML-XSS-LINK`).
