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
 * - `scroll_to`: scroll the target element (payload `target_id`) into view, without motion
 *   when the user requests reduced motion.
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
    } catch {
      return null;
    }
  }

  // A landing is embedded into a host document by both storefronts. Do not let an
  // author-controlled target select the host shell, or a different landing on the page.
  // Standalone previews have no wrapper and are bounded by their own <body> instead.
  var PAGE_ROOT = "[data-fly-page-body], .fly-page-body";
  var SLIDER_ROOT = "[data-fly-slider], [aria-roledescription='carousel']";
  var ACCORDION_ROOT = "[data-fly-accordion], .fly-accordion";
  var visibilityState = new WeakMap();

  function pageRoot(element) {
    return element.closest(PAGE_ROOT) || element.ownerDocument.body;
  }

  // Group discovery and traversal must obey the same boundary as ID lookup. In particular,
  // closest() may otherwise find a host group outside the landing, while querySelectorAll()
  // may find controls in a nested landing owned by a different instance.
  function belongsToPage(element, root) {
    return !!element && !!root && root.contains(element) && pageRoot(element) === root;
  }

  function closestInPage(element, selector, root) {
    var match = element.closest(selector);
    return belongsToPage(match, root) ? match : null;
  }

  function targetId(action, element) {
    return action.target_id || action.panel_id || action.slider_id ||
      element.getAttribute("data-fly-action-target") || "";
  }

  // Component identity takes precedence over an unrelated authored HTML id with the same text.
  // The latter is still the fallback for existing templates whose actions use HTML ids.
  var TARGET_ATTRIBUTES = ["data-fly-target-id", "data-fly-component-id", "data-fly-style-id", "id"];

  function findIn(scope, id, root, attribute, accept) {
    if (scope.nodeType === 1 && scope.getAttribute(attribute) === id &&
        (scope.closest(PAGE_ROOT) || scope.ownerDocument.body) === root &&
        (!accept || accept(scope))) return scope;
    // Only the fixed attribute name enters the selector; author input is compared as a value.
    var candidates = scope.querySelectorAll("[" + attribute + "]");
    for (var i = 0; i < candidates.length; i++) {
      var candidate = candidates[i];
      if (candidate.getAttribute(attribute) === id &&
          (candidate.closest(PAGE_ROOT) || candidate.ownerDocument.body) === root &&
          (!accept || accept(candidate))) return candidate;
    }
    return null;
  }

  function findTarget(action, element, boundary, accept) {
    var id = targetId(action, element);
    if (!id) return null;
    var root = pageRoot(element);
    if (!root) return null;
    var limit = boundary && root.contains(boundary) ? boundary : root;
    // Prefer the nearest common ancestor: existing pages may contain multiple instances of
    // a pre-fix built-in block with the same HTML ids. Do not cross the landing boundary.
    for (var i = 0; i < TARGET_ATTRIBUTES.length; i++) {
      var scope = element;
      while (scope && limit.contains(scope)) {
        var target = findIn(scope, id, root, TARGET_ATTRIBUTES[i], accept);
        if (target) return target;
        if (scope === limit) break;
        scope = scope.parentElement;
      }
    }
    return null;
  }

  // The renderer can emit `display: none` as a stylesheet rule, not an inline style;
  // simply removing `hidden` or clearing style.display does not make such a node visible.
  // Conversely, an authored display rule can override the UA's [hidden] rule. Track and
  // restore the original inline style, and use an inline override only when necessary.
  function setVisible(element, visible, displayHint) {
    var state = visibilityState.get(element);
    if (!state) {
      state = {
        inlineDisplay: element.style.getPropertyValue("display"),
        inlinePriority: element.style.getPropertyPriority("display"),
        lastDisplay: "",
      };
      visibilityState.set(element, state);
    }
    if (!visible) {
      var current = element.ownerDocument.defaultView.getComputedStyle(element).display;
      if (current !== "none") state.lastDisplay = current;
      element.hidden = true;
      element.style.setProperty("display", "none", "important");
    } else {
      element.hidden = false;
      if (state.inlineDisplay && state.inlineDisplay !== "none") {
        element.style.setProperty("display", state.inlineDisplay, state.inlinePriority);
      } else {
        element.style.removeProperty("display");
      }
      if (element.ownerDocument.defaultView.getComputedStyle(element).display === "none") {
        // `revert` recovers the native display for the element when no visible sibling or
        // previously displayed value is available (e.g. an initially collapsed FAQ).
        element.style.setProperty("display", state.lastDisplay || displayHint || "revert", "important");
      }
    }
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
    var isHidden = target.hidden ||
      target.ownerDocument.defaultView.getComputedStyle(target).display === "none";
    var willShow = mode === "show" ? true : mode === "hide" ? false : isHidden;
    setVisible(target, willShow);
    target.setAttribute("aria-hidden", willShow ? "false" : "true");
    element.setAttribute("aria-expanded", willShow ? "true" : "false");
  }

  var TABLIST = '[role="tablist"]';
  var TAB_GROUP = "[data-fly-tabs]";
  var TAB_PANEL = '[role="tabpanel"]';

  function tabOwner(element, root) {
    return closestInPage(element, TABLIST, root) || element.parentElement;
  }

  // Do not target an enclosing group, a nested panel, another tablist, or an explicit
  // neighboring tab group. Component hooks still take precedence over authored HTML ids.
  function ownsTabPanel(candidate, tablist, root) {
    if (candidate.contains(tablist)) return false;
    var candidateList = closestInPage(candidate, TABLIST, root);
    if (candidateList && candidateList !== tablist) return false;
    if (candidateList === tablist && tablist !== root) return false;
    var parentPanel = candidate.parentElement && closestInPage(candidate.parentElement, TAB_PANEL, root);
    if (parentPanel && !parentPanel.contains(tablist)) return false;
    var explicitGroup = closestInPage(candidate, TAB_GROUP, root);
    return !explicitGroup || explicitGroup.contains(tablist);
  }

  function hasForeignTablist(scope, tablist, panels, root) {
    function insideOwnPanel(element) {
      return panels.some(function (panel) { return panel.contains(element); });
    }
    var lists = scope.querySelectorAll(TABLIST);
    for (var i = 0; i < lists.length; i++) {
      var list = lists[i];
      if (list !== tablist && !list.contains(tablist) && belongsToPage(list, root) &&
          !insideOwnPanel(list)) return true;
    }
    // Older authored blocks may omit role=tablist. Their switch_tab controls still form a
    // boundary; a nested control inside an owned panel belongs to a separate inner group.
    var controls = scope.querySelectorAll("[data-fly-action]");
    for (var j = 0; j < controls.length; j++) {
      var control = controls[j];
      if (!belongsToPage(control, root) || tabOwner(control, root) === tablist ||
          insideOwnPanel(control)) continue;
      var other = parseAction(control);
      if (other && other.kind === "switch_tab") return true;
    }
    return false;
  }

  function switchTab(element, action) {
    var root = pageRoot(element);
    var tablist = tabOwner(element, root);
    if (!belongsToPage(tablist, root)) return;
    var tabs = Array.prototype.filter.call(
      tablist.querySelectorAll('[role="tab"], [data-fly-tab-target], [data-fly-action]'),
      function (tab) {
        var tabAction = parseAction(tab);
        return belongsToPage(tab, root) && tabOwner(tab, root) === tablist &&
          tabAction && tabAction.kind === "switch_tab";
      }
    );
    if (tabs.indexOf(element) === -1) return;

    // A header may wrap the list while panels are siblings of that header. Walk up until
    // an owned panel is found, but stop at the landing, explicit group, enclosing panel,
    // or a competing tablist. Missing targets must never borrow a neighbor's panel.
    var boundary = root;
    var explicitGroup = closestInPage(tablist, TAB_GROUP, root);
    var parentPanel = tablist.parentElement && closestInPage(tablist.parentElement, TAB_PANEL, root);
    if (explicitGroup) boundary = explicitGroup;
    if (parentPanel && boundary.contains(parentPanel)) boundary = parentPanel;
    var scope = tablist === boundary ? boundary : tablist.parentElement;
    var resolved = null;
    while (belongsToPage(scope, root) && boundary.contains(scope)) {
      var candidates = tabs.map(function (tab) {
        return findTarget(parseAction(tab), tab, scope, function (candidate) {
          return ownsTabPanel(candidate, tablist, root);
        });
      });
      // A target inside another owned panel is an inner group's content, even if that
      // outer panel has no explicit role. It must not be treated as a sibling panel.
      var panels = candidates.filter(function (panel) {
        return panel && !candidates.some(function (other) {
          return other && other !== panel && other.contains(panel);
        });
      });
      if (hasForeignTablist(scope, tablist, panels, root)) return;
      if (panels.length) {
        resolved = candidates.map(function (panel) {
          return panels.indexOf(panel) === -1 ? null : panel;
        });
        break;
      }
      if (scope === boundary) break;
      scope = scope.parentElement;
    }
    var target = resolved && resolved[tabs.indexOf(element)];
    // A stale/missing reference must not change selection or hide any content.
    if (!target) return;

    for (var i = 0; i < tabs.length; i++) {
      var tab = tabs[i];
      var panel = resolved[i];
      if (!panel) continue;
      var isCurrent = tab === element;
      tab.setAttribute("aria-selected", isCurrent ? "true" : "false");
      tab.classList.toggle("is-active", isCurrent);
      var isTarget = panel === target;
      panel.classList.toggle("is-active", isTarget);
      setVisible(panel, isTarget);
      panel.setAttribute("aria-hidden", isTarget ? "false" : "true");
    }
  }

  function toggleAccordion(element, action) {
    var root = pageRoot(element);
    var accordionRoot = closestInPage(element, ACCORDION_ROOT, root);
    var target = findTarget(action, element, accordionRoot);
    if (!target) return;
    var isExpanded = element.getAttribute("aria-expanded") === "true";
    var willExpand = !isExpanded;

    // The typed contract defaults `exclusive` to false. Only other accordion actions
    // owned by this group participate; nested accordions and unrelated expanded controls
    // are independent.
    if (action.exclusive === true && willExpand && accordionRoot) {
      var activeTriggers = accordionRoot.querySelectorAll('[aria-expanded="true"][data-fly-action]');
      for (var i = 0; i < activeTriggers.length; i++) {
        var otherTrigger = activeTriggers[i];
        var otherAction = parseAction(otherTrigger);
        if (otherTrigger === element || !belongsToPage(otherTrigger, root) ||
            closestInPage(otherTrigger, ACCORDION_ROOT, root) !== accordionRoot ||
            !otherAction || otherAction.kind !== "toggle_accordion") continue;
        var otherTarget = findTarget(otherAction, otherTrigger, accordionRoot);
        if (!otherTarget) continue;
        otherTrigger.setAttribute("aria-expanded", "false");
        otherTrigger.classList.remove("is-expanded");
        otherTarget.classList.remove("is-expanded");
        setVisible(otherTarget, false);
        otherTarget.setAttribute("aria-hidden", "true");
      }
    }

    element.setAttribute("aria-expanded", willExpand ? "true" : "false");
    element.classList.toggle("is-expanded", willExpand);
    target.classList.toggle("is-expanded", willExpand);
    setVisible(target, willExpand);
    target.setAttribute("aria-hidden", willExpand ? "false" : "true");
  }

  function resolveSlider(element, action) {
    var id = targetId(action, element);
    if (id && id !== "self") return findTarget(action, element);
    return closestInPage(element, SLIDER_ROOT, pageRoot(element));
  }

  function belongsToSlider(element, slider) {
    var root = pageRoot(slider);
    if (!belongsToPage(element, root)) return false;
    var nearest = closestInPage(element, SLIDER_ROOT, root);
    return nearest === slider || (!nearest && slider.contains(element));
  }

  function switchSlide(element, action) {
    var slider = resolveSlider(element, action);
    if (!slider) return;

    var candidates = slider.querySelectorAll('[data-fly-slide], [aria-roledescription="slide"]');
    var slides = [];
    for (var i = 0; i < candidates.length; i++) {
      if (belongsToSlider(candidates[i], slider)) slides.push(candidates[i]);
    }
    if (slides.length === 0) return;

    var currentIndex = -1;
    for (var j = 0; j < slides.length; j++) {
      if (slides[j].classList.contains("is-active") ||
          (!slides[j].hidden && slides[j].ownerDocument.defaultView.getComputedStyle(slides[j]).display !== "none")) {
        currentIndex = j;
        break;
      }
    }
    if (currentIndex === -1) currentIndex = 0;

    var nextIndex = currentIndex;
    if (action.direction === "next") {
      nextIndex = (currentIndex + 1) % slides.length;
    } else if (action.direction === "prev") {
      nextIndex = (currentIndex - 1 + slides.length) % slides.length;
    } else if (typeof action.index === "number" && Number.isInteger(action.index)) {
      nextIndex = Math.max(0, Math.min(action.index, slides.length - 1));
    } else if (action.target_slide) {
      var found = false;
      for (var a = 0; a < TARGET_ATTRIBUTES.length && !found; a++) {
        for (var s = 0; s < slides.length; s++) {
          if (slides[s].getAttribute(TARGET_ATTRIBUTES[a]) === action.target_slide) {
            nextIndex = s;
            found = true;
            break;
          }
        }
      }
      if (!found) return;
    }

    var displayHint = slides[currentIndex].ownerDocument.defaultView.getComputedStyle(slides[currentIndex]).display;
    if (displayHint === "none") displayHint = "";
    slider.setAttribute("data-fly-slider-active", String(nextIndex));
    for (var k = 0; k < slides.length; k++) {
      var slide = slides[k];
      var isCurrent = k === nextIndex;
      slide.classList.toggle("is-active", isCurrent);
      setVisible(slide, isCurrent, displayHint);
      slide.setAttribute("aria-hidden", isCurrent ? "false" : "true");
    }

    var dots = slider.querySelectorAll(".fly-slider__dot, [data-fly-action]");
    var indicators = [];
    var activeColor = "";
    var inactiveColor = "";
    for (var d = 0; d < dots.length; d++) {
      var dot = dots[d];
      var dotAction = parseAction(dot);
      if (!belongsToSlider(dot, slider) || !dotAction || dotAction.kind !== "switch_slide" ||
          typeof dotAction.index !== "number" || resolveSlider(dot, dotAction) !== slider) continue;
      var color = dot.ownerDocument.defaultView.getComputedStyle(dot).backgroundColor;
      indicators.push({ element: dot, index: dotAction.index, color: color });
      if (dotAction.index === currentIndex) activeColor = color;
      else if (!inactiveColor) inactiveColor = color;
    }
    for (var n = 0; n < indicators.length; n++) {
      var indicator = indicators[n];
      var isDotCurrent = indicator.index === nextIndex;
      indicator.element.setAttribute("aria-selected", isDotCurrent ? "true" : "false");
      indicator.element.classList.toggle("is-active", isDotCurrent);
      // Existing built-in dots encode selected/unselected colors in individual style hooks,
      // not a class rule. Preserve those author colors instead of hard-coding white/grey.
      // If the author has a class rule that already changed the color, it wins.
      if (isDotCurrent !== (indicator.index === currentIndex) && activeColor && inactiveColor && activeColor !== inactiveColor &&
          indicator.element.ownerDocument.defaultView.getComputedStyle(indicator.element).backgroundColor === indicator.color) {
        indicator.element.style.backgroundColor = isDotCurrent ? activeColor : inactiveColor;
      }
    }
  }

  function prefersReducedMotion(element) {
    var view = element.ownerDocument.defaultView;
    return !!view && typeof view.matchMedia === "function" &&
      view.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  function scrollToTarget(element, action) {
    var target = findTarget(action, element);
    if (!target) return;
    if (typeof target.scrollIntoView === "function") {
      // `auto` can still animate if the page sets CSS scroll-behavior: smooth.
      target.scrollIntoView({ behavior: prefersReducedMotion(target) ? "instant" : "smooth", block: "start" });
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
    if (prefersReducedMotion(target) || typeof target.animate !== "function") {
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
    if (!belongsToPage(element, pageRoot(origin))) return;
    var action = parseAction(element);
    if (action && handleAction(action, element)) {
      // A custom action owns the click. The native control may be a descendant OR ancestor
      // of the action element, so inspect the actual click target instead of only the action.
      // Preserve the native checked state of checkbox/radio inputs used as triggers.
      var nativeControl = origin.closest("a, button, input");
      var tag = nativeControl && nativeControl.tagName;
      var type = (nativeControl && nativeControl.type || "").toLowerCase();
      if (tag === "A" || (tag === "BUTTON" && type !== "button") ||
          (tag === "INPUT" && (type === "submit" || type === "image" || type === "reset"))) {
        event.preventDefault();
      }
    }
  }

  var touchStartX = 0;
  var touchStartY = 0;
  var touchSlider = null;
  var touchPageRoot = null;

  function onTouchStart(event) {
    touchSlider = null;
    touchPageRoot = null;
    if (!event.touches || event.touches.length !== 1) return;
    var target = event.target;
    if (!target || typeof target.closest !== "function") return;
    touchPageRoot = pageRoot(target);
    touchSlider = closestInPage(target, SLIDER_ROOT, touchPageRoot);
    touchStartX = event.touches[0].clientX;
    touchStartY = event.touches[0].clientY;
  }

  function cancelTouch() {
    touchSlider = null;
    touchPageRoot = null;
  }

  function onTouchEnd(event) {
    if (!belongsToPage(touchSlider, touchPageRoot) ||
        !event.changedTouches || event.changedTouches.length !== 1) {
      cancelTouch();
      return;
    }
    var deltaX = event.changedTouches[0].clientX - touchStartX;
    var deltaY = event.changedTouches[0].clientY - touchStartY;
    if (Math.abs(deltaX) > 40 && Math.abs(deltaX) > Math.abs(deltaY)) {
      var direction = deltaX < 0 ? "next" : "prev";
      switchSlide(touchSlider, { direction: direction });
    }
    cancelTouch();
  }

  function start(doc) {
    var documentRef = doc || document;
    if (documentRef[ADAPTER_KEY]) return documentRef[ADAPTER_KEY];
    documentRef.addEventListener("click", onClick);
    documentRef.addEventListener("touchstart", onTouchStart, { passive: true });
    documentRef.addEventListener("touchend", onTouchEnd, { passive: true });
    documentRef.addEventListener("touchcancel", cancelTouch, { passive: true });
    var adapter = {
      stop: function () {
        documentRef.removeEventListener("click", onClick);
        documentRef.removeEventListener("touchstart", onTouchStart);
        documentRef.removeEventListener("touchend", onTouchEnd);
        documentRef.removeEventListener("touchcancel", cancelTouch);
        cancelTouch();
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
