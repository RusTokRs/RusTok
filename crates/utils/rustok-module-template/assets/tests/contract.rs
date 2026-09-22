#[test]
fn sdk_exposes_the_typed_broker_import() {
    let _invoke: fn(&str, &str, &str) -> Result<String, String> =
        rustok_module_sdk::rustok::module::host::invoke;
}
