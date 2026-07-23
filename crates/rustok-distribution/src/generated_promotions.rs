// This baseline lists reviewed first-party modules promoted into every compiled
// distribution. CI may replace the file inside an immutable build workspace
// when additional reviewed static promotions are selected.

use rustok_core::ModuleRegistry;
use rustok_social_graph::SocialGraphModule;

pub(crate) fn register_promoted_modules(registry: ModuleRegistry) -> ModuleRegistry {
    registry.register(SocialGraphModule)
}
