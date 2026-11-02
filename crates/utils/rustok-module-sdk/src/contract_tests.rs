use super::{SDK_VERSION, WIT_PACKAGE, WIT_SOURCE, WIT_WORLD};

#[test]
fn sdk_constants_contract_verification() {
    assert!(!SDK_VERSION.is_empty(), "SDK_VERSION must not be empty");
    assert_eq!(WIT_PACKAGE, "rustok:module@1.0.0");
    assert_eq!(WIT_WORLD, "module-runtime");
    assert!(!WIT_SOURCE.is_empty(), "WIT_SOURCE must not be empty");
}

#[test]
fn wit_schema_structural_invariants() {
    // 1. Package declaration
    assert!(
        WIT_SOURCE.contains("package rustok:module@1.0.0;"),
        "WIT must declare canonical package rustok:module@1.0.0"
    );

    // 2. Host interface with invoke capability
    assert!(
        WIT_SOURCE.contains("interface host {"),
        "WIT must declare host interface"
    );
    assert!(
        WIT_SOURCE.contains("invoke: func("),
        "host interface must declare invoke func"
    );
    assert!(
        WIT_SOURCE.contains("capability: string,"),
        "invoke must accept capability"
    );
    assert!(
        WIT_SOURCE.contains("operation: string,"),
        "invoke must accept operation"
    );
    assert!(
        WIT_SOURCE.contains("input: string,"),
        "invoke must accept input"
    );
    assert!(
        WIT_SOURCE.contains("-> result<string, string>;"),
        "invoke must return result<string, string>"
    );

    // 3. Module runtime world
    assert!(
        WIT_SOURCE.contains("world module-runtime {"),
        "WIT must declare world module-runtime"
    );
    assert!(
        WIT_SOURCE.contains("import host;"),
        "world must import host interface"
    );
    assert!(
        WIT_SOURCE.contains("export run: func(input: string) -> result<string, string>;"),
        "world must export run function"
    );
}

