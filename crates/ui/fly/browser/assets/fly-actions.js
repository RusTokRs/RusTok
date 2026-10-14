/**
 * Fly action runtime for published pages.
 *
 * Fly materializes author-defined component actions into `data-fly-action-kind` and
 * `data-fly-action` (a JSON payload) attributes and ships no behaviour by itself. This
 * standalone script is that behaviour. It is deliberately tiny and dependency-free:
 *
 * - `emit_event` / `provider_action`: dispatch a bubbling `fly:action` CustomEvent from the
 *   source element with `detail = { kind, event|provider, action, payload|input, source }`.
 *   Hosts subscribe with `element.addEventListener("fly:action", ...)` or at document level.
 * - `toggle_visibility`: show/hide/toggle the target element (payload `target_id`, `mode`),
 *   keeping `aria-expanded` on the trigger and `aria-hidden` on the target in sync.
 * - `scroll_to`: smooth-scroll the target element (payload `target_id`) into view.
 * - `animate`: play an allow-listed Web Animations keyframe set (payload `animation`,
 *   optional `duration_ms`) on the target element. The keyframe sets below are the only
 *   ones; payloads can name them but never supply CSS. Reduced-motion users get no
 *   animation (the end state is applied where the keyframes define one).
 *
 * Native actions (`navigate_*`, `submit_form`) never carry `data-fly-action` and are left to
 * the browser. The runtime is idempotent: including the script twice installs one listener.
 */
(function () {
  "use strict";

  var ADAPTER_KEY = Symbol.for("fly.actions.adapter");

  function parseAction(element) {
    var raw = element.getAttribute("data-fly-action");
    if (!raw) return null;
    try {
      var parsed = JSON.parse(raw);
      return parsed && typeof parsed === "object" ? parsed : null;
    } catch (error) {
      return null;
    }
  }

  function findTarget(action, element) {
    var targetId =
      action.target_id || element.getAttribute("data-fly-action-target") || "";
    if (!targetId) return null;
    var root = element.ownerDocument || document;
    if (root.getElementById) {
      var byId = root.getElementById(targetId);
      if (byId) return byId;
    }
    // Instrumented renders (editor canvas, previews) address components by their Fly id
    // attribute instead of a plain `id`; published renders emit `id` for action targets.
    if (root.querySelector) {
      try {
        return root.querySelector(
          '[data-fly-component-id="' + targetId.replace(/["\\]/g, "\\$&") + '"]'
        );
      } catch (error) {
        return null;
      }
    }
    return null;
  }

  function dispatchCustom(element, action) {
    var detail = {
      kind: action.kind,
      source: element,
      sourceId: element.getAttribute("data-fly-component-id") || null,
    };
    if (action.kind === "emit_event") {
      detail.event = action.event || "";
      detail.payload = action.payload || {};
    } else {
      detail.provider = action.provider || "";
      detail.action = action.action || "";
      detail.input = action.input || {};
    }
    element.dispatchEvent(
      new CustomEvent("fly:action", { bubbles: true, cancelable: true, detail: detail })
    );
  }

  function toggleVisibility(element, action) {
    var target = findTarget(action, element);
    if (!target) return;
    var mode = action.mode || "toggle";
    var willShow =
      mode === "show" ? true : mode === "hide" ? false : target.hidden;
    target.hidden = !willShow;
    target.setAttribute("aria-hidden", willShow ? "false" : "true");
    element.setAttribute("aria-expanded", willShow ? "true" : "false");
  }

  function scrollToTarget(element, action) {
    var target = findTarget(action, element);
    if (!target) return;
    if (typeof target.scrollIntoView === "function") {
      target.scrollIntoView({ behavior: "smooth", block: "start" });
    }
    if (typeof target.focus === "function") {
      target.focus({ preventScroll: true });
    }
  }

  var ANIMATIONS = {
    fade_in: [
      { opacity: "0" },
      { opacity: "1" },
    ],
    fade_out: [
      { opacity: "1" },
      { opacity: "0" },
    ],
    slide_up: [
      { opacity: "0", transform: "translateY(16px)" },
      { opacity: "1", transform: "translateY(0)" },
    ],
    slide_down: [
      { opacity: "0", transform: "translateY(-16px)" },
      { opacity: "1", transform: "translateY(0)" },
    ],
    zoom_in: [
      { opacity: "0", transform: "scale(0.96)" },
      { opacity: "1", transform: "scale(1)" },
    ],
    pulse: [
      { transform: "scale(1)" },
      { transform: "scale(1.04)" },
      { transform: "scale(1)" },
    ],
  };

  function animateTarget(element, action) {
    var target = findTarget(action, element);
    if (!target) return;
    var keyframes = ANIMATIONS[action.animation];
    if (!keyframes) return;
    var duration = Number(action.duration_ms);
    if (!Number.isFinite(duration) || duration <= 0) duration = 300;
    duration = Math.min(10000, Math.max(50, duration));
    var reduced =
      typeof window !== "undefined" &&
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduced || typeof target.animate !== "function") {
      var endState = keyframes[keyframes.length - 1];
      if (endState && endState.opacity) target.style.opacity = endState.opacity;
      return;
    }
    target.animate(keyframes, {
      duration: duration,
      easing: "ease-out",
      fill: "forwards",
    });
  }

  function handleAction(action, element) {
    if (!action || typeof action.kind !== "string") return false;
    switch (action.kind) {
      case "emit_event":
      case "provider_action":
        dispatchCustom(element, action);
        return true;
      case "toggle_visibility":
        toggleVisibility(element, action);
        return true;
      case "scroll_to":
        scrollToTarget(element, action);
        return true;
      case "animate":
        animateTarget(element, action);
        return true;
      default:
        return false;
    }
  }

  function onClick(event) {
    var origin = event.target;
    if (!origin || typeof origin.closest !== "function") return;
    var element = origin.closest("[data-fly-action]");
    if (!element) return;
    var action = parseAction(element);
    if (action && handleAction(action, element)) {
      // Custom actions are buttons, not links; never let a stray href navigate.
      if (element.tagName === "A") event.preventDefault();
    }
  }

  function start(doc) {
    var documentRef = doc || document;
    if (documentRef[ADAPTER_KEY]) return documentRef[ADAPTER_KEY];
    documentRef.addEventListener("click", onClick);
    var adapter = {
      stop: function () {
        documentRef.removeEventListener("click", onClick);
        delete documentRef[ADAPTER_KEY];
      },
      handleAction: handleAction,
    };
    documentRef[ADAPTER_KEY] = adapter;
    return adapter;
  }

  var api = { start: start, handleAction: handleAction, parseAction: parseAction };
  if (typeof globalThis !== "undefined") {
    globalThis.FlyActions = api;
  }
  if (typeof document !== "undefined") {
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", function () {
        start(document);
      });
    } else {
      start(document);
    }
  }
})();
