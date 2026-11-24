# FastAPI Service Architecture

## Typed Boundary & Pure Services

- Keep ASGI route functions under `routers/` thin: validate inputs/outputs with typed schemas and delegate business rules to pure, I/O-isolated service functions.
- Avoid module-level mutable singletons; pass dependencies explicitly with `<= 4` parameters per function (`GF003`).
