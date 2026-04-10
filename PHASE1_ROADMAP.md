# Phase 1 — Core Parser and Compiler Skeleton (0.1.x)

Phase 1 delivers a correct, well-tested parser and class resolver that accepts valid Ironclad source and rejects invalid input with precise, actionable error reporting. No backend emission occurs in this phase. The grammar covers the full language: core filesystem primitives, the six compiler-native subsystem domains (storage, filesystem, SELinux, services, firewall, network), users, packages, variables, loops, conditionals, and the single-inheritance class system. The milestone is complete when `ironclad compile example.ic` parses a multi-class system declaration with subsystem blocks, flattens inheritance, and prints the fully resolved system definition with traceable property origins.

---

## 1.1 — Project Scaffold

**Version: 0.1.0-dev**

Set up the Cargo workspace, crate boundaries, and CI. No logic — just the structure everything else builds into.

**Deliverables:**

- Initialize a Cargo workspace at the repository root.
- Create four crates: `ironclad-grammar` (pest grammar and raw parse tree), `ironclad-ast` (AST types and resolution), `ironclad-diagnostics` (error types, source spans, reporting), `ironclad-cli` (binary entry point).
- Add dependencies: `pest` and `pest_derive` in `ironclad-grammar`, `serde` with derive in `ironclad-ast`, `clap` in `ironclad-cli`.
- Add a `.github/workflows/ci.yml` running `cargo build`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt -- --check` on push and PR.
- Add a `tests/fixtures/` directory with a single placeholder `.ic` file.
- Verify CI passes green on an empty test suite.

**Done when:** `cargo build --workspace` succeeds and CI is green.

---

## 1.2 — Grammar: Primitives and Literals

**Version: 0.1.0-dev**

Define the lowest-level grammar rules in the `.pest` file. These are the atoms everything else is composed from.

**Deliverables:**

- Write `ironclad.pest` with rules for: `WHITESPACE`, `COMMENT` (line comments with `//` and block comments with `/* */`), `identifier` (alphanumeric plus underscores, cannot start with a digit), `string_literal` (double-quoted, with escape sequences for `\\`, `\"`, `\n`, `\t`), `integer_literal` (decimal, with optional `0x` hex prefix), `boolean_literal` (`true` / `false`), `path_literal` (string used for filesystem paths — may be a string literal with semantic meaning applied later).
- Write unit tests using pest's `parses_to!` macro confirming each rule accepts valid input and rejects invalid input.
- Test edge cases: empty strings, identifiers that are keywords, hex literals, comments inside expressions.

**Done when:** All primitive rules parse correctly with full test coverage of happy and unhappy paths.

---

## 1.3 — Grammar: Property Assignments and Blocks

**Version: 0.1.0-dev**

Add the structural grammar: key-value assignments, typed blocks, and nesting. This is the skeleton of what an Ironclad file looks like.

**Deliverables:**

- Add grammar rules for: `property_assignment` (`key: value` and `key = value` — decide on one canonical form and document the choice), `block` (`identifier { ... }`), `list_value` (`[ item, item, ... ]`), `enum_value` (bare identifiers used as enumerated choices, e.g. `enforcing`, `ext4`, `s6`).
- Add nesting support: blocks can contain property assignments and other blocks.
- Write pest tests parsing structures like:
  ```
  disk {
    root {
      filesystem: ext4
      size: 20480
      mount: "/"
    }
  }
  ```
- Test: nested blocks, lists of strings, lists of integers, empty blocks, trailing commas in lists (decide and document whether allowed).

**Done when:** Arbitrary nested block structures with property assignments parse correctly.

---

## 1.4 — Grammar: System and Class Declarations

**Version: 0.1.0-dev**

Add top-level declaration syntax: `system` and `class` keywords with inheritance.

**Deliverables:**

- Add grammar rules for: `system_decl` (`system <name> { ... }` and `system <name> extends <parent> { ... }`), `class_decl` (`class <name> { ... }` and `class <name> extends <parent> { ... }`), `source_file` (a sequence of `class_decl` and/or `system_decl` entries, with optional `include` statements).
- Add `include` syntax: `include "path/to/file.ic"` at the top level.
- Write pest tests for: a file with one class, a file with multiple classes and one system, a system extending a class, a class extending another class, include statements.
- Test rejection: system extending a system (if disallowed), missing class name, missing braces, duplicate top-level names (this is a parse-level test; semantic duplicate detection comes later).

**Done when:** A complete multi-declaration `.ic` file parses successfully against the grammar.

---

## 1.5 — Grammar: Domain-Specific Syntax

**Version: 0.1.0-dev**

Finalize grammar for the domain constructs that need special syntax or notation — the parts of the language specific to Linux system administration.

**Deliverables:**

- Add grammar rules for the six compiler-native subsystem domains:
  - **Storage:** `disk`, `mdraid`, `zpool`, `stratis`, `luks2`, `lvm`, filesystem types (`ext4`, `xfs`, `btrfs`, etc.), mount expressions
  - **SELinux:** `selinux` block with mode, policy, users, roles, booleans, context expressions (`user_u:role_r:type_t:s0-s15:c0.c1023`)
  - **Services/Init:** `init systemd` and `init s6` blocks with `service`, `timer`, `socket` declarations
  - **Firewall:** `firewall` block with `table`, `chain`, `rule`, `set`, `map` declarations
  - **Network:** `network` block with `interface`, `bond`, `bridge`, `vlan`, `dns`, `routes` declarations
  - **Users/Packages:** `users` block with `user`, `group`, `policy` declarations; `packages` block with `pkg`, `repo`, `group`, `module` declarations
- Add grammar rules for: size suffixes (`20G`, `512M`, `4K`), network CIDR notation (`192.168.1.0/24`), mode literals (`0644`, `0755`).
- Add `var` declaration syntax: `var name = value` and `var name: type = value`.
- Add conditional syntax: `if expression { ... }`, `elif`, `else`.
- Add loop syntax: `for identifier in expression { ... }` and `range()`.
- Add `constraint` block syntax for compile-time assertions.
- Finalize and freeze the Phase 1 grammar. After this step, the `.pest` file is the canonical language specification for 0.1.x.
- Write a `GRAMMAR.md` documenting every rule with examples.

**Done when:** The grammar is frozen for 0.1.x, documented, and all rules have tests.

---

## 1.6 — AST Data Structures

**Version: 0.1.0-dev**

Define the Rust types that represent parsed Ironclad source. These types are the contract between the parser and every downstream pass.

**Deliverables:**

- In `ironclad-ast`, define: `SourceFile` (list of declarations), `ClassDecl` (name, optional parent name, body), `SystemDecl` (name, optional parent name, body), `Block` (name, list of properties and child blocks), `Property` (key, value, span), `Value` enum (String, Integer, Boolean, List, Enum, SizeValue, ModeValue, SELinuxLabel, CIDRAddress, Path, Reference), `Span` (file path, byte offset start, byte offset end, line, column). Define AST node types for each compiler-native subsystem domain: `StorageDecl`, `SelinuxDecl`, `InitDecl`, `ServiceDecl`, `FirewallDecl`, `NetworkDecl`, `UsersDecl`, `PackagesDecl`. Define `VarDecl`, `ConstraintDecl`, `IfBlock`, `ForBlock`, `FuncDecl`, `EmbedStmt`, `ImportStmt`.
- Derive `Debug`, `Clone`, `PartialEq`, `Serialize` on all types.
- Implement `Display` for `Value` and for the AST as a whole (a human-readable pretty-print of the tree).
- Write unit tests constructing AST nodes by hand and verifying `Display` output matches expected format.

**Done when:** All AST types compile, serialize, and display correctly. No parsing logic yet.

---

## 1.7 — Diagnostic Infrastructure

**Version: 0.1.0-dev**

Build the error reporting system before wiring up the parser, so every error from day one has file, line, column, and a human-readable message.

**Deliverables:**

- In `ironclad-diagnostics`, define: `Diagnostic` struct (severity, message, span, optional help text, optional related spans), `Severity` enum (Error, Warning, Info), `DiagnosticBag` (collects multiple diagnostics during a compilation pass).
- Integrate the `ariadne` or `miette` crate for span-highlighted terminal output (source line printed with underline/caret pointing at the error).
- Write a function that converts a `pest::error::Error` into a `Diagnostic`.
- Write tests verifying that a known bad input produces a diagnostic with the correct span and message.

**Done when:** `DiagnosticBag` can collect errors and render them with source-highlighted output to stderr.

---

## 1.8 — Parser: Pest Pairs to AST

**Version: 0.1.0-dev**

Wire the grammar to the AST. Each pest rule maps to a function that constructs an AST node.

**Deliverables:**

- In `ironclad-grammar`, implement `pub fn parse_source(input: &str, file_path: &Path) -> Result<SourceFile, DiagnosticBag>`.
- Walk pest `Pairs` recursively, constructing `ClassDecl`, `SystemDecl`, `Block`, `Property`, and `Value` nodes. Attach `Span` to every node using pest's span information.
- Handle `include` statements by recording them in the `SourceFile` for the resolver to process (do not resolve includes in the parser — just record the paths).
- Convert all pest errors into `Diagnostic` instances with precise source locations.
- Write integration tests: parse a valid multi-class `.ic` fixture file and assert the resulting AST matches expected structure. Parse an invalid file and assert the diagnostic message and span are correct.

**Done when:** Valid `.ic` files produce a correct AST. Invalid files produce diagnostics with accurate line/column positions.

---

## 1.9 — Include Resolution and Multi-File Loading

**Version: 0.1.0-dev**

Handle `include` statements so a project can be split across files.

**Deliverables:**

- Implement file loading: given a root `.ic` file, parse it, collect `include` paths, resolve them relative to the including file's directory, parse those files, and merge all declarations into a single `SourceFile`.
- Detect and report circular includes as a diagnostic error.
- Detect and report duplicate top-level declaration names across files.
- Write tests with a `tests/fixtures/multi_file/` directory: a root file including two class files, a circular include scenario, a missing include path.

**Done when:** A multi-file Ironclad project loads and merges into a single AST.

---

## 1.10 — Class Resolution: Inheritance Flattening

**Version: 0.1.0-dev**

The core of Phase 1. Take the merged AST with unresolved inheritance and produce a fully flattened system definition.

**Deliverables:**

- Implement the resolution pass in `ironclad-ast`: topologically sort the class hierarchy, detect inheritance cycles (report as diagnostic error), flatten each class by merging parent properties with child overrides.
- Property resolution rules: child properties override parent properties at the same path. Child blocks merge with parent blocks (child adds new properties and overrides existing ones). A child cannot remove a parent property — only override its value.
- Track property origin: every property in the resolved output records which class and source span it came from (directly declared vs. inherited).
- Produce a `ResolvedSystem` type: the final flattened system with no unresolved references, every property carrying its origin.
- Write tests covering: single inheritance, two-level inheritance, property override, block merging, inheritance cycle detection, missing parent class, diamond detection (if disallowed — single inheritance means this shouldn't occur, but test the error if someone tries).

**Done when:** A `system Foo extends Bar` declaration produces a fully flattened `ResolvedSystem` with correct property values and traceable origins.

---

## 1.11 — CLI: End-to-End Pipeline

**Version: 0.1.0**

Wire everything together in the `ironclad-cli` binary. This is the first usable release.

**Deliverables:**

- Implement `ironclad compile <file.ic>` subcommand: loads the file, resolves includes, parses, resolves classes, and prints the `ResolvedSystem` as pretty-printed human-readable text to stdout.
- Add `--format` flag: `text` (default, human-readable), `json` (serde JSON serialization of the resolved AST).
- Diagnostics (errors and warnings) go to stderr with ariadne/miette source highlighting.
- Exit code 0 on success, 1 on error.
- Add `ironclad check <file.ic>` subcommand: same as compile but only reports diagnostics without printing the resolved output (for CI use).
- Write integration tests that invoke the binary via `std::process::Command`, feed it fixture files, and assert stdout/stderr/exit code.

**Done when:** `ironclad compile example.ic` prints a resolved system and `ironclad check broken.ic` reports errors with highlighted source spans.

---

## 1.12 — Error Quality Pass

**Version: 0.1.1**

Go back through every diagnostic and make it genuinely helpful. This is the difference between a tool people curse at and a tool people trust.

**Deliverables:**

- Audit every diagnostic message for clarity. Each error must state what was expected, what was found, and where.
- Add help text to common errors: "Did you mean `extends`?", "Property `X` was already declared in parent class `Y` at file:line", "Class `Z` forms an inheritance cycle: Z → A → B → Z".
- Add related spans where useful: when a property override conflicts, show both the parent declaration and the child override.
- Add warnings for likely mistakes: empty blocks, unused classes (declared but never extended or instantiated), deeply nested inheritance (configurable threshold, default 5).
- Write tests for every new/improved diagnostic message.

**Done when:** A contributor unfamiliar with Ironclad can read any error message and understand what to fix without consulting documentation.

---

## 1.13 — Test Suite and Documentation

**Version: 0.1.2**

Harden the test suite and write the docs that make 0.1.x usable and contributable.

**Deliverables:**

- Organize `tests/fixtures/` into subdirectories: `valid/` (files that must parse and resolve), `invalid/` (files that must produce specific diagnostics), `multi_file/` (include resolution tests).
- Each fixture has a companion `.expected` file (expected resolved JSON output or expected diagnostic text).
- Add a CI step that runs the full fixture suite.
- Write `docs/GRAMMAR.md`: the complete grammar specification with examples for every rule.
- Write `docs/LANGUAGE_GUIDE.md`: a tutorial-style introduction — "your first Ironclad file," property types, blocks, classes, inheritance, includes.
- Update `ROADMAP.md` to mark Phase 1 complete and link to the detailed phase docs.

**Done when:** Test coverage is comprehensive, docs are written, and Phase 1 is tagged as 0.1.2.

---

## Dependency Graph

```
1.1 (scaffold)
 ├── 1.2 (primitives)
 │    └── 1.3 (blocks)
 │         └── 1.4 (class/system decls)
 │              └── 1.5 (domain syntax) ── grammar frozen
 ├── 1.6 (AST types)
 ├── 1.7 (diagnostics)
 │
 └── 1.8 (parser: pest → AST)  ← requires 1.2–1.7
      └── 1.9 (includes)
           └── 1.10 (class resolution)
                └── 1.11 (CLI)  ← 0.1.0 release
                     ├── 1.12 (error quality)
                     └── 1.13 (tests + docs) ← 0.1.2 release
```

Steps 1.2–1.5 (grammar), 1.6 (AST types), and 1.7 (diagnostics) can progress in parallel once the scaffold is in place. Everything converges at 1.8 when the parser wires grammar to AST.
