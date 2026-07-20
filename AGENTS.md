# AGENTS.md

Before meaningful Rust edits, read the [Rust Style Doctrine](/home/main/programming/projects/rust_starter/docs/rust-style-doctrine.md).
It intentionally rejects several pieces of conventional Rust advice; preserve
those inversions instead of translating them back into generic approachable code.

Run `./check.py check` after meaningful local edits. Use `./check.py verify`
when you need a non-mutating CI-style gate.

This crate owns the Dwemer Poolrooms visual language and water physics. Keep
application concepts out of it: consumers describe geometry and forcing; this
crate owns simulation, optics, timing, and GPU representation.

## Physical chrome

Authored widgets are miniature mechanisms submerged in the same world as the
water. Their geometry, constraints, inertia, contact, and displacement must
come from one coherent physical model. Hidden machinery may be omitted and
material response may be artistically compressed, but visible motion must not
contradict the mechanism that would produce it. Reskins of stock controls do
not belong in the crafted-widget menagerie.

All authored hardware comes from one foundry. Screen x-y is the assembly plane,
+y points down-screen, and the viewer lies on +z. Its distant key is fixed in
the y-z plane at 60° above the top-of-screen horizon: L=(0, −½, √3/2). Bronze
palette, specular law, cylindrical stock, stamped facets, black recesses, and
machined rim gauges are shared parts, never widget-local approximations.
