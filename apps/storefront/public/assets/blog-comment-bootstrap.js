const SESSION_KEY = "rustok-admin-session";
const ADAPTER_PATH = "/richtext/frame/leptos-adapter.mjs";
const MODULE_PATH = "/assets/blog-comment/rustok_storefront.js";
const WASM_PATH = "/assets/blog-comment/rustok_storefront_bg.wasm";

function hasActiveSession() {
  try {
    const value = JSON.parse(localStorage.getItem(SESSION_KEY) ?? "null");
    return (
      typeof value?.token === "string" &&
      value.token.length > 0 &&
      Number.isFinite(value?.expires_at) &&
      value.expires_at > Date.now() / 1000
    );
  } catch {
    return false;
  }
}

async function start() {
  const root = document.querySelector("[data-blog-comment-island='true']");
  if (!root || !hasActiveSession()) {
    return;
  }

  await import(ADAPTER_PATH);
  const module = await import(MODULE_PATH);
  await module.default(WASM_PATH);
  module.start_blog_comment_client();
}

start().catch((error) => {
  console.error("Blog comment client failed to start", error);
  const root = document.querySelector("[data-blog-comment-island='true']");
  if (root) {
    root.setAttribute("data-blog-comment-client-error", "true");
  }
});
