use anyhow::{Context, Result};
use playwright_rs::api::LaunchOptions;
use playwright_rs::Playwright;

const TOPIC_COMPOSITION_SELECTOR: &str = "body:has([data-storefront-composition='forum-topic-reactions']):not(:has([data-storefront-composition='forum-reply-reactions']))";
const REPLY_COMPOSITION_SELECTOR: &str = "body:has([data-storefront-composition='forum-reply-reactions']):not(:has([data-storefront-composition='forum-topic-reactions']))";

fn required_url(name: &str) -> Result<String> {
    let value = std::env::var(name).with_context(|| format!("missing required {name}"))?;
    let value = value.trim().to_string();
    anyhow::ensure!(!value.is_empty(), "{name} must not be empty");
    Ok(value)
}

async fn assert_composition(url: &str, selector: &str, label: &str) -> Result<()> {
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

    let response = page
        .goto(url, None)
        .await
        .with_context(|| format!("navigate to {label}"))?
        .context("missing navigation response")?;
    let status = response.status();
    anyhow::ensure!(status < 400, "expected {label} status < 400, got {status}");

    let composition = page.locator(selector);
    let text = composition
        .text_content()
        .await
        .with_context(|| format!("read {label} host composition"))?
        .unwrap_or_default();
    anyhow::ensure!(
        !text.trim().is_empty(),
        "expected visible {label} Reactions composition"
    );

    browser.close().await.context("close chromium")?;
    Ok(())
}

#[tokio::test]
async fn leptos_storefront_forum_topic_reactions_mount_once() -> Result<()> {
    let url = required_url("RUSTOK_FORUM_TOPIC_REACTIONS_E2E_URL")?;
    anyhow::ensure!(
        !url.contains("reply="),
        "topic evidence URL must not select a reply"
    );
    assert_composition(&url, TOPIC_COMPOSITION_SELECTOR, "Forum topic").await
}

#[tokio::test]
async fn leptos_storefront_forum_selected_reply_reactions_replace_topic_bar() -> Result<()> {
    let url = required_url("RUSTOK_FORUM_REPLY_REACTIONS_E2E_URL")?;
    anyhow::ensure!(
        url.contains("reply="),
        "reply evidence URL must carry the canonical reply query selection"
    );
    assert_composition(&url, REPLY_COMPOSITION_SELECTOR, "Forum selected reply").await
}
