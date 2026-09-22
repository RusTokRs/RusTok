struct FixtureModule;

impl rustok_module_sdk::Guest for FixtureModule {
    fn run(input: String) -> Result<String, String> {
        Ok(input)
    }
}

rustok_module_sdk::export!(FixtureModule);

#[test]
fn author_api_exposes_the_generated_host_import_signature() {
    let _invoke: fn(&str, &str, &str) -> Result<String, String> =
        rustok_module_sdk::rustok::module::host::invoke;
}
