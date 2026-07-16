const FLY_PROTOCOL = "fly_iframe_v1";
const ADAPTER_KEY = Symbol.for("fly.browser.adapter");
const ROOT_SELECTOR = "[data-fly-browser-root]";
const IFRAME_SELECTOR = "iframe[data-fly-iframe-canvas]";

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function parseEnvelope(raw) {
  if (typeof raw !== "string") return null;
  try {
    const envelope = JSON.parse(raw);
    if (!isObject(envelope)) return null;
    if (envelope.protocol !== FLY_PROTOCOL) return null;
    if (typeof envelope.instance_id !== "string") return null;
    if (!Number.isSafeInteger(envelope.sequence) || envelope.sequence < 0) return null;
    if (!isObject(envelope.message) || typeof envelope.message.type !== "string") return null;
    return envelope;
  } catch (_) {
    return null;
  }
}

function instanceIdFor(iframe) {
  const explicit = iframe.dataset.flyInstanceId;
  if (explicit) return explicit;
  return iframe.id.endsWith("-frame") ? iframe.id.slice(0, -6) : iframe.id;
}

function rectStyle(rect, zoom) {
  const safeZoom = Number.isFinite(zoom) && zoom > 0 ? zoom : 1;
  return [
    "display:block",
    `left:${Number(rect.left || 0) * safeZoom}px`,
    `top:${Number(rect.top || 0) * safeZoom}px`,
    `width:${Number(rect.width || 0) * safeZoom}px`,
    `height:${Number(rect.height || 0) * safeZoom}px`,
  ].join(";");
}

function ensureOverlay(frameHost, kind) {
  let overlay = frameHost.querySelector(`[data-fly-browser-overlay="${kind}"]`);
  if (overlay) return overlay;
  overlay = document.createElement("div");
  overlay.dataset.flyBrowserOverlay = kind;
  overlay.setAttribute("aria-hidden", "true");
  overlay.style.cssText = "display:none;position:absolute;pointer-events:none;z-index:30;box-sizing:border-box";
  if (kind === "hovered") overlay.style.border = "1px dashed rgb(96 165 250)";
  if (kind === "selected") overlay.style.border = "2px solid rgb(37 99 235)";
  if (kind === "insertion") {
    overlay.style.border = "3px solid rgb(22 163 74)";
    overlay.style.background = "rgb(22 163 74 / 0.10)";
  }
  frameHost.appendChild(overlay);
  return overlay;
}

function eventDetail(adapter, envelope) {
  return {
    protocol: envelope.protocol,
    instanceId: envelope.instance_id,
    sequence: envelope.sequence,
    message: envelope.message,
    iframeId: adapter.iframe.id,
    pageId: adapter.root.dataset.flyPageId || null,
    revision: adapter.root.dataset.flyRevision || null,
    projectHash: adapter.root.dataset.flyProjectHash || null,
  };
}

export class FlyBrowserAdapter {
  constructor(root, options = {}) {
    if (!(root instanceof Element)) throw new TypeError("Fly browser root must be an Element");
    this.root = root;
    this.options = options;
    this.iframe = root.querySelector(options.iframeSelector || IFRAME_SELECTOR);
    if (!(this.iframe instanceof HTMLIFrameElement)) {
      throw new Error("Fly iframe canvas was not found inside the browser root");
    }
    this.instanceId = instanceIdFor(this.iframe);
    this.expectedOrigin = options.expectedOrigin || root.dataset.flyExpectedOrigin || "null";
    this.intentEndpoint = options.intentEndpoint || root.dataset.flyIntentEndpoint || null;
    this.csrfToken = options.csrfToken || root.dataset.flyCsrfToken || null;
    this.drawOverlays = options.drawOverlays !== false;
    this.postIntents = options.postIntents !== false;
    this.lastSequence = null;
    this.geometry = new Map();
    this.selectedId = null;
    this.hoveredId = null;
    this.zoom = 1;
    this.abortController = new AbortController();
    this.frameHost = this.iframe.parentElement || root;
    this.overlays = this.drawOverlays
      ? {
          hovered: ensureOverlay(this.frameHost, "hovered"),
          selected: ensureOverlay(this.frameHost, "selected"),
          insertion: ensureOverlay(this.frameHost, "insertion"),
        }
      : null;
  }

  start() {
    const { signal } = this.abortController;
    window.addEventListener("message", (event) => this.onMessage(event), { signal });
    this.root.addEventListener("fly:select", (event) => {
      this.selectedId = event.detail?.componentId || null;
      this.drawSelection();
    }, { signal });
    this.root.addEventListener("fly:hover", (event) => {
      this.hoveredId = event.detail?.componentId || null;
      this.drawSelection();
    }, { signal });
    this.root.addEventListener("fly:insertion-overlay", (event) => {
      this.drawInsertion(event.detail?.rect || null);
    }, { signal });
    this.bindSsrControls(signal);
    this.root.dataset.flyBrowserMounted = "true";
    this.root.dispatchEvent(new CustomEvent("fly:browser-ready", {
      bubbles: true,
      detail: { instanceId: this.instanceId, adapter: this },
    }));
    return this;
  }

  stop() {
    this.abortController.abort();
    this.root.dataset.flyBrowserMounted = "false";
    for (const overlay of Object.values(this.overlays || {})) overlay.remove();
    if (this.root[ADAPTER_KEY] === this) delete this.root[ADAPTER_KEY];
  }

  onMessage(event) {
    if (event.source !== this.iframe.contentWindow) return;
    if (event.origin !== this.expectedOrigin) return;
    const envelope = parseEnvelope(event.data);
    if (!envelope || envelope.instance_id !== this.instanceId) return;
    if (this.lastSequence !== null && envelope.sequence <= this.lastSequence) return;
    this.lastSequence = envelope.sequence;
    this.applyBrowserMessage(envelope.message);
    const detail = eventDetail(this, envelope);
    this.root.dispatchEvent(new CustomEvent("fly:canvas-message", { bubbles: true, detail }));
    if (this.postIntents && this.shouldPost(envelope.message.type)) {
      void this.postIntent(detail);
    }
  }

  applyBrowserMessage(message) {
    switch (message.type) {
      case "ready":
        this.root.dataset.flyCanvasConnected = "true";
        break;
      case "viewport_changed":
        this.zoom = Number.isFinite(message.zoom) && message.zoom > 0 ? message.zoom : this.zoom;
        this.drawSelection();
        break;
      case "geometry_snapshot":
        this.geometry.clear();
        for (const component of message.components || []) {
          if (component?.component_id && component?.rect) {
            this.geometry.set(component.component_id, component.rect);
          }
        }
        this.drawSelection();
        break;
      case "focus_requested":
        this.selectedId = message.component_id || null;
        this.drawSelection();
        this.root.dispatchEvent(new CustomEvent("fly:select", {
          bubbles: true,
          detail: { componentId: this.selectedId, source: "iframe" },
        }));
        break;
      case "hover_requested":
        this.hoveredId = message.component_id || null;
        this.drawSelection();
        break;
      case "teardown":
        this.root.dataset.flyCanvasConnected = "false";
        this.geometry.clear();
        this.selectedId = null;
        this.hoveredId = null;
        this.drawSelection();
        break;
      default:
        break;
    }
  }

  bindSsrControls(signal) {
    for (const element of this.root.querySelectorAll("[data-fly-block-id]")) {
      element.setAttribute("draggable", "true");
      element.addEventListener("dragstart", (event) => {
        const blockId = element.dataset.flyBlockId;
        if (!blockId) return;
        event.dataTransfer?.setData("application/x-fly-block", blockId);
        this.emitIntent("begin_palette_drag", { block_id: blockId });
      }, { signal });
    }
    for (const element of this.root.querySelectorAll("[data-fly-component-id]")) {
      element.addEventListener("click", () => {
        const componentId = element.dataset.flyComponentId || null;
        this.selectedId = componentId;
        this.drawSelection();
        this.emitIntent("select", { component_id: componentId });
      }, { signal });
    }
    this.iframe.addEventListener("load", () => {
      this.lastSequence = null;
      this.geometry.clear();
      this.drawSelection();
    }, { signal });
  }

  emitIntent(type, payload = {}) {
    const detail = {
      protocol: FLY_PROTOCOL,
      instanceId: this.instanceId,
      type,
      payload,
      pageId: this.root.dataset.flyPageId || null,
      revision: this.root.dataset.flyRevision || null,
      projectHash: this.root.dataset.flyProjectHash || null,
    };
    this.root.dispatchEvent(new CustomEvent("fly:browser-intent", { bubbles: true, detail }));
    if (this.postIntents) void this.postIntent(detail);
  }

  shouldPost(type) {
    return new Set([
      "drop_requested",
      "key_stroke",
      "cancel_drag_requested",
      "focus_requested",
      "hover_requested",
    ]).has(type);
  }

  async postIntent(detail) {
    if (!this.intentEndpoint) return null;
    const headers = { "content-type": "application/json", "x-fly-browser": "fly_browser_v1" };
    if (this.csrfToken) headers["x-csrf-token"] = this.csrfToken;
    try {
      const response = await fetch(this.intentEndpoint, {
        method: "POST",
        credentials: "same-origin",
        headers,
        body: JSON.stringify(detail),
      });
      const result = response.headers.get("content-type")?.includes("application/json")
        ? await response.json()
        : await response.text();
      this.root.dispatchEvent(new CustomEvent("fly:browser-intent-response", {
        bubbles: true,
        detail: { ok: response.ok, status: response.status, result, request: detail },
      }));
      return result;
    } catch (error) {
      this.root.dispatchEvent(new CustomEvent("fly:browser-error", {
        bubbles: true,
        detail: { error: String(error), request: detail },
      }));
      return null;
    }
  }

  drawSelection() {
    if (!this.overlays) return;
    this.drawOverlay(this.overlays.selected, this.selectedId && this.geometry.get(this.selectedId));
    this.drawOverlay(this.overlays.hovered, this.hoveredId && this.geometry.get(this.hoveredId));
  }

  drawInsertion(rect) {
    if (!this.overlays) return;
    this.drawOverlay(this.overlays.insertion, rect);
  }

  drawOverlay(overlay, rect) {
    if (!rect) {
      overlay.style.display = "none";
      return;
    }
    overlay.style.cssText += `;${rectStyle(rect, this.zoom)}`;
  }
}

export function mountFlyBrowser(root, options = {}) {
  if (!(root instanceof Element)) return null;
  if (root[ADAPTER_KEY]) return root[ADAPTER_KEY];
  const adapter = new FlyBrowserAdapter(root, options).start();
  root[ADAPTER_KEY] = adapter;
  return adapter;
}

export function mountAllFlyBrowsers(options = {}) {
  const selector = options.rootSelector || ROOT_SELECTOR;
  return Array.from(document.querySelectorAll(selector))
    .map((root) => mountFlyBrowser(root, options))
    .filter(Boolean);
}

export function unmountAllFlyBrowsers(selector = ROOT_SELECTOR) {
  for (const root of document.querySelectorAll(selector)) root[ADAPTER_KEY]?.stop();
}

const api = {
  protocol: FLY_PROTOCOL,
  version: "fly_browser_v1",
  FlyBrowserAdapter,
  mount: mountFlyBrowser,
  mountAll: mountAllFlyBrowsers,
  unmountAll: unmountAllFlyBrowsers,
};

globalThis.FlyBrowser = Object.assign(globalThis.FlyBrowser || {}, api);

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => mountAllFlyBrowsers(), { once: true });
} else {
  mountAllFlyBrowsers();
}
