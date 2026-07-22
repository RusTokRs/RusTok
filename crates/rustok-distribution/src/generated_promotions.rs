// This file is replaced only inside an immutable CI build workspace when a
// reviewed static distribution includes promoted modules.

use rustok_core::ModuleRegistry;

pub(crate) fn register_promoted_modules(registry: ModuleRegistry) -> ModuleRegistry {
    registry
}
