use rustok_module_build_dispatcher::{ModuleBuildDispatcherConfig, run_dispatcher};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_dispatcher(ModuleBuildDispatcherConfig::from_env()?).await?;
    Ok(())
}
