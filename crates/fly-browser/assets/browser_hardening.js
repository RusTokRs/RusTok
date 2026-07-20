(() => {
  const api = globalThis.FlyBrowser;
  const Adapter = api?.FlyBrowserAdapter;
  if (!Adapter || Adapter.prototype.__flyResourceGuardInstalled) return;

  const DEFAULT_MAX_MESSAGE_BYTES = 1024 * 1024;
  const DEFAULT_MAX_GEOMETRY_COMPONENTS = 4096;
  const STATUS_SELECTOR = '[data-fly-browser-status="resource-limit"]';

  const isObject = (value) =>
    value !== null && typeof value === "object" && !Array.isArray(value);

  const boundedPositiveInteger = (value, fallback) => {
    const parsed = Number(value);
    return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : fallback;
  };

  const utf8ByteLength = (value) => {
    if (typeof TextEncoder === "function") {
      return new TextEncoder().encode(value).byteLength;
    }
    return typeof Blob === "function" ? new Blob([value]).size : value.length;
  };

  const limitFor = (adapter, optionName, dataName, fallback) =>
    boundedPositiveInteger(
      adapter.options?.[optionName] ?? adapter.root.dataset[dataName],
      fallback,
    );

  const ensureStatus = (adapter) => {
    let status = adapter.root.querySelector(STATUS_SELECTOR);
    if (status) return status;
    status = document.createElement("p");
    status.dataset.flyBrowserStatus = "resource-limit";
    status.setAttribute("role", "status");
    status.setAttribute("aria-live", "polite");
    status.setAttribute("aria-atomic", "true");
    status.style.cssText =
      "position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0";
    adapter.root.appendChild(status);
    return status;
  };

  const normalizedLimit = (detail, fallbackKind) => ({
    kind:
      typeof detail?.kind === "string" && detail.kind
        ? detail.kind
        : fallbackKind,
    limit: boundedPositiveInteger(detail?.limit, 1),
    observed: Math.max(0, Number(detail?.observed) || 0),
  });

  const reportResourceLimit = (adapter, detail, fallbackKind) => {
    const resourceLimit = normalizedLimit(detail, fallbackKind);
    adapter.root.dataset.flyResourceLimited = resourceLimit.kind;
    adapter.root.dispatchEvent(
      new CustomEvent("fly:browser-resource-limit", {
        bubbles: true,
        detail: {
          ...resourceLimit,
          instanceId: adapter.instanceId,
          pageId: adapter.pageId,
        },
      }),
    );
    const message =
      adapter.options?.resourceLimitMessage ||
      adapter.root.dataset.flyResourceLimitMessage ||
      "Editor canvas resource limit reached.";
    const status = ensureStatus(adapter);
    status.textContent = "";
    queueMicrotask(() => {
      status.textContent = `${message} ${resourceLimit.kind}: ${resourceLimit.observed}/${resourceLimit.limit}.`;
    });
  };

  const originalOnMessage = Adapter.prototype.onMessage;
  Adapter.prototype.onMessage = function onMessageWithResourceLimit(event) {
    if (
      event.source === this.iframe.contentWindow &&
      event.origin === this.expectedOrigin &&
      typeof event.data === "string"
    ) {
      const limit = limitFor(
        this,
        "maxMessageBytes",
        "flyMaxMessageBytes",
        DEFAULT_MAX_MESSAGE_BYTES,
      );
      const observed = utf8ByteLength(event.data);
      if (observed > limit) {
        reportResourceLimit(
          this,
          { kind: "message_bytes", limit, observed },
          "message_bytes",
        );
        return;
      }
    }
    return originalOnMessage.call(this, event);
  };

  const originalApplyBrowserMessage = Adapter.prototype.applyBrowserMessage;
  Adapter.prototype.applyBrowserMessage = function applyBrowserMessageWithResourceLimit(
    message,
  ) {
    if (message?.type === "geometry_snapshot") {
      if (isObject(message.resource_limit)) {
        const result = originalApplyBrowserMessage.call(this, message);
        reportResourceLimit(
          this,
          message.resource_limit,
          "geometry_components",
        );
        return result;
      }
      const components = Array.isArray(message.components)
        ? message.components
        : [];
      const limit = limitFor(
        this,
        "maxGeometryComponents",
        "flyMaxGeometryComponents",
        DEFAULT_MAX_GEOMETRY_COMPONENTS,
      );
      if (components.length > limit) {
        this.geometry.clear();
        this.drawSelection();
        this.drawDrop();
        reportResourceLimit(
          this,
          {
            kind: "geometry_components",
            limit,
            observed: components.length,
          },
          "geometry_components",
        );
        return;
      }
      if (this.root.dataset.flyResourceLimited === "geometry_components") {
        delete this.root.dataset.flyResourceLimited;
      }
    }
    return originalApplyBrowserMessage.call(this, message);
  };

  const originalStop = Adapter.prototype.stop;
  Adapter.prototype.stop = function stopWithResourceLimitCleanup() {
    const result = originalStop.call(this);
    delete this.root.dataset.flyResourceLimited;
    this.root.querySelector(STATUS_SELECTOR)?.remove();
    return result;
  };

  Object.defineProperty(Adapter.prototype, "__flyResourceGuardInstalled", {
    configurable: false,
    enumerable: false,
    value: true,
    writable: false,
  });
})();
