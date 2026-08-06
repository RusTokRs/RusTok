const MODULE_PATH = "/assets/pages-inline-edit/rustok_storefront.js";
const WASM_PATH = "/assets/pages-inline-edit/rustok_storefront_bg.wasm";

async function start() {
  const root = document.getElementById("pages-inline-edit-client-root");
  if (!root || root.dataset.pagesAuthoringRoute !== "true") {
    return;
  }

  const module = await import(MODULE_PATH);
  await module.default(WASM_PATH);
  module.start_pages_inline_edit_client();
}

start().catch((error) => {
  console.error("Pages inline edit client failed to start", error);
  const root = document.getElementById("pages-inline-edit-client-root");
  if (root) {
    root.setAttribute("data-pages-inline-edit-client-error", "true");
  }
});
