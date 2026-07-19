# muskitty-selectors

[![crates.io](https://img.shields.io/crates/v/muskitty-selectors.svg)](https://crates.io/crates/muskitty-selectors)
[![Documentation](https://docs.rs/muskitty-selectors/badge.svg)](https://docs.rs/muskitty-selectors)
[![License](https://img.shields.io/crates/l/muskitty-selectors.svg)](https://github.com/muskitty-dev/muskitty-selectors/blob/main/LICENSE)

A from-scratch Selectors Level 4 parser and matching engine written in
pure Rust, implementing [Selectors Level 4](https://drafts.csswg.org/selectors-4/)
on top of [`muskitty-css-parser`](https://crates.io/crates/muskitty-css-parser).

Part of the [MusKitty](https://github.com/muskitty-dev) browser engine project.

## Status

| Component | Spec Coverage | Tests |
|-----------|---------------|-------|
| §3 Data Model | L716-1357 | 6 |
| §5 Elemental selectors | L1805-1995 | 10 |
| §6 Attribute selectors | L1996-2533 | 11 |
| §4 Logical combinations | L1358-1804 | 10 |
| §13 Tree-structural pseudo-classes | L3792-4359 | 12 |
| §15 Combinators | L4360-4532 | 12 |
| §17 Specificity | L4534-4633 | 22 |
| §18 Matching engine | L4816-5026 | 19+ |
| **Total** | §3 / §4 / §5 / §6 / §13 / §14 / §15 / §17 / §18 | **145** |

- Zero `unsafe` code
- Zero C/C++ dependencies
- Runtime dependency: `muskitty-css-parser` (which re-exports `muskitty-css-tokenizer`)
- Rust stable toolchain only
- MSRV 1.82

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
muskitty-selectors = "0.1.0"
```

Or run:

```bash
cargo add muskitty-selectors
```

## Quick Start

```rust
use muskitty_selectors::{parse_a_selector, matches, Specificity};

let list = parse_a_selector("div.foo > span").unwrap();
let spec: Specificity = list.specificity_max();
// (0, 1, 2) — one class + two type selectors.
```

To match against your own element tree, implement the
`muskitty_selectors::Element` trait:

```rust
use muskitty_selectors::{parse_a_selector, matches, Element};

#[derive(Clone)]
struct MyElement { /* ... */ }

impl Element for MyElement {
    fn local_name(&self) -> String { /* ... */ }
    // ... 13 other trait methods
}

let list = parse_a_selector("a:hover").unwrap();
let el = MyElement { /* ... */ };
if matches(&list, &el) {
    // ...
}
```

## Architecture

```
muskitty-selectors/
  src/
    types.rs              §3 data model: SelectorList, ComplexSelector,
                          CompoundSelector, SubclassSelector, PseudoClass,
                          PseudoElement, AnPlusB, Combinator
    error.rs              SelectorParseError
    specificity.rs        §17 A/B/C triplet computation
    parser/               §3 grammar productions
      mod.rs              parse_a_selector / parse_a_relative_selector entry points
      simple.rs           §5 / §6.5 / §6.6 type / universal / class / id
      attribute.rs        §6 attribute selectors (presence / exact / ~= / |= / ^= / $= / *=)
      compound.rs         §3 compound-selector production
      complex.rs          §3 complex-selector production + §15 combinators
      list.rs             §3 selector-list production (comma-separated)
      an_plus_b.rs        §13.5 An+B notation
      relative.rs         §4.5 relative selectors for :has()
    matching/            §18 matching engine
      mod.rs              Element trait + matches / query_selector / query_selector_all
                          + right-to-left complex-selector walk (§18 L4902-4919)
      simple_matcher.rs   §5 / §6 simple-selector matching
      pseudo_matcher.rs   §13 tree-structural + An+B + §4 logical combinations
      dom_impl.rs         Reference impl for muskitty_dom::Node (dev-only)
  tests/
    11 test files, 145 tests total
```

### Spec Coverage

**Parser** — consumes a token stream produced by `muskitty_css_parser::tokenize`
and builds selector data structures. No DOM dependency.

- §3 Data Model and Parsing
- §4 Logical Combinations (`:is()` / `:where()` / `:not()` / `:has()`)
- §5 Elemental Selectors (type / universal)
- §6 Attribute Selectors (all 7 syntaxes + modifier)
- §6.5 Class selectors, §6.6 ID selectors
- §13 Tree-Structural Pseudo-classes (`:root` / `:empty` / `:first-child` /
  `:nth-child(An+B [of S]?)` / etc.)
- §15 Combinators (Descendant / Child / NextSibling / SubsequentSibling)
- §3 Compound and Complex selectors

**Specificity** — computes the A/B/C triplet per §17.

- `:is()` / `:not()` / `:has()` take the maximum specificity of their arguments
- `:where()` always contributes 0
- `:nth-child(An+B of S)` adds the specificity of `S`

**Matching** — matches parsed selectors against an element tree via the
`Element` trait. The engine walks complex selectors right-to-left per
§18 L4902-4919.

- §18 `matches(selector, element)` — single-element match test
- §18 `query_selector(root, selector)` — first match in tree order
- §18 `query_selector_all(root, selector)` — all matches in tree order

### Deferred

- `:has()` multi-compound relative selectors (`:has(.a > .b)`) — SP-8
  supports single-compound only; multi-compound returns `false`.
- Strict namespace matching (`ns|tag`) — currently treated as
  "any namespace".
- §7-§12 UI / location / linguistic / resource / display / input
  pseudo-classes — parsing supported, matching stub returns `false`.
- WPT subset integration.

## Building

```bash
cargo check
cargo build
```

## Testing

```bash
# All 145 tests
cargo test
```

## Design Principles

1. **CSSWG is ground truth** — Implementation follows the spec exactly.
2. **Spec-compliant, not test-compliant** — Tests verify the code; code is
   never modified to pass a test unless the spec proves the test is wrong.
3. **Right-to-left matching** — Complex selectors are walked right-to-left
   per §18 L4902-4919 (subject first, then ancestors/siblings).
4. **Zero unsafe** — Pure safe Rust.

## Spec Reference

This implementation references:

- [Selectors Level 4](https://drafts.csswg.org/selectors-4/) — Primary authority
  - §3: Data Model and Selectors Parsing
  - §4: Logical Combinations
  - §5: Elemental Selectors
  - §6: Attribute Selectors
  - §13: Tree-Structural Pseudo-classes
  - §15: Combinators
  - §17: Specificity
  - §18: API Hooks (matching engine)

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

Copyright 2026 MusCat / MusKitty Bit-Torch Community
