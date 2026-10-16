/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

/**
 * Fly action runtime for published pages.
 *
 * Dispatches `fly:action` CustomEvent on `emit_event` and `provider_action`,
 * and natively handles `toggle_visibility`, `scroll_to`, and `animate`.
 */

const ADAPTER_KEY = Symbol.for("fly.actions.adapter");

interface FlyActionPayload {
  kind: string;
  target_id?: string;
  mode?: "toggle" | "show" | "hide";
  animation?: "fade_in" | "fade_out" | "slide_up" | "slide_down" | "zoom_in" | "pulse";
  duration_ms?: number;
  event?: string;
  payload?: Record<string, unknown>;
  provider?: string;
  action?: string;
  input?: Record<string, unknown>;
}

function parseAction(element: Element): FlyActionPayload | null {
  const raw = element.getAttribute("data-fly-action");
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch {
    return null;
  }
}

function findTarget(action: FlyActionPayload, element: Element): HTMLElement | null {
  const targetId = action.target_id || element.getAttribute("data-fly-action-target") || "";
  if (!targetId) return null;
  const root = element.ownerDocument || document;

  const byId = root.getElementById(targetId);
  if (byId) return byId;

  try {
    return root.querySelector(
      `[data-fly-component-id="${targetId.replace(/["\\]/g, "\\$&")}"]`
    ) as HTMLElement | null;
  } catch {
    return null;
  }
}

function dispatchCustom(element: Element, action: FlyActionPayload) {
  const detail: Record<string, unknown> = {
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
    new CustomEvent("fly:action", {
      bubbles: true,
      cancelable: true,
      detail,
    })
  );
}

function toggleVisibility(element: Element, action: FlyActionPayload) {
  const target = findTarget(action, element);
  if (!target) return;
  const mode = action.mode || "toggle";
  const willShow = mode === "show" ? true : mode === "hide" ? false : target.hidden;
  target.hidden = !willShow;
  target.setAttribute("aria-hidden", willShow ? "false" : "true");
  element.setAttribute("aria-expanded", willShow ? "true" : "false");
}

function scrollToTarget(element: Element, action: FlyActionPayload) {
  const target = findTarget(action, element);
  if (!target) return;
  if (typeof target.scrollIntoView === "function") {
    target.scrollIntoView({ behavior: "smooth", block: "start" });
  }
  if (typeof target.focus === "function") {
    target.focus({ preventScroll: true });
  }
}

const ANIMATIONS: Record<string, Keyframe[]> = {
  fade_in: [{ opacity: "0" }, { opacity: "1" }],
  fade_out: [{ opacity: "1" }, { opacity: "0" }],
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

function animateTarget(element: Element, action: FlyActionPayload) {
  const target = findTarget(action, element);
  if (!target) return;
  const keyframes = action.animation ? ANIMATIONS[action.animation] : undefined;
  if (!keyframes) return;
  let duration = Number(action.duration_ms);
  if (!Number.isFinite(duration) || duration <= 0) duration = 300;
  duration = Math.min(10000, Math.max(50, duration));

  const reduced =
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  if (reduced || typeof target.animate !== "function") {
    const endState = keyframes[keyframes.length - 1];
    if (endState && endState.opacity) target.style.opacity = String(endState.opacity);
    return;
  }

  target.animate(keyframes, {
    duration,
    easing: "ease-out",
    fill: "forwards",
  });
}

function handleAction(action: FlyActionPayload, element: Element): boolean {
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

function onClick(event: MouseEvent) {
  const origin = event.target as Element | null;
  if (!origin || typeof origin.closest !== "function") return;
  const element = origin.closest("[data-fly-action]");
  if (!element) return;
  const action = parseAction(element);
  if (action && handleAction(action, element)) {
    if (element.tagName === "A") event.preventDefault();
  }
}

export function initFlyActions(doc = typeof document !== "undefined" ? document : null) {
  if (!doc) return;
  const targetDoc = doc as unknown as Record<symbol, boolean>;
  if (targetDoc[ADAPTER_KEY]) return;
  doc.addEventListener("click", onClick as EventListener);
  targetDoc[ADAPTER_KEY] = true;
}
