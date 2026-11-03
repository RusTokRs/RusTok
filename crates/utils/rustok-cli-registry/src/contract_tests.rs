use rustok_cli_core::{CommandDescriptor, CommandProvider};
use rustok_runtime::RuntimeComposition;

use crate::{selected_distribution_registry, SelectedDistributionRegistry};

struct TestProvider {
    namespace: &'static str,
    name: &'static str,
}

#[async_trait::async_trait]
impl CommandProvider for TestProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor::new(
            self.namespace,
            self.name,
            "Test cmd",
        )]
    }
}

#[test]
fn test_empty_registry_contract() {
    let registry = SelectedDistributionRegistry::empty();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
    assert_eq!(registry.providers().len(), 0);
    assert!(registry.commands().is_empty());

    let debug_repr = format!("{registry:?}");
    assert!(debug_repr.contains("SelectedDistributionRegistry"));
    assert!(debug_repr.contains("providers_len: 0"));
}

#[test]
fn test_commands_lexicographical_sorting_contract() {
    let p1 = Box::new(TestProvider {
        namespace: "zebra",
        name: "b",
    });
    let p2 = Box::new(TestProvider {
        namespace: "apple",
        name: "z",
    });
    let p3 = Box::new(TestProvider {
        namespace: "apple",
        name: "a",
    });

    let registry = SelectedDistributionRegistry::from_providers(vec![p1, p2, p3]);
    assert_eq!(registry.len(), 3);
    assert!(!registry.is_empty());

    let cmds = registry.commands();
    assert_eq!(cmds.len(), 3);
    assert_eq!(cmds[0].namespace, "apple");
    assert_eq!(cmds[0].name, "a");
    assert_eq!(cmds[1].namespace, "apple");
    assert_eq!(cmds[1].name, "z");
    assert_eq!(cmds[2].namespace, "zebra");
    assert_eq!(cmds[2].name, "b");
}

#[test]
fn test_selected_distribution_registry_integration() {
    let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
    let registry = selected_distribution_registry(&runtime);

    assert_eq!(registry.len(), 9);
    assert!(!registry.is_empty());

    let commands = registry.commands();
    assert!(!commands.is_empty());

    // Verify key namespaces are present
    let namespaces: std::collections::BTreeSet<_> =
        commands.iter().map(|c| c.namespace.as_str()).collect();
    assert!(namespaces.contains("core"));
    assert!(namespaces.contains("blog"));
    assert!(namespaces.contains("media"));
    assert!(namespaces.contains("module"));
    assert!(namespaces.contains("rbac"));
    assert!(namespaces.contains("social_graph"));
}
