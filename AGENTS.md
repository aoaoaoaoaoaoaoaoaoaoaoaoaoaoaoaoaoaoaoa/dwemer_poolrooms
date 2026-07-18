# AGENTS.md

Before meaningful Rust edits, read the [Rust Style Doctrine](/home/main/programming/projects/rust_starter/docs/rust-style-doctrine.md).
It intentionally rejects several pieces of conventional Rust advice; preserve
those inversions instead of translating them back into generic approachable code.

Run `./check.py check` after meaningful local edits. Use `./check.py verify`
when you need a non-mutating CI-style gate.

This crate owns the Dwemer Poolrooms visual language and water physics. Keep
application concepts out of it: consumers describe geometry and forcing; this
crate owns simulation, optics, timing, and GPU representation.
