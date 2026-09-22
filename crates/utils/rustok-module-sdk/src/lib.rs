//! Generated Rust guest bindings for the canonical RusToK module Component
//! Model contract.

/// Independently released author SDK identity recorded in build provenance.
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Independently deployed WIT package identity.
pub const WIT_PACKAGE: &str = "rustok:module@1.0.0";

/// Current guest world selected by the module build contract.
pub const WIT_WORLD: &str = "module-runtime";

/// Canonical WIT source packaged with the author SDK.
pub const WIT_SOURCE: &str = include_str!("../wit/module-runtime.wit");

pub mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "module-runtime",
        pub_export_macro: true,
    });
}

pub use bindings::{Guest, rustok};

/// Export a `Guest` implementation through the canonical component world.
#[macro_export]
macro_rules! export {
    ($component:ident) => {
        $crate::bindings::export!($component with_types_in $crate::bindings);
    };
}

#[cfg(test)]
mod tests {
    use super::{WIT_PACKAGE, WIT_SOURCE, WIT_WORLD};

    #[test]
    fn packaged_wit_has_the_frozen_identity_and_surface() {
        assert!(WIT_SOURCE.contains(&format!("package {WIT_PACKAGE};")));
        assert!(WIT_SOURCE.contains(&format!("world {WIT_WORLD}")));
        assert!(WIT_SOURCE.contains("import host;"));
        assert!(WIT_SOURCE.contains("export run:"));
    }
}
