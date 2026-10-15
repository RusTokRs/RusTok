// Marker helper for prose sources.
//
// Verifier marker checks on prose (`implementation-plan-*.md`, `CRATE_API.md`, READMEs) are
// wording contracts. Comparing them against the raw file text makes the guard depend on where
// the author happened to wrap a line, so the same sentence fails the gate after an unrelated
// reflow. `flattenWhitespace` is applied to both the source and the marker, which keeps the
// wording assertion exactly as written while removing the line-break coupling.
//
// Code sources keep exact (unflattened) markers: there the layout is part of the contract.

export const flattenWhitespace = (source) => source.replace(/\s+/g, ' ').trim();
