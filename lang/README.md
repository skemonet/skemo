# skemo-lang — the skemo language

Rust library that **reads, checks, formats and edits** `.skemo` diagram files, and resolves their CSS styles. It
is the "language" part of [skemo](https://github.com/skemonet/skemo): it draws nothing, it understands the text.

A diagram is a **Pug** file restricted to four tags (`diagram`, `card`, `zone`, `to`), typed by CSS classes:

```pug
diagram(dir="down")
  card#fw.t0 Firewall
    li 10.0.0.1/24
    to(core).wan transit
  zone#lan.lan Internal LAN
    card#core.t1 Core switch
```

## What the library provides

- A tolerant **parser** with diagnostics (line, column, hint; in English or French, following the language chosen by the program, D-038) that never panics, even on
  garbled text (tested on 4 million corrupted sources).
- A **model**: cards, zones, arrows, animations (markers `@2`, `@3-5:focus`), dates for the `timeline` and `frise`
  modes.
- **Surgical editing**: change one attribute (`move_card`, `set_via`, `set_side`, `add_class`…) rewriting only
  what changes, comments and layout preserved. This is what lets a graphical editor stay faithful to the text.
- **Canonical formatting** (`fmt`).
- **Styles**: a minimal CSS engine written for the project (`var()`, `color-mix()`,
  `@media (prefers-color-scheme)`, `!important`, common selectors) and the **default theme** (light, dark,
  print).

## Usage

```toml
# Cargo.toml: for now as a path or git dependency (not yet published on crates.io)
skemo-lang = { git = "https://github.com/skemonet/skemo" }
```

```rust
let src = "card#a Start\n  to(b) next\ncard#b End\n";

// read and check
let doc = skemo_lang::parse(src);
assert!(!doc.has_errors());
assert_eq!(doc.model.edges[0].id, "a->b");
for d in doc.diagnostics.iter() {
    eprint!("{}", skemo_lang::diag::render(src, "schema.skemo", d));
}

// format
let tidy = skemo_lang::fmt::format(src).unwrap();

// edit: pin card "a" at (120, 40); the rest of the text is untouched
let edited = skemo_lang::edit::move_card(src, "a", 120.0, 40.0).unwrap();
assert!(edited.contains(r#"at="120,40""#));
```

**WebAssembly**-compatible, no system dependency. The format contract is [`FORMAT.md`](FORMAT.md): any change
to the parser starts with an update of that document. The user documentation (docs.skemo.net) is built from
[`site/`](site/README.md).

## Repository contents

| Path | Role |
|---|---|
| `FORMAT.md` | **the format specification** (syntax, attributes, modes, animation, style) |
| `src/` | parser (`parse`, `syntax`), `model`, `edit`, `fmt`, `anim`, `dates`, `style/` (CSS) |
| `default.reference.css`, `theme/` | default theme |
| `site/` | the documentation site (docs.skemo.net): pages, examples, builder |
| `fixtures/` | test data (reference models from the prototype, sources with errors) |

## Development

```bash
../tools/ci.sh         # quality + security of the whole repository (see CONTRIBUTING.md)
cargo test --release   # parity with the reference models, diagnostics as snapshots (insta)
```

See also `SPEC.md` (requirements) and the [`skemo`](https://github.com/skemonet/skemo) repository for the big
picture. License: MIT OR Apache-2.0 (`LICENSE-MIT` and `LICENSE-APACHE` at the root of the repository).
