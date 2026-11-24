# Next.js App & Pages Router Architecture

## Server-First Boundaries

- Keep `app/**` and `pages/**` route files focused on composition, delegating validation and domain workflows to pure modules under `src/`.
- Isolate client-only interactive state into minimal leaf components without leaking inline styles or raw design primitives (`GF060`, `GF061`).
