/** Bridge Fly events from a landing to the RusToK storefront. */
(function () {
  "use strict";
  var STATE_KEY = Symbol.for("rustok.fly.storefront.bridge");
  var state = window[STATE_KEY] || { installed: false, count: null };
  window[STATE_KEY] = state;

  // With no Cart owner connected, an authored "0" is not evidence of an empty cart.
  // Re-run on each included script so multiple independently rendered landings synchronize.
  function syncCounters() {
    var counters = document.querySelectorAll("[data-fly-page-body] [data-fly-island='cart_count']");
    for (var i = 0; i < counters.length; i++) {
      counters[i].textContent = state.count === null ? "—" : String(state.count);
      counters[i].setAttribute("data-fly-cart-count-status", state.count === null ? "unknown" : "confirmed");
    }
  }
  syncCounters();
  if (state.installed) return;
  state.installed = true;

  // Inline scripts inserted by client navigation need not execute. Keep newly mounted landing
  // counters in sync with the owner's last observation without reinstalling event listeners.
  var counterSelector = "[data-fly-island='cart_count']";
  var observer = new MutationObserver(function (records) {
    for (var i = 0; i < records.length; i++) {
      var added = records[i].addedNodes;
      for (var j = 0; j < added.length; j++) {
        var node = added[j];
        if (node.nodeType === 1 && (node.matches(counterSelector) || node.querySelector(counterSelector))) {
          syncCounters();
          return;
        }
      }
    }
  });
  observer.observe(document.documentElement, { childList: true, subtree: true });

  function landingRoot(element) {
    return element && typeof element.closest === "function"
      ? element.closest("[data-fly-page-body]") : null;
  }

  // fly:action payloads are opaque author data. They have not passed the typed
  // navigate_url validator, so check them before asking the browser to navigate.
  function safeNavigationHref(raw) {
    if (typeof raw !== "string") return null;
    var href = raw.trim();
    if (!href || href.length > 2048 || href.startsWith("//") || /[\s\\]/.test(href)) return null;
    for (var i = 0; i < href.length; i++) {
      var code = href.charCodeAt(i);
      if (code < 32 || (code >= 127 && code <= 159)) return null;
    }
    if (href.startsWith("/") || href.startsWith("#") || href.startsWith("?")) return href;
    if (/^(mailto|tel):.+/i.test(href)) return href;
    if (/^https?:\/\//i.test(href)) {
      try {
        var url = new URL(href);
        return (url.protocol === "https:" || url.protocol === "http:") && url.hostname ? href : null;
      } catch {
        return null;
      }
    }
    return null;
  }

  // Keep this handoff in lockstep with the Next storefront. An authored landing has no
  // owner-confirmed price; the Product buyer validates/quotes and asks for confirmation.
  function buyerHandoffHref(data) {
    var href = data.buyerHref;
    var variant = data.variantId;
    if (typeof href !== "string" || !href.startsWith("/") || !safeNavigationHref(href) ||
        typeof variant !== "string" ||
        !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(variant) ||
        /^0{8}(?:-0{4}){3}-0{12}$/.test(variant)) return null;
    var rawQuantity = data.quantity === undefined ? 1 : data.quantity;
    if ((typeof rawQuantity !== "number" &&
         (typeof rawQuantity !== "string" || !/^[0-9]{1,10}$/.test(rawQuantity))) ||
        !Number.isInteger(Number(rawQuantity)) || Number(rawQuantity) < 1 ||
        Number(rawQuantity) > 2147483647) return null;
    try {
      var url = new URL(href, window.location.origin);
      // Published landing payloads are shared across both hosts. The canonical buyerHref uses
      // /[locale]/products/[handle]; this host maps it to its Product module route and handle.
      var route = /^\/(?:([^/]+)\/)?products\/([^/]+)\/?$/.exec(url.pathname);
      if (url.origin !== window.location.origin || !route) return null;
      var segments = [route[1], route[2]];
      for (var i = 0; i < segments.length; i++) {
        if (segments[i] === undefined) continue;
        var segment = decodeURIComponent(segments[i]);
        if (!segment || segment.includes("/") || segment.includes("\\")) return null;
        for (var j = 0; j < segment.length; j++) {
          var code = segment.charCodeAt(j);
          if (code < 32 || (code >= 127 && code <= 159)) return null;
        }
      }
      var keys = new Set();
      for (var key of url.searchParams.keys()) {
        if (keys.has(key) || key === "handle" || key === "variant_id" || key === "quantity" ||
            key === "cart_id" || key.startsWith("axis.")) return null;
        keys.add(key);
      }
      var query = new URLSearchParams();
      query.set("handle", decodeURIComponent(route[2]));
      for (var entry of url.searchParams) query.append(entry[0], entry[1]);
      query.set("variant_id", variant.toLowerCase());
      query.set("quantity", String(Number(rawQuantity)));
      return (route[1] ? "/" + route[1] : "") + "/modules/products?" + query.toString() + url.hash;
    } catch {
      return null;
    }
  }

  function reportError(root, code) {
    var host = root.parentElement;
    if (host) {
      var errors = host.querySelectorAll(":scope > [data-fly-bridge-error]");
      for (var i = 0; i < errors.length; i++) {
        errors[i].hidden = errors[i].getAttribute("data-fly-bridge-error") !== code;
      }
    }
    if (code) root.dispatchEvent(new CustomEvent("fly:bridge-error", { bubbles: true, detail: { code: code } }));
  }

  function cartAction(root, name) {
    // A real host consumer acknowledges by preventDefault(). Without one, surface an error
    // instead of claiming a cart drawer was opened or changed.
    var handled = !window.dispatchEvent(new CustomEvent("rustok:cart:" + name, {
      bubbles: true, cancelable: true, detail: { source: root }
    }));
    reportError(root, handled ? null : "cart_unavailable");
  }

  window.addEventListener("fly:action", function (event) {
    var root = landingRoot(event.target);
    if (!root) return;
    var detail = event.detail || {};
    // Named provider actions belong to their provider, not a similarly named host event.
    if (detail.kind !== "emit_event") return;
    var name = detail.event || "";
    var data = detail.payload || {};

    if (name === "add_to_cart" || name === "cart:add" || name === "buy_now") {
      var buyerHref = buyerHandoffHref(data);
      if (!buyerHref) {
        reportError(root, "buyer_handoff_invalid");
        return;
      }
      reportError(root, null);
      window.location.assign(buyerHref);
    } else if (name === "cart:open" || name === "open_cart") {
      cartAction(root, "open");
    } else if (name === "cart:close" || name === "close_cart") {
      cartAction(root, "close");
    } else if (name === "cart:toggle" || name === "toggle_cart") {
      cartAction(root, "toggle");
    } else if (name === "navigate") {
      var href = safeNavigationHref(data.href || data.url);
      if (href) window.location.assign(href);
    }
  });

  document.addEventListener("click", function (event) {
    var target = event.target;
    var root = landingRoot(target);
    if (!root || event.defaultPrevented || event.button !== 0 || event.metaKey ||
        event.ctrlKey || event.shiftKey || event.altKey) return;
    var trigger = target.closest("[data-fly-island='cart_trigger']");
    // The canonical action runtime handles a co-located Fly action; do not toggle twice.
    var action = target.closest("[data-fly-action]");
    if (action && root.contains(action)) return;
    if (trigger && root.contains(trigger)) {
      event.preventDefault();
      cartAction(root, "toggle");
      return;
    }
    var anchor = target.closest("a");
    if (!anchor || !root.contains(anchor) || (anchor.target && anchor.target !== "_self") ||
        anchor.hasAttribute("download") || anchor.hasAttribute("data-fly-action")) return;
    var href = anchor.getAttribute("href");
    if (!href || !href.startsWith("#") || href.length <= 1) return;
    var id;
    try {
      id = decodeURIComponent(href.slice(1));
    } catch {
      return;
    }
    var candidates = root.querySelectorAll("[id], a[name]");
    for (var i = 0; i < candidates.length; i++) {
      var candidate = candidates[i];
      if (landingRoot(candidate) === root &&
          (candidate.id === id || candidate.getAttribute("name") === id)) {
        event.preventDefault();
        candidate.scrollIntoView({ behavior: "smooth" });
        return;
      }
    }
  });

  window.addEventListener("rustok:cart:updated", function (event) {
    var detail = event.detail || {};
    if (detail.status === "unknown") {
      // The owner must invalidate observations on an auth, tenant, capability, or restore change.
      state.count = null;
      syncCounters();
      return;
    }
    var count = typeof detail.count !== "undefined" ? detail.count : detail.itemCount;
    if (typeof count === "number" && Number.isSafeInteger(count) && count >= 0) {
      state.count = count;
      syncCounters();
    }
  });
})();
