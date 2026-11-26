# React Router v7 Data & Framework Architecture (`GF021`, `GF072`, `GF080`)

## Server/Client Separation, Loaders & Progressive Navigation

- Keep server-only modules (`*.server.*`, `drizzle-orm/*`, server credentials) confined to `loader` and `action` route functions; never import or reference them in UI components, `*.client.*` modules, `clientLoader`, `clientAction`, or JSX render trees (`RR-SERVER-LEAK-TO-CLIENT`).
- Keep `loader` and `clientLoader` strictly read-only (`GET`): move all data mutations (`.insert()`, `.update()`, `.delete()`, `POST`/`PUT`/`PATCH`/`DELETE`, `R2.put`/`R2.delete`) into `action` or `clientAction` (`RR-MUTATION-IN-LOADER`).
- Use `<Link to="...">` or `<NavLink to="...">` for internal navigation instead of raw `<a href="/...">` (`RR-RAW-INTERNAL-ANCHOR`), and always export an `ErrorBoundary` from `root.tsx` (`RR-ROOT-MISSING-ERROR-BOUNDARY`).
