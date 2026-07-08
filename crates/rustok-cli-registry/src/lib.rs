use rustok_cli_core::{CommandDescriptor, CommandProvider};

mod generated;

#[derive(Default)]
pub struct SelectedDistributionRegistry {
    providers: Vec<Box<dyn CommandProvider>>,
}

impl SelectedDistributionRegistry {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_providers(providers: Vec<Box<dyn CommandProvider>>) -> Self {
        Self { providers }
    }

    pub fn providers(&self) -> Vec<&dyn CommandProvider> {
        self.providers
            .iter()
            .map(|provider| provider.as_ref())
            .collect()
    }

    pub fn commands(&self) -> Vec<CommandDescriptor> {
        let mut commands = self
            .providers
            .iter()
            .flat_map(|provider| provider.commands())
            .collect::<Vec<_>>();
        commands.sort_by(|left, right| {
            left.namespace
                .cmp(&right.namespace)
                .then(left.name.cmp(&right.name))
        });
        commands
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

pub fn selected_distribution_registry() -> SelectedDistributionRegistry {
    SelectedDistributionRegistry::from_providers(generated::generated_providers())
}

#[cfg(test)]
mod tests {
    use super::{selected_distribution_registry, SelectedDistributionRegistry};
    use rustok_cli_core::{CommandDescriptor, CommandProvider};

    struct ModuleProvider;

    impl CommandProvider for ModuleProvider {
        fn commands(&self) -> Vec<CommandDescriptor> {
            vec![CommandDescriptor::new(
                "module",
                "inspect",
                "Inspect module state",
            )]
        }
    }

    #[test]
    fn selected_distribution_includes_platform_provider() {
        let registry = selected_distribution_registry();

        assert!(!registry.is_empty());
        assert_eq!(registry.providers().len(), 1);
        assert!(registry
            .commands()
            .iter()
            .any(|command| command.namespace == "core" && command.name == "version"));
    }

    #[test]
    fn registry_exposes_provider_references() {
        let registry = SelectedDistributionRegistry::from_providers(vec![Box::new(ModuleProvider)]);

        assert_eq!(registry.providers().len(), 1);
        assert_eq!(registry.commands()[0].namespace, "module");
        assert_eq!(registry.commands()[0].name, "inspect");
    }
}
