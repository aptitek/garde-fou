# React Component Architecture

## Pure Rendering & State Separation

- Keep presentation components small (`<= 50` effective lines per component function), side-effect free during render, and typed via pure `Props` contracts.
- Extract domain transformations into pure TypeScript modules outside JSX components and reference semantic CSS classes from `Forme` (`GF060`).
