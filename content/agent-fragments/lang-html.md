# W3C HTML5 & Semantic Document Standards (`GF066`, `GF080`)

## Content Models, Landmarks & Accessibility

- Declare a non-empty `lang` attribute on root `<html>`, maintain a single non-hidden `<main>` landmark, and keep static `id="..."` attributes unique per document (`W3C-LANDMARK-AND-ID-UNIQUENESS`).
- Never nest interactive controls (`<button>`, `<a href>`, `<input>`, `<select>`, `<textarea>`, `<details>`, `<iframe>`) inside `<button>` or `<a>`, nor `<form>` inside `<form>` (`W3C-INTERACTIVE-NESTING`), never pass children to HTML5 void elements (`<img>`, `<input>`, `<br>`, `<hr>`, `<meta>`, `<link>`), and never place block elements inside `<p>` or `<span>` (`W3C-VOID-ELEMENT-CHILDREN`, `W3C-CONTENT-MODEL`).
