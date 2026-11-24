# Django Application Architecture

## Thin Views & Domain Services

- Keep `views.py` and `urls.py` focused on HTTP transport, placing domain invariants and query composition inside dedicated service modules covered by deterministic tests (`GF076`).
