use rustok_core::registry::ModuleRegistry;
use rustok_mcp::McpServerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModuleRegistry::new();
    let config = McpServerConfig::new(registry);

    rustok_mcp::serve_stdio(config).await?;
    Ok(())
}
