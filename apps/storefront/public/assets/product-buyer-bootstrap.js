// The guard observes same-document navigations before the WASM client starts. External
// navigation invalidates this mount permanently; owner-controlled replacements preserve the
// current route and are the only history writes that do not invalidate it. This is an in-page
// consistency fence, not an authorization mechanism or a cross-tab lock.
export function installStorefrontNavigationGuard(win) {
  const property = "__rustokStorefrontNavigation";
  if (win[property]) {
    if (!win[property].current()) throw new Error("Storefront navigation is stale");
    return win[property];
  }
  const history = win.history;
  const originalPush = history.pushState;
  const originalReplace = history.replaceState;
  if (typeof originalPush !== "function" || typeof originalReplace !== "function") {
    throw new Error("Storefront history is unavailable");
  }
  let expected = win.location.href;
  let valid = true;
  const invalidate = () => {
    if (!valid) return;
    valid = false;
    win.dispatchEvent(new win.Event("rustok:storefront:navigation"));
  };
  const push = function (...args) {
    invalidate();
    return Reflect.apply(originalPush, this, args);
  };
  const replace = function (...args) {
    invalidate();
    return Reflect.apply(originalReplace, this, args);
  };
  const current = () => {
    if (valid && (history.pushState !== push || history.replaceState !== replace || win.location.href !== expected)) {
      invalidate();
    }
    return valid;
  };
  const guard = Object.freeze({
    current,
    replace(href) {
      if (!current() || typeof href !== "string") return false;
      try {
        const here = new URL(expected);
        const next = new URL(href, here);
        // The Product and Cart owners may change selectors, not the page or its fragment.
        if (next.origin !== here.origin || next.pathname !== here.pathname || next.hash !== here.hash) return false;
        Reflect.apply(originalReplace, history, [history.state, "", next.href]);
        expected = win.location.href;
        return true;
      } catch (_) {
        invalidate();
        return false;
      }
    },
  });
  try {
    history.pushState = push;
    history.replaceState = replace;
    if (history.pushState !== push || history.replaceState !== replace) throw new Error("History hooks are unavailable");
    win.addEventListener("popstate", invalidate);
    win.addEventListener("hashchange", invalidate);
    Object.defineProperty(win, property, { value: guard, configurable: false, writable: false });
  } catch (error) {
    if (history.pushState === push) history.pushState = originalPush;
    if (history.replaceState === replace) history.replaceState = originalReplace;
    win.removeEventListener("popstate", invalidate);
    win.removeEventListener("hashchange", invalidate);
    throw error;
  }
  return guard;
}

// One same-origin Cart owner also supplies the Product buyer. Credentials stay in AuthProvider.
if (typeof document !== "undefined") {
  const product = document.querySelector("[data-product-buyer-island='true']");
  const host = document.querySelector("[data-rustok-cart-host='true']");
  const key = Symbol.for("rustok.storefront.cart-buyer-client");

  async function start() {
    if (!host || window[key]) return;
    window[key] = true;
    const navigation = installStorefrontNavigationGuard(window);
    const client = await import("/assets/product-buyer/rustok_storefront.js");
    await client.default("/assets/product-buyer/rustok_storefront_bg.wasm");
    if (!navigation.current()) throw new Error("Storefront changed while the client was loading");
    client.start_product_buyer_client();
  }

  start().catch(() => {
    window[key] = false;
    console.error("Cart and Product client could not start; purchases remain disabled.");
    if (host) host.setAttribute("data-rustok-cart-client-error", "true");
    if (product) {
      product.setAttribute("data-product-buyer-client-error", "true");
      const notice = document.createElement("p");
      notice.setAttribute("role", "alert");
      notice.textContent = product.getAttribute("data-product-buyer-client-error-label") || "Product interaction is unavailable.";
      product.appendChild(notice);
    }
  });
}
