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
 * - `switch_tab`: switch active tab in a tablist (payload `panel_id`), synchronizing `aria-selected`
 *   and `.is-active` on tabs and toggling visibility/`aria-hidden` on tab panels.
 * - `toggle_accordion`: expand/collapse accordion panel (payload `target_id`, `exclusive`),
 *   synchronizing `aria-expanded` on trigger, `aria-hidden` on content, and optionally collapsing siblings.
 * - `switch_slide`: navigate slider/carousel (payload `slider_id`, `direction`: 'next'|'prev', `index`, `target_slide`),
 *   synchronizing active slide visibility, aria-hidden, and dot indicators, with native swipe/touch support.
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
      action.target_id ||
      action.panel_id ||
      element.getAttribute("data-fly-action-target") ||
      "";
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

  function switchTab(element, action) {
    var target = findTarget(action, element);

    // 1. Update tab triggers in the tablist or common parent
    var tablist = element.closest('[role="tablist"]') || element.parentElement;
    if (tablist) {
      var tabs = tablist.querySelectorAll('[role="tab"], [data-fly-tab-target], [data-fly-action*="switch_tab"]');
      for (var i = 0; i < tabs.length; i++) {
        var tab = tabs[i];
        var isCurrent = tab === element;
        tab.setAttribute("aria-selected", isCurrent ? "true" : "false");
        if (isCurrent) {
          tab.classList.add("is-active");
        } else {
          tab.classList.remove("is-active");
        }
      }
    } else {
      element.setAttribute("aria-selected", "true");
      element.classList.add("is-active");
    }

    // 2. Update target panel and sibling panels
    if (target) {
      var panelsContainer = target.parentElement;
      if (panelsContainer) {
        var panels = panelsContainer.children;
        for (var j = 0; j < panels.length; j++) {
          var panel = panels[j];
          var isTarget = panel === target;
          panel.hidden = !isTarget;
          panel.setAttribute("aria-hidden", isTarget ? "false" : "true");
          if (isTarget) {
            panel.classList.add("is-active");
            if (panel.style.display === "none") panel.style.display = "";
          } else {
            panel.classList.remove("is-active");
            panel.style.display = "none";
          }
        }
      } else {
        target.hidden = false;
        target.setAttribute("aria-hidden", "false");
        target.classList.add("is-active");
        if (target.style.display === "none") target.style.display = "";
      }
    }
  }

  function toggleAccordion(element, action) {
    var target = findTarget(action, element);
    var isExpanded = element.getAttribute("aria-expanded") === "true";
    var willExpand = !isExpanded;

    var isExclusive = action.exclusive !== false;
    if (isExclusive && willExpand) {
      var accordionRoot =
        element.closest("[data-fly-accordion]") ||
        element.closest(".fly-accordion");
      if (accordionRoot) {
        var activeTriggers = accordionRoot.querySelectorAll('[aria-expanded="true"]');
        for (var i = 0; i < activeTriggers.length; i++) {
          var otherTrigger = activeTriggers[i];
          if (otherTrigger !== element) {
            otherTrigger.setAttribute("aria-expanded", "false");
            otherTrigger.classList.remove("is-expanded");
            var otherAction = parseAction(otherTrigger);
            if (otherAction) {
              var otherTarget = findTarget(otherAction, otherTrigger);
              if (otherTarget) {
                otherTarget.hidden = true;
                otherTarget.setAttribute("aria-hidden", "true");
                otherTarget.classList.remove("is-expanded");
                otherTarget.style.display = "none";
              }
            }
          }
        }
      }
    }

    element.setAttribute("aria-expanded", willExpand ? "true" : "false");
    if (willExpand) {
      element.classList.add("is-expanded");
    } else {
      element.classList.remove("is-expanded");
    }

    if (target) {
      target.hidden = !willExpand;
      target.setAttribute("aria-hidden", willExpand ? "false" : "true");
      if (willExpand) {
        target.classList.add("is-expanded");
        if (target.style.display === "none") target.style.display = "";
      } else {
        target.classList.remove("is-expanded");
        target.style.display = "none";
      }
    }
  }

  function switchSlide(element, action) {
    var slider =
      findTarget(action, element) ||
      (element && typeof element.closest === "function"
        ? element.closest("[data-fly-slider]") || element.closest('[aria-roledescription="carousel"]')
        : null);
    if (!slider) return;

    var slides = slider.querySelectorAll('[data-fly-slide], [aria-roledescription="slide"]');
    if (slides.length === 0) return;

    var currentIndex = -1;
    for (var i = 0; i < slides.length; i++) {
      if (slides[i].classList.contains("is-active") || (!slides[i].hidden && slides[i].style.display !== "none")) {
        currentIndex = i;
        break;
      }
    }
    if (currentIndex === -1) currentIndex = 0;

    var nextIndex = currentIndex;
    if (action.direction === "next") {
      nextIndex = (currentIndex + 1) % slides.length;
    } else if (action.direction === "prev") {
      nextIndex = (currentIndex - 1 + slides.length) % slides.length;
    } else if (typeof action.index === "number") {
      nextIndex = Math.max(0, Math.min(action.index, slides.length - 1));
    } else if (action.target_slide) {
      for (var s = 0; s < slides.length; s++) {
        if (slides[s].id === action.target_slide || slides[s].getAttribute("data-fly-component-id") === action.target_slide) {
          nextIndex = s;
          break;
        }
      }
    }

    slider.setAttribute("data-fly-slider-active", String(nextIndex));

    for (var k = 0; k < slides.length; k++) {
      var slide = slides[k];
      var isCurrent = k === nextIndex;
      slide.hidden = !isCurrent;
      slide.setAttribute("aria-hidden", isCurrent ? "false" : "true");
      if (isCurrent) {
        slide.classList.add("is-active");
        if (slide.style.display === "none") slide.style.display = "";
      } else {
        slide.classList.remove("is-active");
        slide.style.display = "none";
      }
    }

    var dots = slider.querySelectorAll(".fly-slider__dot, [data-fly-action*='switch_slide']");
    for (var d = 0; d < dots.length; d++) {
      var dot = dots[d];
      var dotAction = parseAction(dot);
      if (dotAction && typeof dotAction.index === "number") {
        var isDotCurrent = dotAction.index === nextIndex;
        dot.setAttribute("aria-selected", isDotCurrent ? "true" : "false");
        if (isDotCurrent) {
          dot.classList.add("is-active");
          dot.style.background = "#ffffff";
        } else {
          dot.classList.remove("is-active");
          dot.style.background = "rgba(255,255,255,0.4)";
        }
      }
    }
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
      case "switch_tab":
        switchTab(element, action);
        return true;
      case "toggle_accordion":
        toggleAccordion(element, action);
        return true;
      case "switch_slide":
        switchSlide(element, action);
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

  var touchStartX = 0;
  var touchStartY = 0;
  var touchSlider = null;

  function onTouchStart(event) {
    if (!event.touches || event.touches.length !== 1) return;
    var target = event.target;
    if (!target || typeof target.closest !== "function") return;
    touchSlider = target.closest("[data-fly-slider], [aria-roledescription='carousel']");
    touchStartX = event.touches[0].clientX;
    touchStartY = event.touches[0].clientY;
  }

  function onTouchEnd(event) {
    if (!touchSlider || !event.changedTouches || event.changedTouches.length !== 1) {
      touchSlider = null;
      return;
    }
    var deltaX = event.changedTouches[0].clientX - touchStartX;
    var deltaY = event.changedTouches[0].clientY - touchStartY;
    if (Math.abs(deltaX) > 40 && Math.abs(deltaX) > Math.abs(deltaY)) {
      var direction = deltaX < 0 ? "next" : "prev";
      switchSlide(touchSlider, { direction: direction });
    }
    touchSlider = null;
  }

  function start(doc) {
    var documentRef = doc || document;
    if (documentRef[ADAPTER_KEY]) return documentRef[ADAPTER_KEY];
    documentRef.addEventListener("click", onClick);
    documentRef.addEventListener("touchstart", onTouchStart, { passive: true });
    documentRef.addEventListener("touchend", onTouchEnd, { passive: true });
    var adapter = {
      stop: function () {
        documentRef.removeEventListener("click", onClick);
        documentRef.removeEventListener("touchstart", onTouchStart);
        documentRef.removeEventListener("touchend", onTouchEnd);
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
