// Same-origin public buyer client; authentication is restored by the canonical AuthProvider.
// Guest capabilities remain in the owner-issued HttpOnly cookie, never browser storage or HTML.
const root = document.querySelector("[data-product-buyer-island='true']");

async function start() {
  if (!root) return;
  const client = await import("/assets/product-buyer/rustok_storefront.js");
  await client.default("/assets/product-buyer/rustok_storefront_bg.wasm");
  client.start_product_buyer_client();
}

start().catch(() => {
  console.error("Product buyer client could not start; purchases remain disabled.");
  if (root) {
    root.setAttribute("data-product-buyer-client-error", "true");
    const notice = document.createElement("p");
    notice.setAttribute("role", "alert");
    notice.textContent = root.getAttribute("data-product-buyer-client-error-label");
    root.appendChild(notice);
  }
});
