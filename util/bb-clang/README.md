# bb-clang

> Structured representations of C/C++ entities, powered by libclang.

`bb-clang` is the initial crate that this project started with, in the hopes of me becoming more familiar with Clang and its concepts.

Instead of working over something like clang bindings to Rust, we use [`clang-rs`](https://github.com/KyleMayes/clang-rs) by KyleMayes to do our bidding.

---

## Design

`bb-clang` is just a sort-of frontend to specific kinds of entities you will encounter in the AST.

The purpose is to take in the entity object, and lift it to a structured representation of itself. Some of them would be:

- **[`Struct`](./src/struct_.rs)** -- A structured representation of C/C++ `struct` (or `class`) declarations. They most often contain [`Field`](./src/field.rs)s.

- **[`Field`](./src/field.rs)** -- A structured representation of C/C++ field declarations. They are always the semantic children of [`Struct`](./src/struct_.rs)s and their declaration's underlying type might be used to obtain a new [`Struct`](./src/struct_.rs) as well.

- **[`Enum`](./src/enum_.rs)** -- A structured representation of C/C++ enum declarations. They most often contain [`Constant`](./src/constant/mod.rs) of the specific enum constant declaration kind, and they have an associated type.

- **[`Constant`](./src/constant/mod.rs)** -- A generic representation of all constants (variable declarations that evaluate at compile-time; enum constant declarations; simple, non-builtin macro definitions). We only support (and expose) numeric ones.
  We evaluate the value of the macro after getting the tokens that make it up from Clang, and turning them into [`cexpr`](https://crates.io/crates/cexpr) tokens to evaluate them as a macro definition.
  Moreover, we also support decomposing more complex macros into body tokens that will later be used in conjunction with [`cexpr`](https://crates.io/crates/cexpr) to parse the individual tokens that make it up, to understand how the value came to be.

> **Note** — This is always subject to change, please make sure that you create your own understanding by reading the source code.

---

We also offer:

- **[`SourceLocation`](./src/location.rs)** -- A simple abstraction over source locations.

- **[`Traits`](./src/traits.rs)** -- Some extensions over clang-rs that made the experience personally better for me, but they may be a bit opinionated.

---

## Extras

### Serialization

Almost every type exposed is serializable, for the purpose of turning the information into easily distributable data.

A core belief of `bb` is that it is meant to help others not only view the data, but interact with it, and extract it. You may build your own tooling from joining these utilities together through the frontend CLIs (with the `--json` flag.)

### Pretty displays

We implement pretty display helpers for most of the entities here. They are later used in the CLI tooling.
