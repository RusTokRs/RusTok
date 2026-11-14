import assert from "node:assert/strict";
import { test } from "node:test";
import { installStorefrontNavigationGuard } from "../public/assets/product-buyer-bootstrap.js";

function browser() {
  const win = new EventTarget();
  win.Event = Event;
  win.location = { href: "https://shop.example/en/product?quantity=1#details" };
  win.history = {
    state: { retained: true },
    pushState(state, _title, href) {
      this.state = state;
      if (href !== undefined && href !== null) win.location.href = new URL(href, win.location.href).href;
    },
    replaceState(state, _title, href) {
      this.state = state;
      if (href !== undefined && href !== null) win.location.href = new URL(href, win.location.href).href;
    },
  };
  return win;
}

test("owner replacements preserve the route, fragment and existing history state", () => {
  const win = browser();
  let invalidations = 0;
  win.addEventListener("rustok:storefront:navigation", () => invalidations++);
  const guard = installStorefrontNavigationGuard(win);
  assert.equal(guard.replace("/en/product?quantity=2#details"), true);
  assert.equal(win.location.href, "https://shop.example/en/product?quantity=2#details");
  assert.deepEqual(win.history.state, { retained: true });
  assert.equal(guard.replace("/en/cart#details"), false);
  assert.equal(guard.replace("https://elsewhere.example/en/product#details"), false);
  assert.equal(guard.replace("/en/product?quantity=3#other"), false);
  assert.equal(guard.current(), true);
  assert.equal(invalidations, 0);
  assert.equal(installStorefrontNavigationGuard(win), guard);
});

test("external history changes invalidate synchronously, even on same-URL ABA", () => {
  for (const method of ["pushState", "replaceState"]) {
    const win = browser();
    const guard = installStorefrontNavigationGuard(win);
    let locationWhenInvalidated;
    win.addEventListener("rustok:storefront:navigation", () => { locationWhenInvalidated = win.location.href; });
    win.history[method](null, "", "/en/product?quantity=2#details");
    assert.equal(locationWhenInvalidated, "https://shop.example/en/product?quantity=1#details");
    win.history[method](null, "", "/en/product?quantity=1#details");
    assert.equal(guard.current(), false);
    assert.equal(guard.replace("/en/product?cart_id=stale#details"), false);
    assert.throws(() => installStorefrontNavigationGuard(win));
  }
});

test("popstate, hashchange, missing hooks and unannounced location changes fail closed", () => {
  for (const change of [
    (win) => win.dispatchEvent(new Event("popstate")),
    (win) => win.dispatchEvent(new Event("hashchange")),
    (win) => { win.history.replaceState = () => {}; },
    (win) => { win.location.href = "https://shop.example/en/product?quantity=3#details"; },
  ]) {
    const win = browser();
    const guard = installStorefrontNavigationGuard(win);
    let invalidations = 0;
    win.addEventListener("rustok:storefront:navigation", () => invalidations++);
    change(win);
    assert.equal(guard.current(), false);
    assert.equal(invalidations, 1);
  }
});

test("an unavailable history hook does not install a permissive guard", () => {
  const win = browser();
  const originalReplace = win.history.replaceState;
  Object.defineProperty(win.history, "pushState", { writable: false });
  assert.throws(() => installStorefrontNavigationGuard(win));
  assert.equal(win.__rustokStorefrontNavigation, undefined);
  assert.equal(win.history.replaceState, originalReplace);
});
