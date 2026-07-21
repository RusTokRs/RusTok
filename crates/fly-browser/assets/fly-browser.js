const FLY_PROTOCOL = "fly_iframe";
const ADAPTER_VERSION = "fly_browser";
const ADAPTER_KEY = Symbol.for("fly.browser.adapter");
const ROOT_SELECTOR = "[data-fly-browser-root]";
const IFRAME_SELECTOR = "iframe[data-fly-iframe-canvas]";
const TOKEN_KEY = "rustok-admin-token";
const TENANT_KEY = "rustok-admin-tenant";
const DRAFT_PREFIX = "fly:ssr-draft:";
const INTENT_REQUEST_ABORTED_CODE = "INTENT_REQUEST_ABORTED";

/**
 * @typedef {"external" | "timeout" | "adapter_stop"} IntentAbortKind
 * @typedef {{
 *   code?: string,
 *   kind?: IntentAbortKind,
 *   error?: string,
 * }} IntentAbortMetadata
 * @typedef {{
 *   signal?: AbortSignal,
 *   abort?: IntentAbortMetadata,
 * }} IntentTransportOptions
 */

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isAbortSignal(value) {
  return typeof AbortSignal === "function" && value instanceof AbortSignal;
}

function normalizedTransportOptions(value) {
  const transport = isObject(value) ? value : {};
  return {
    signal: isAbortSignal(transport.signal) ? transport.signal : undefined,
    abort: isObject(transport.abort) ? transport.abort : {},
  };
}

function nonEmptyString(value) {
  return typeof value === "string" && value.trim() ? value : null;
}

function abortError(transport, error) {
  const configured = nonEmptyString(transport.abort.error);
  if (configured) return configured;
  if (transport.signal?.reason !== undefined)
    return String(transport.signal.reason);
  return String(error);
}

function intentAbortDetail(
  adapter,
  transport,
  request,
  requestGeneration,
  current,
  error,
) {
  return {
    code: nonEmptyString(transport.abort.code) || INTENT_REQUEST_ABORTED_CODE,
    kind: nonEmptyString(transport.abort.kind) || "external",
    error: abortError(transport, error),
    intent: request.intent || null,
    request,
    requestGeneration,
    current,
    instanceId: adapter.instanceId,
    pageId: adapter.pageId,
  };
}

function storedString(key) {
  try {
    const raw = globalThis.localStorage?.getItem(key);
    if (!raw) return null;
    try {
      const parsed = JSON.parse(raw);
      return typeof parsed === "string" && parsed.trim() ? parsed.trim() : null;
    } catch (_) {
      return raw.trim() || null;
    }
  } catch (_) {
    return null;
  }
}

function draftStorageKey(pageId) {
  return `${DRAFT_PREFIX}${pageId || "unbound"}`;
}

function readDraftSession(pageId) {
  try {
    const raw = globalThis.sessionStorage?.getItem(draftStorageKey(pageId));
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (!isObject(parsed) || typeof parsed.token !== "string") return null;
    return {
      token: parsed.token,
      generation: Number.isSafeInteger(parsed.generation)
        ? parsed.generation
        : null,
    };
  } catch (_) {
    return null;
  }
}

function writeDraftSession(pageId, token, generation) {
  if (typeof token !== "string" || !token) return;
  try {
    globalThis.sessionStorage?.setItem(
      draftStorageKey(pageId),
      JSON.stringify({ token, generation }),
    );
  } catch (_) {
    // Private browsing/storage denial must not break server-side authoring.
  }
}

function parseEnvelope(raw) {
  if (typeof raw !== "string") return null;
  try {
    const envelope = JSON.parse(raw);
    if (!isObject(envelope)) return null;
    if (envelope.protocol !== FLY_PROTOCOL) return null;
    if (typeof envelope.instance_id !== "string") return null;
    if (!Number.isSafeInteger(envelope.sequence) || envelope.sequence < 0)
      return null;
    if (
      !isObject(envelope.message) ||
      typeof envelope.message.type !== "string"
    )
      return null;
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

function ensureOverlay(frameHost, kind) {
  let overlay = frameHost.querySelector(`[data-fly-browser-overlay="${kind}"]`);
  if (overlay) return overlay;
  overlay = document.createElement("div");
  overlay.dataset.flyBrowserOverlay = kind;
  overlay.setAttribute("aria-hidden", "true");
  overlay.style.cssText =
    "display:none;position:absolute;pointer-events:none;z-index:30;box-sizing:border-box";
  if (kind === "hovered") overlay.style.border = "1px dashed rgb(96 165 250)";
  if (kind === "selected") overlay.style.border = "2px solid rgb(37 99 235)";
  if (kind === "insertion") {
    overlay.style.border = "3px solid rgb(22 163 74)";
    overlay.style.background = "rgb(22 163 74 / 0.10)";
  }
  frameHost.appendChild(overlay);
  return overlay;
}

function applyRect(overlay, rect, zoom) {
  if (!rect) {
    overlay.style.display = "none";
    return;
  }
  const scale = Number.isFinite(zoom) && zoom > 0 ? zoom : 1;
  overlay.style.display = "block";
  overlay.style.left = `${Number(rect.left || 0) * scale}px`;
  overlay.style.top = `${Number(rect.top || 0) * scale}px`;
  overlay.style.width = `${Number(rect.width || 0) * scale}px`;
  overlay.style.height = `${Number(rect.height || 0) * scale}px`;
}

function normalizedIntent(adapter, input) {
  const message = isObject(input.message) ? input.message : null;
  const intent = String(input.intent || input.type || message?.type || "")
    .trim()
    .toLowerCase();
  const payload = isObject(input.payload)
    ? { ...input.payload }
    : message
      ? { ...message }
      : {};
  delete payload.type;
  if (adapter.selectedId && !payload.selected_component_id) {
    payload.selected_component_id = adapter.selectedId;
  }
  return {
    protocol: FLY_PROTOCOL,
    instance_id: adapter.instanceId,
    intent,
    payload,
    sequence: Number.isSafeInteger(input.sequence) ? input.sequence : null,
    page_id: adapter.pageId,
    revision: adapter.root.dataset.flyRevision || null,
    project_hash: adapter.root.dataset.flyProjectHash || null,
    draft_token: adapter.draftSession?.token || null,
    draft_generation: Number.isSafeInteger(adapter.draftSession?.generation)
      ? adapter.draftSession.generation
      : null,
  };
}

function containsPoint(rect, point) {
  const left = Number(rect?.left || 0);
  const top = Number(rect?.top || 0);
  const width = Number(rect?.width || 0);
  const height = Number(rect?.height || 0);
  return (
    width >= 0 &&
    height >= 0 &&
    point.x >= left &&
    point.y >= top &&
    point.x <= left + width &&
    point.y <= top + height
  );
}

function dropPosition(rect, point) {
  const height = Math.max(Number(rect.height || 0), 1);
  const ratio = (point.y - Number(rect.top || 0)) / height;
  if (ratio <= 0.24) return "before";
  if (ratio >= 0.76) return "after";
  return "inside";
}

function dropOverlayRect(rect, position) {
  if (!rect || position === "inside") return rect;
  const line = 4;
  const top =
    position === "before"
      ? Number(rect.top || 0) - line / 2
      : Number(rect.top || 0) + Number(rect.height || 0) - line / 2;
  return {
    left: Number(rect.left || 0),
    top,
    width: Number(rect.width || 0),
    height: line,
  };
}

export class FlyBrowserAdapter {
  constructor(root, options = {}) {
    if (!(root instanceof Element)) {
      throw new TypeError("Fly browser root must be an Element");
    }
    this.root = root;
    this.options = options;
    this.iframe = root.querySelector(options.iframeSelector || IFRAME_SELECTOR);
    if (!(this.iframe instanceof HTMLIFrameElement)) {
      throw new Error(
        "Fly iframe canvas was not found inside the browser root",
      );
    }
    this.instanceId = instanceIdFor(this.iframe);
    this.pageId = root.dataset.flyPageId || null;
    this.expectedOrigin =
      options.expectedOrigin || root.dataset.flyExpectedOrigin || "null";
    this.intentEndpoint =
      options.intentEndpoint || root.dataset.flyIntentEndpoint || null;
    this.csrfToken = options.csrfToken || root.dataset.flyCsrfToken || null;
    this.accessToken = options.accessToken || storedString(TOKEN_KEY);
    this.tenantSlug = options.tenantSlug || storedString(TENANT_KEY);
    this.draftSession = readDraftSession(this.pageId);
    this.drawOverlays = options.drawOverlays !== false;
    this.postIntents = options.postIntents !== false;
    this.intentRequestGeneration = 0;
    this.latestIntentRequestGeneration = 0;
    this.lastSequence = null;
    this.geometry = new Map();
    this.selectedId = null;
    this.hoveredId = null;
    this.activeDrag = null;
    this.activeDrop = null;
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
    window.addEventListener("message", (event) => this.onMessage(event), {
      signal,
    });
    this.root.addEventListener(
      "fly:select",
      (event) => {
        this.selectedId = event.detail?.componentId || null;
        this.drawSelection();
      },
      { signal },
    );
    this.root.addEventListener(
      "fly:hover",
      (event) => {
        this.hoveredId = event.detail?.componentId || null;
        this.drawSelection();
      },
      { signal },
    );
    this.root.addEventListener(
      "fly:insertion-overlay",
      (event) => {
        if (this.overlays) {
          applyRect(
            this.overlays.insertion,
            event.detail?.rect || null,
            this.zoom,
          );
        }
      },
      { signal },
    );
    this.bindSsrControls(signal);
    this.root.dataset.flyBrowserMounted = "true";
    this.root.dispatchEvent(
      new CustomEvent("fly:browser-ready", {
        bubbles: true,
        detail: { instanceId: this.instanceId, adapter: this },
      }),
    );
    return this;
  }

  stop() {
    this.latestIntentRequestGeneration = ++this.intentRequestGeneration;
    this.abortController.abort();
    this.root.dataset.flyBrowserMounted = "false";
    this.root.dataset.flyCanvasConnected = "false";
    this.lastSequence = null;
    this.geometry.clear();
    this.selectedId = null;
    this.hoveredId = null;
    this.zoom = 1;
    this.cancelDrag();
    this.drawSelection();
    for (const overlay of Object.values(this.overlays || {})) overlay.remove();
    if (this.root[ADAPTER_KEY] === this) delete this.root[ADAPTER_KEY];
  }

  onMessage(event) {
    if (event.source !== this.iframe.contentWindow) return;
    if (event.origin !== this.expectedOrigin) return;
    const envelope = parseEnvelope(event.data);
    if (!envelope || envelope.instance_id !== this.instanceId) return;
    if (this.lastSequence !== null && envelope.sequence <= this.lastSequence)
      return;
    this.lastSequence = envelope.sequence;
    this.applyBrowserMessage(envelope.message);
    const detail = {
      protocol: envelope.protocol,
      instanceId: envelope.instance_id,
      sequence: envelope.sequence,
      message: envelope.message,
      iframeId: this.iframe.id,
      pageId: this.pageId,
      revision: this.root.dataset.flyRevision || null,
      projectHash: this.root.dataset.flyProjectHash || null,
    };
    this.root.dispatchEvent(
      new CustomEvent("fly:canvas-message", { bubbles: true, detail }),
    );
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
        this.zoom =
          Number.isFinite(message.zoom) && message.zoom > 0
            ? message.zoom
            : this.zoom;
        this.drawSelection();
        this.drawDrop();
        break;
      case "geometry_snapshot":
        this.geometry.clear();
        for (const component of message.components || []) {
          if (component?.component_id && component?.rect) {
            this.geometry.set(component.component_id, component.rect);
          }
        }
        this.drawSelection();
        this.drawDrop();
        break;
      case "focus_requested":
        this.selectedId = message.component_id || null;
        this.drawSelection();
        this.root.dispatchEvent(
          new CustomEvent("fly:select", {
            bubbles: true,
            detail: { componentId: this.selectedId, source: "iframe" },
          }),
        );
        break;
      case "hover_requested":
        this.hoveredId = message.component_id || null;
        this.drawSelection();
        break;
      case "drag_moved":
        this.updateDropCandidate(message.position);
        break;
      case "drop_requested":
        void this.commitDrop(message.position);
        break;
      case "cancel_drag_requested":
        this.cancelDrag();
        break;
      case "teardown":
        this.root.dataset.flyCanvasConnected = "false";
        this.geometry.clear();
        this.selectedId = null;
        this.hoveredId = null;
        this.cancelDrag();
        this.drawSelection();
        break;
      default:
        break;
    }
  }

  bindSsrControls(signal) {
    for (const element of this.root.querySelectorAll("[data-fly-block-id]")) {
      element.setAttribute("draggable", "true");
      element.addEventListener(
        "dragstart",
        (event) => {
          const blockId = element.dataset.flyBlockId;
          if (!blockId) return;
          event.dataTransfer?.setData("application/x-fly-block", blockId);
          event.dataTransfer?.setData("text/plain", blockId);
          this.activeDrag = { kind: "block", block_id: blockId };
          this.activeDrop = null;
          this.root.dataset.flyDragging = "block";
        },
        { signal },
      );
      element.addEventListener("dragend", () => this.cancelDrag(), { signal });
    }
    for (const element of this.root.querySelectorAll(
      "[data-fly-component-id]",
    )) {
      const componentId = element.dataset.flyComponentId;
      element.addEventListener(
        "click",
        () => {
          this.selectedId = componentId || null;
          this.drawSelection();
          this.root.dispatchEvent(
            new CustomEvent("fly:select", {
              bubbles: true,
              detail: { componentId: this.selectedId, source: "ssr-control" },
            }),
          );
        },
        { signal },
      );
      element.setAttribute("draggable", "true");
      element.addEventListener(
        "dragstart",
        (event) => {
          if (!componentId) return;
          event.dataTransfer?.setData(
            "application/x-fly-component",
            componentId,
          );
          event.dataTransfer?.setData("text/plain", componentId);
          this.selectedId = componentId;
          this.activeDrag = { kind: "component", component_id: componentId };
          this.activeDrop = null;
          this.root.dataset.flyDragging = "component";
        },
        { signal },
      );
      element.addEventListener("dragend", () => this.cancelDrag(), { signal });
    }
    this.root.addEventListener(
      "click",
      (event) => {
        const actionElement =
          event.target instanceof Element
            ? event.target.closest("[data-fly-action]")
            : null;
        if (!(actionElement instanceof Element)) return;
        const action = actionElement.dataset.flyAction;
        if (!action) return;
        switch (action) {
          case "insert-block": {
            event.preventDefault();
            const blockId = actionElement.closest("[data-fly-block-id]")
              ?.dataset.flyBlockId;
            if (blockId)
              void this.emitIntent("insert_block", { block_id: blockId });
            break;
          }
          case "begin-block-drag": {
            event.preventDefault();
            const blockId = actionElement.closest("[data-fly-block-id]")
              ?.dataset.flyBlockId;
            if (blockId) {
              this.activeDrag = { kind: "block", block_id: blockId };
              this.activeDrop = null;
              this.root.dataset.flyDragging = "block";
            }
            break;
          }
          case "select-component":
            this.selectedId = actionElement.dataset.flyComponentId || null;
            this.drawSelection();
            break;
          default: {
            if (!action.startsWith("intent:")) break;
            event.preventDefault();
            const intent = action.slice(7);
            if (intent === "begin_selected_move") {
              if (this.selectedId) {
                this.activeDrag = {
                  kind: "component",
                  component_id: this.selectedId,
                };
                this.activeDrop = null;
                this.root.dataset.flyDragging = "component";
              }
            } else if (intent === "cancel_drag") {
              this.cancelDrag();
            } else {
              void this.emitIntent(intent, {});
            }
          }
        }
      },
      { signal },
    );
    this.iframe.addEventListener(
      "load",
      () => {
        this.lastSequence = null;
        this.geometry.clear();
        this.activeDrop = null;
        this.drawSelection();
        this.drawDrop();
      },
      { signal },
    );
  }

  updateDropCandidate(point) {
    if (!this.activeDrag || !isObject(point)) {
      this.activeDrop = null;
      this.drawDrop();
      return null;
    }
    const candidates = [];
    for (const [componentId, rect] of this.geometry.entries()) {
      if (
        this.activeDrag.kind === "component" &&
        this.activeDrag.component_id === componentId
      ) {
        continue;
      }
      if (!containsPoint(rect, point)) continue;
      candidates.push({
        componentId,
        rect,
        area:
          Math.max(Number(rect.width || 0), 0) *
          Math.max(Number(rect.height || 0), 0),
      });
    }
    candidates.sort((left, right) => left.area - right.area);
    const target = candidates[0] || null;
    this.activeDrop = target
      ? {
          target_component_id: target.componentId,
          position: dropPosition(target.rect, point),
          rect: target.rect,
        }
      : null;
    this.drawDrop();
    return this.activeDrop;
  }

  async commitDrop(point) {
    const candidate = this.updateDropCandidate(point);
    if (!candidate || !this.activeDrag) {
      this.cancelDrag();
      return null;
    }
    const payload = {
      source: { ...this.activeDrag },
      target_component_id: candidate.target_component_id,
      position: candidate.position,
    };
    this.cancelDrag();
    return this.emitIntent("drop", payload);
  }

  cancelDrag() {
    this.activeDrag = null;
    this.activeDrop = null;
    delete this.root.dataset.flyDragging;
    this.drawDrop();
  }

  emitIntent(intent, payload = {}) {
    const request = normalizedIntent(this, { intent, payload });
    this.root.dispatchEvent(
      new CustomEvent("fly:browser-intent", {
        bubbles: true,
        detail: request,
      }),
    );
    if (this.postIntents) return this.postIntent(request);
    return Promise.resolve(null);
  }

  shouldPost(type) {
    return ["key_stroke"].includes(type);
  }

  /**
   * @param {unknown} input
   * @param {IntentTransportOptions} requestOptions
   */
  async postIntent(input, requestOptions = {}) {
    if (!this.intentEndpoint) return null;
    const request = normalizedIntent(this, input);
    if (!request.intent) return null;
    const transport = normalizedTransportOptions(requestOptions);
    const requestGeneration = ++this.intentRequestGeneration;
    this.latestIntentRequestGeneration = requestGeneration;
    const headers = {
      "content-type": "application/json",
      "x-fly-browser": ADAPTER_VERSION,
    };
    if (this.csrfToken) headers["x-csrf-token"] = this.csrfToken;
    if (this.accessToken) {
      headers.authorization = `Bearer ${this.accessToken}`;
      headers["x-fly-access-token"] = this.accessToken;
    }
    if (this.tenantSlug) headers["x-tenant-slug"] = this.tenantSlug;
    try {
      const response = await fetch(this.intentEndpoint, {
        method: "POST",
        credentials: "same-origin",
        headers,
        body: JSON.stringify(request),
        signal: transport.signal,
      });
      const result = response.headers
        .get("content-type")
        ?.includes("application/json")
        ? await response.json()
        : await response.text();
      const current = requestGeneration === this.latestIntentRequestGeneration;
      this.root.dispatchEvent(
        new CustomEvent("fly:browser-intent-response", {
          bubbles: true,
          detail: {
            ok: response.ok,
            status: response.status,
            result,
            request,
            requestGeneration,
            current,
          },
        }),
      );
      if (current && response.ok && isObject(result)) {
        const state = isObject(result.result) ? result.result : result;
        if (typeof state.revision_id === "string") {
          this.root.dataset.flyRevision = state.revision_id;
        }
        if (typeof state.project_hash === "string") {
          this.root.dataset.flyProjectHash = state.project_hash;
        }
        if (typeof result.draft_token === "string") {
          this.draftSession = {
            token: result.draft_token,
            generation: Number.isSafeInteger(result.draft_generation)
              ? result.draft_generation
              : null,
          };
          writeDraftSession(
            this.pageId,
            this.draftSession.token,
            this.draftSession.generation,
          );
        }
        if (result.reload === true) {
          globalThis.location.reload();
        } else if (typeof result.location === "string") {
          globalThis.location.assign(result.location);
        }
      }
      return result;
    } catch (error) {
      const current = requestGeneration === this.latestIntentRequestGeneration;
      if (transport.signal?.aborted) {
        this.root.dispatchEvent(
          new CustomEvent("fly:browser-intent-aborted", {
            bubbles: true,
            detail: intentAbortDetail(
              this,
              transport,
              request,
              requestGeneration,
              current,
              error,
            ),
          }),
        );
        return null;
      }
      this.root.dispatchEvent(
        new CustomEvent("fly:browser-error", {
          bubbles: true,
          detail: {
            error: String(error),
            request,
            requestGeneration,
            current,
          },
        }),
      );
      return null;
    }
  }

  drawSelection() {
    if (!this.overlays) return;
    applyRect(
      this.overlays.selected,
      this.selectedId && this.geometry.get(this.selectedId),
      this.zoom,
    );
    applyRect(
      this.overlays.hovered,
      this.hoveredId && this.geometry.get(this.hoveredId),
      this.zoom,
    );
  }

  drawDrop() {
    if (!this.overlays) return;
    const rect = this.activeDrop
      ? dropOverlayRect(this.activeDrop.rect, this.activeDrop.position)
      : null;
    applyRect(this.overlays.insertion, rect, this.zoom);
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

export function bootstrapFlyBrowsers(options = {}) {
  if (options.autoMount === false) return [];
  return mountAllFlyBrowsers(options);
}

export function unmountAllFlyBrowsers(selector = ROOT_SELECTOR) {
  for (const root of document.querySelectorAll(selector))
    root[ADAPTER_KEY]?.stop();
}

const api = {
  protocol: FLY_PROTOCOL,
  version: ADAPTER_VERSION,
  FlyBrowserAdapter,
  mount: mountFlyBrowser,
  mountAll: mountAllFlyBrowsers,
  bootstrap: bootstrapFlyBrowsers,
  unmountAll: unmountAllFlyBrowsers,
};

globalThis.FlyBrowser = Object.assign(globalThis.FlyBrowser || {}, api);

const bootstrapConfig = globalThis.__FLY_BROWSER_CONFIG__ || {};
if (bootstrapConfig.autoMount !== false) {
  if (document.readyState === "loading") {
    document.addEventListener(
      "DOMContentLoaded",
      () => bootstrapFlyBrowsers(bootstrapConfig),
      { once: true },
    );
  } else {
    bootstrapFlyBrowsers(bootstrapConfig);
  }
}
