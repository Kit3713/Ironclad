# Contributing to Ironclad

Ironclad is pre-alpha. The language, compiler architecture, and internal APIs are all subject to change. Contributions are welcome, but please coordinate before investing significant effort.

## Before You Start

1. **Check the roadmap.** Read [ROADMAP.md](ROADMAP.md) and the relevant phase doc ([PHASE1_ROADMAP.md](PHASE1_ROADMAP.md), [PHASE2_ROADMAP.md](PHASE2_ROADMAP.md)). If your idea is already planned, reference the task number in your PR.

2. **Open an issue first** for anything non-trivial. Describe what you want to change and why. This avoids wasted work on changes that conflict with the project direction.

3. **Small PRs are better.** One logical change per PR. If a feature touches the grammar, parser, AST, and validator, that's fine in one PR — but don't bundle unrelated changes.

## Development Setup

```bash
# Clone and build
git clone https://github.com/Kit3713/Ironclad.git
cd Ironclad
cargo build --workspace

# Run all checks (these must pass before submitting a PR)
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

### Project Structure

```
src/
  storage.pest    # PEG grammar (pest)
  parser.rs       # Grammar -> AST
  ast.rs          # Type definitions
  validate.rs     # Semantic validation
  tests.rs        # Test suite
  main.rs         # CLI entry point
crates/
  ironclad-diagnostics/   # Error reporting infrastructure
examples/                 # Example .icl files
SYNTAX-SPEC/              # Language syntax specifications
```

## What To Work On

Good first contributions:

- **New example files** in `examples/` that exercise edge cases or demonstrate patterns from the syntax specs.
- **Additional validation rules** in `validate.rs` — check the syntax specs for constraints that aren't yet enforced.
- **Test coverage** — read through `validate.rs` and write tests for validation paths that aren't covered.
- **Grammar edge cases** — find inputs that should parse but don't, or inputs that parse but shouldn't.

Larger contributions (open an issue first):

- New domain parsers (network, users, packages, firewall — see `SYNTAX-SPEC/`).
- Crate restructuring per Phase 1.1.
- Class/system declaration syntax (Phase 1.4).

## Code Standards

- **All checks must pass:** `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`.
- **Tests required** for new functionality. Parser changes need both valid-input and invalid-input tests.
- **No unnecessary dependencies.** Justify new crate additions in the PR description.
- **Error messages matter.** Every diagnostic should state what was expected, what was found, and where. Include a hint when the fix is non-obvious.
- **Follow existing patterns.** Look at how similar functionality is already implemented before adding something new.

## Commit Messages

- Use imperative mood: "Add validation for thin pool overcommit", not "Added" or "Adds".
- Keep the subject line under 72 characters.
- Reference roadmap tasks when applicable: "Implement Phase 1.3 block grammar".

## Syntax Specification Changes

The files in `SYNTAX-SPEC/` define what the language should look like. Changes to these are design decisions, not just code changes. Always open an issue to discuss syntax changes before implementing them.

## Security

If you discover a security issue (the compiler producing weaker configurations than declared), see [SECURITY.md](SECURITY.md). Do not open a public issue.
