# Rust module author SDK

## Canonical Contract

`wit/module-runtime.wit` is the sole source of truth for the current Component
Model boundary. Its independently deployed package version is an external ABI
identity, not an internal implementation family. The repository keeps one
current Rust model and does not maintain parallel guest contracts.

The world imports one function:

```wit
invoke: func(
    capability: string,
    operation: string,
    input: string,
) -> result<string, string>;
```

The world exports `run(input: string) -> result<string, string>`. Both input and
output strings carry the canonical JSON binding envelope admitted by the module
control plane. A guest receives no WASI or ambient platform imports.

## Rust Usage

```rust
struct Module;

impl rustok_module_sdk::Guest for Module {
    fn run(input: String) -> Result<String, String> {
        rustok_module_sdk::rustok::module::host::invoke(
            "platform.events",
            "publish",
            &input,
        )?;
        Ok(input)
    }
}

rustok_module_sdk::export!(Module);
```

The build worker compiles guest crates with the pinned component toolchain and
then derives the WIT surface from the produced component. Admission rejects an
artifact whose imports, exports, package, world, or package version differs
from the immutable build request.
