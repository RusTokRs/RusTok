import test from "node:test";
import assert from "node:assert/strict";
import { requirePortSurface } from "./rust-public-surface.mjs";

const registry = { ports: [{ name: "ReadPort" }, { name: "WritePort" }] };

test("an inline admitted port facade publishes the same explicit root API", () => {
  const source = `
    mod private_implementation;
    pub mod ports {
      pub use crate::private_implementation::{ReadPort, WritePort};
    }
    pub use ports::{ReadPort, WritePort};
  `;
  assert.deepEqual(requirePortSurface(source, registry), { ok: true });
});

test("inline facade acceptance does not allow a missing root port export", () => {
  const source = `
    pub mod ports { pub struct ReadPort; pub struct WritePort; }
    pub use ports::{ReadPort};
  `;
  const result = requirePortSurface(source, registry);
  assert.equal(result.ok, false);
  assert.match(result.message, /WritePort/);
});

test("a private module is not silently advertised as the declared public port facade", () => {
  const source = "mod ports; pub use ports::{ReadPort, WritePort};";
  assert.equal(requirePortSurface(source, registry).ok, false);
});
