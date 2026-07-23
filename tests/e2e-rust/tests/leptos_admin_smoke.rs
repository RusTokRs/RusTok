use anyhow::{Context, Result};
use playwright_rs::api::LaunchOptions;
use playwright_rs::Playwright;

fn leptos_admin_url(path: &str) -> String {
    let base = std::env::var("RUSTOK_LEPTOS_ADMIN_E2E_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

#[tokio::test]
async fn leptos_admin_mcp_route_renders() -> Result<()> {
    let playwright = Playwright::launch().await.context("launch playwright")?;
    let launch_options = std::env::var("PLAYWRIGHT_CHROMIUM_EXECUTABLE")
        .ok()
        .map(|path| LaunchOptions::new().executable_path(path));
    let browser = match launch_options {
        Some(options) => playwright
            .chromium()
            .launch_with_options(options)
            .await
            .context("launch chromium")?,
        None => playwright
            .chromium()
            .launch()
            .await
            .context("launch chromium")?,
    };
    let page = browser.new_page().await.context("open page")?;

    let url = leptos_admin_url("/mcp");
    let response = page
        .goto(&url, None)
        .await
        .context("navigate to Leptos MCP route")?
        .context("missing navigation response")?;

    let status = response.status();
    anyhow::ensure!(status < 400, "expected /mcp status < 400, got {status}");

    let body = page.locator("body").await;
    let text = body
        .text_content()
        .await
        .context("read body text")?
        .unwrap_or_default();
    anyhow::ensure!(!text.trim().is_empty(), "expected non-empty body text");

    browser.close().await.context("close chromium")?;
    Ok(())
}
