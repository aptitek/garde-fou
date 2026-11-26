//! Pure `#![no_std]` leaf sum types for Astro, React Router v7, Panda CSS,
//! Material Design 3 Expressive (`M3E`), W3C HTML5 + WCAG 2.2 AA, Cloudflare Workers/D1/R2,
//! and Drizzle ORM violations (`GF080`).

use serde::{Deserialize, Serialize};

/// Astro (`.astro` & Content Collections) violations (`GF080` / `GF063`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum AstroViolation {
    /// `set:html={...}` unescaped HTML injection directive.
    NoSetHtml,
    /// Browser global (`window`, `document`, `localStorage`, `navigator`) or React Hook in server frontmatter `---`.
    FrontmatterClientApi,
    /// Non-serializable function or class instance passed to a hydrated `client:*` island component.
    NonSerializableIslandProp,
    /// Unjustified eager `client:load` / `client:only` hydration or `<script is:inline>`.
    ClientLoadBudget,
}

impl AstroViolation {
    /// Canonical `ASTRO-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoSetHtml => "ASTRO-NO-SET-HTML",
            Self::FrontmatterClientApi => "ASTRO-FRONTMATTER-CLIENT-API",
            Self::NonSerializableIslandProp => "ASTRO-NON-SERIALIZABLE-ISLAND-PROP",
            Self::ClientLoadBudget => "ASTRO-CLIENT-LOAD-BUDGET",
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::NoSetHtml => {
                "Render escaped Astro expressions `{expr}` or Markdown Content Collections instead of `set:html`."
            }
            Self::FrontmatterClientApi => {
                "Move browser APIs (`window`, `document`, `localStorage`) and React Hooks out of server `---` frontmatter into client islands or `<script>` tags."
            }
            Self::NonSerializableIslandProp => {
                "Pass plain JSON-serializable data props to `client:*` hydrated islands instead of functions or class instances."
            }
            Self::ClientLoadBudget => {
                "Prefer `client:visible` or `client:idle` over eager `client:load`/`client:only` and bundled `<script>` tags over `<script is:inline>`."
            }
        }
    }
}

/// React Router v7 Data & Framework mode violations (`GF080` / `GF072`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ReactRouterViolation {
    /// `*.server.*` module, `drizzle-orm/*`, or server secret leaked to client code, `clientLoader`, `clientAction`, or JSX.
    ServerLeakToClient,
    /// Data mutation (`.insert`, `.update`, `.delete`, `POST`/`PUT`/`PATCH`/`DELETE`, `R2.put`) inside `loader` or `clientLoader`.
    MutationInLoader,
    /// Raw internal `<a href="/...">` anchor instead of `<Link to="...">` or `<NavLink to="...">`.
    RawInternalAnchor,
    /// `root.tsx` route module missing an exported `ErrorBoundary`.
    RootMissingErrorBoundary,
}

impl ReactRouterViolation {
    /// Canonical `RR-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ServerLeakToClient => "RR-SERVER-LEAK-TO-CLIENT",
            Self::MutationInLoader => "RR-MUTATION-IN-LOADER",
            Self::RawInternalAnchor => "RR-RAW-INTERNAL-ANCHOR",
            Self::RootMissingErrorBoundary => "RR-ROOT-MISSING-ERROR-BOUNDARY",
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::ServerLeakToClient => {
                "Confine `*.server.*` and `drizzle-orm` imports to server `loader` and `action` functions; never reference them in `clientLoader`, `clientAction`, `*.client.*`, or JSX."
            }
            Self::MutationInLoader => {
                "Keep `loader` and `clientLoader` read-only; move all database/storage mutations into `action` or `clientAction`."
            }
            Self::RawInternalAnchor => {
                "Replace internal `<a href=\"/...\">` anchors with React Router `<Link to=\"/...\">` or `<NavLink to=\"/...\">`."
            }
            Self::RootMissingErrorBoundary => {
                "Export an `ErrorBoundary` component from `root.tsx` to handle uncaught route errors gracefully."
            }
        }
    }
}

/// Panda CSS build-time token discipline violations (`GF080` / `GF060` / `GF061`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PandaViolation {
    /// Arbitrary `[...]` value, `!important`, raw color/unit, or primitive palette token (`blue.500`) in `Code`.
    ArbitraryOrPrimitive,
    /// Committed generated file inside `styled-system/`.
    CommittedStyledSystem,
}

impl PandaViolation {
    /// Canonical `PANDA-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ArbitraryOrPrimitive => "PANDA-ARBITRARY-OR-PRIMITIVE",
            Self::CommittedStyledSystem => "PANDA-GENERATED-COMMITTED",
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::ArbitraryOrPrimitive => {
                "Replace arbitrary `[...]` escapes, `!important`, raw units/colors, and primitive palette scales (`blue.500`) with semantic Panda CSS tokens."
            }
            Self::CommittedStyledSystem => {
                "Remove generated `styled-system/` files from Git tracking and keep `styled-system/` in `.gitignore`."
            }
        }
    }
}

/// Material Design 3 Expressive (`M3E`) violations (`GF080` / `GF061` / `GF066`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum M3eViolation {
    /// Mismatched container surface and `on*` content ink (`bg: 'primary'` with `color: 'onSurface'`).
    TonalPairing,
    /// `Spatial` overshoot spring applied to opacity/color, or raw `cubic-bezier`/`ms` outside token files.
    SpringPhysicsMismatch,
    /// Raw `boxShadow`, `borderRadius`, or `fontSize`/`fontWeight` outside MD3 elevation/shape/type roles.
    RawElevationShapeTypo,
    /// Arbitrary hover opacity replacing MD3 state layers, or interactive icon target `< 48dp` (`< 24px` AA).
    TouchTargetAndStateLayer,
}

impl M3eViolation {
    /// Canonical `M3E-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TonalPairing => "M3E-TONAL-PAIRING",
            Self::SpringPhysicsMismatch => "M3E-SPRING-PHYSICS-MISMATCH",
            Self::RawElevationShapeTypo => "M3E-RAW-ELEVATION-SHAPE-TYPO",
            Self::TouchTargetAndStateLayer => "M3E-TOUCH-TARGET-AND-STATE-LAYER",
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::TonalPairing => {
                "Pair every MD3 surface token with its matching `on*` token (`primary` <-> `onPrimary`, `primaryContainer` <-> `onPrimaryContainer`, `errorContainer` <-> `onErrorContainer`)."
            }
            Self::SpringPhysicsMismatch => {
                "Use `Effects` springs (`*Effects*` / `--motion-spring-*-effects`) for `opacity`/`color`/`bg` and `Spatial` springs (`*Spatial*`) only for geometry/transform."
            }
            Self::RawElevationShapeTypo => {
                "Use MD3 `elevation.level0..5`, shape scale (`none`..`extraExtraLarge`, `full`), and typography roles (`display`/`headline`/`title`/`body`/`label`)."
            }
            Self::TouchTargetAndStateLayer => {
                "Use MD3 state-layer tokens instead of arbitrary `_hover: { opacity }` and keep interactive targets `>= 48px` (`48dp`)."
            }
        }
    }
}

/// WHATWG / W3C HTML5 structural content-model violations (`GF066` / `GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum W3cViolation {
    /// Interactive control (`<button>`, `<a href>`, `<input>`) nested inside `<button>`/`<a>`, or `<form>` in `<form>`.
    InteractiveNesting,
    /// Child nodes passed to an HTML5 void element (`<img>`, `<input>`, `<br>`, `<hr>`, `<meta>`, `<link>`).
    VoidElementChildren,
    /// Block element inside `<p>`/`<span>`, invalid direct child of `<ul>`/`<ol>`/`<tr>`, or obsolete tag (`<center>`, `<font>`, `<marquee>`).
    ContentModel,
    /// Duplicate static `id="..."`, multiple visible `<main>` landmarks, or `<html>` without non-empty `lang`.
    LandmarkAndIdUniqueness,
}

#[rustfmt::skip]
impl W3cViolation {
    /// Canonical `W3C-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InteractiveNesting => "W3C-INTERACTIVE-NESTING",
            Self::VoidElementChildren => "W3C-VOID-ELEMENT-CHILDREN",
            Self::ContentModel => "W3C-CONTENT-MODEL",
            Self::LandmarkAndIdUniqueness => "W3C-LANDMARK-AND-ID-UNIQUENESS",
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::InteractiveNesting => "Never nest interactive controls (`<button>`, `<a href>`, `<input>`, `<select>`) inside `<button>` or `<a>`, nor `<form>` inside `<form>`.",
            Self::VoidElementChildren => "Remove children and closing tags from HTML5 void elements (`<img>`, `<input>`, `<br>`, `<hr>`, `<meta>`, `<link>`, `<source>`, `<track>`, `<wbr>`).",
            Self::ContentModel => "Place block elements outside `<p>`/`<span>`, keep `<li>` as direct children of `<ul>`/`<ol>` and `<td>`/`<th>` in `<tr>`, and remove obsolete HTML tags.",
            Self::LandmarkAndIdUniqueness => "Keep static `id` attributes unique, render at most one non-hidden `<main>` landmark, and declare a non-empty `lang` on `<html>`.",
        }
    }
}

/// WAI-ARIA 1.2 & WCAG 2.2 AA/AAA accessibility violations (`GF066` / `GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum A11yViolation {
    /// Unknown/misspelled `role` or `aria-*` attribute, invalid value type, missing required role property, or redundant role.
    AriaInvalid,
    /// `aria-hidden="true"` on a focusable element without `disabled` or `tabIndex={-1}`.
    HiddenFocusable,
    /// Positive `tabIndex > 0`, `autoFocus`, `onMouseOver` without `onFocus`, or `outline: none` without `:focus-visible`.
    TabindexAndFocusRing,
    /// Unlabeled form input, `<video>`/`<audio>` without `<track kind="captions">`, or spatial animation without `prefers-reduced-motion`.
    MediaFormMotion,
}

#[rustfmt::skip]
impl A11yViolation {
    /// Canonical `A11Y-*` rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AriaInvalid => "A11Y-ARIA-INVALID",
            Self::HiddenFocusable => "A11Y-HIDDEN-FOCUSABLE",
            Self::TabindexAndFocusRing => "A11Y-TABINDEX-AND-FOCUS-RING",
            Self::MediaFormMotion => "A11Y-MEDIA-FORM-MOTION",
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::AriaInvalid => "Use valid WAI-ARIA 1.2 roles and `aria-*` attributes with well-typed values, provide required role states (`aria-checked`, `aria-valuenow`), and omit redundant roles.",
            Self::HiddenFocusable => "Never set `aria-hidden=\"true\"` on focusable controls unless disabled or paired with `tabIndex={-1}`.",
            Self::TabindexAndFocusRing => "Use `tabIndex={0}` or `-1` (never `> 0`), avoid `autoFocus`, pair hover handlers with focus handlers, and preserve `:focus-visible` outlines.",
            Self::MediaFormMotion => "Associate every form control with a `<label>` or `aria-label`, provide `<track kind=\"captions\">` on media, and guard animations with `@media (prefers-reduced-motion: reduce)`.",
        }
    }
}
