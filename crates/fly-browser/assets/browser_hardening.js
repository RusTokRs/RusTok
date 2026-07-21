(() => {
  const api = globalThis.FlyBrowser;
  const Adapter = api?.FlyBrowserAdapter;
  if (!Adapter || Adapter.prototype.__flyResourceGuardInstalled) return;

  const DEFAULT_MAX_MESSAGE_BYTES = 1024 * 1024;
  const DEFAULT_MAX_GEOMETRY_COMPONENTS = 4096;
  const DEFAULT_MAX_PENDING_INTENT_REQUESTS = 8;
  const PENDING_INTENT_LIMIT_CODE = "PENDING_INTENT_LIMIT";
  const ADAPTER_KEY = Symbol.for("fly.browser.adapter");
  const PROBLEM_REPORTER_KEY = Symbol.for("fly.browser.problem.reporter");
  const PENDING_INTENT_CONTROLLERS_KEY = Symbol.for(
    "fly.browser.pending.intent.controllers",
  );
  const RESOURCE_STATUS_SELECTOR =
    '[data-fly-browser-status="resource-limit"]';
  const PROBLEM_STATUS_SELECTOR = '[data-fly-browser-status="problem"]';
  const ROOT_SELECTOR = "[data-fly-browser-root]";

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

  const visuallyHiddenStyle =
    "position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0";

  const ensureResourceStatus = (adapter) => {
    let status = adapter.root.querySelector(RESOURCE_STATUS_SELECTOR);
    if (status) return status;
    status = document.createElement("p");
    status.dataset.flyBrowserStatus = "resource-limit";
    status.setAttribute("role", "status");
    status.setAttribute("aria-live", "polite");
    status.setAttribute("aria-atomic", "true");
    status.style.cssText = visuallyHiddenStyle;
    adapter.root.appendChild(status);
    return status;
  };

  const ensureProblemStatus = (adapter) => {
    let status = adapter.root.querySelector(PROBLEM_STATUS_SELECTOR);
    if (status) return status;
    status = document.createElement("p");
    status.dataset.flyBrowserStatus = "problem";
    status.setAttribute("role", "alert");
    status.setAttribute("aria-live", "assertive");
    status.setAttribute("aria-atomic", "true");
    status.style.cssText = visuallyHiddenStyle;
    adapter.root.appendChild(status);
    return status;
  };

  const normalizedStringList = (value) =>
    Array.isArray(value)
      ? [...new Set(value.filter((item) => typeof item === "string" && item))]
      : [];

  const normalizedProblem = (adapter, detail, fallbackCode) => {
    const result = isObject(detail?.result) ? detail.result : {};
    const request = isObject(detail?.request) ? detail.request : {};
    const status = Number.isSafeInteger(detail?.status) ? detail.status : 0;
    const code =
      typeof result.code === "string" && result.code
        ? result.code
        : fallbackCode || (status > 0 ? `HTTP_${status}` : "BROWSER_REQUEST_FAILED");
    const error =
      typeof result.error === "string" && result.error
        ? result.error
        : typeof detail?.error === "string" && detail.error
          ? detail.error
          : status > 0
            ? `Editor action failed with status ${status}.`
            : "Editor action failed.";
    return {
      status,
      code,
      error,
      intent:
        typeof result.intent === "string" && result.intent
          ? result.intent
          : typeof request.intent === "string" && request.intent
            ? request.intent
            : null,
      capability:
        typeof result.capability === "string" && result.capability
          ? result.capability
          : null,
      required: normalizedStringList(result.required),
      missing: normalizedStringList(result.missing),
      instanceId: adapter.instanceId,
      pageId: adapter.pageId,
    };
  };

  const clearBrowserProblem = (adapter) => {
    delete adapter.root.dataset.flyBrowserProblem;
    adapter.root.querySelector(PROBLEM_STATUS_SELECTOR)?.remove();
  };

  const reportBrowserProblem = (adapter, detail, fallbackCode) => {
    const problem = normalizedProblem(adapter, detail, fallbackCode);
    adapter.root.dataset.flyBrowserProblem = problem.code;
    adapter.root.dispatchEvent(
      new CustomEvent("fly:browser-problem", {
        bubbles: true,
        detail: problem,
      }),
    );
    const status = ensureProblemStatus(adapter);
    status.textContent = "";
    queueMicrotask(() => {
      status.textContent = problem.error;
    });
  };

  const pendingIntentControllers = (adapter) => {
    let controllers = adapter[PENDING_INTENT_CONTROLLERS_KEY];
    if (controllers instanceof Map) return controllers;
    controllers = new Map();
    adapter[PENDING_INTENT_CONTROLLERS_KEY] = controllers;
    return controllers;
  };

  const rejectPendingIntent = (adapter, input, limit, observed) => {
    const request = isObject(input) ? input : {};
    const message =
      adapter.options?.pendingIntentLimitMessage ||
      adapter.root.dataset.flyPendingIntentLimitMessage ||
      "Editor action limit reached.";
    const error = `${message} ${observed}/${limit}.`;
    const intent =
      typeof request.intent === "string" && request.intent
        ? request.intent
        : typeof request.type === "string" && request.type
          ? request.type
          : null;
    const detail = {
      code: PENDING_INTENT_LIMIT_CODE,
      error,
      intent,
      limit,
      observed,
      instanceId: adapter.instanceId,
      pageId: adapter.pageId,
    };
    adapter.root.dispatchEvent(
      new CustomEvent("fly:browser-intent-rejected", {
        bubbles: true,
        detail,
      }),
    );
    reportBrowserProblem(
      adapter,
      {
        status: 0,
        result: {
          code: detail.code,
          error: detail.error,
          intent: detail.intent,
        },
        request,
      },
      PENDING_INTENT_LIMIT_CODE,
    );
    return null;
  };

  const installProblemReporter = (adapter) => {
    if (!adapter || adapter[PROBLEM_REPORTER_KEY]) return adapter;
    adapter[PROBLEM_REPORTER_KEY] = true;
    const signal = adapter.abortController?.signal;
    const options = signal ? { signal } : undefined;
    adapter.root.addEventListener(
      "fly:browser-intent-response",
      (event) => {
        if (event.detail?.current === false) return;
        if (event.detail?.ok === true) {
          clearBrowserProblem(adapter);
        } else {
          reportBrowserProblem(adapter, event.detail, null);
        }
      },
      options,
    );
    adapter.root.addEventListener(
      "fly:browser-error",
      (event) => {
        if (event.detail?.current === false) return;
        reportBrowserProblem(
          adapter,
          {
            status: 0,
            error: event.detail?.error,
            request: event.detail?.request,
          },
          "NETWORK_ERROR",
        );
      },
      options,
    );
    return adapter;
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
    const status = ensureResourceStatus(adapter);
    status.textContent = "";
    queueMicrotask(() => {
      status.textContent = `${message} ${resourceLimit.kind}: ${resourceLimit.observed}/${resourceLimit.limit}.`;
    });
  };

  const originalStart = Adapter.prototype.start;
  Adapter.prototype.start = function startWithProblemReporter() {
    const result = originalStart.call(this);
    installProblemReporter(this);
    return result;
  };

  const originalPostIntent = Adapter.prototype.postIntent;
  Adapter.prototype.postIntent = function postIntentWithPendingLimit(input) {
    if (!this.intentEndpoint) return originalPostIntent.call(this, input);
    const controllers = pendingIntentControllers(this);
    const limit = limitFor(
      this,
      "maxPendingIntentRequests",
      "flyMaxPendingIntentRequests",
      DEFAULT_MAX_PENDING_INTENT_REQUESTS,
    );
    const observed = controllers.size + 1;
    if (observed > limit) {
      return Promise.resolve(rejectPendingIntent(this, input, limit, observed));
    }

    const controller = new AbortController();
    const requestKey = Symbol("fly.browser.intent.request");
    controllers.set(requestKey, controller);
    const originalFetch = globalThis.fetch;
    let pending;
    try {
      if (typeof originalFetch === "function") {
        globalThis.fetch = (resource, init = {}) =>
          originalFetch.call(globalThis, resource, {
            ...init,
            signal: controller.signal,
          });
      }
      pending = originalPostIntent.call(this, input);
    } catch (error) {
      controllers.delete(requestKey);
      throw error;
    } finally {
      if (typeof originalFetch === "function") globalThis.fetch = originalFetch;
    }
    return Promise.resolve(pending).finally(() => {
      controllers.delete(requestKey);
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
  Adapter.prototype.stop = function stopWithBrowserHardeningCleanup() {
    const result = originalStop.call(this);
    const controllers = this[PENDING_INTENT_CONTROLLERS_KEY];
    if (controllers instanceof Map) {
      for (const controller of controllers.values()) controller.abort();
      controllers.clear();
    }
    delete this[PENDING_INTENT_CONTROLLERS_KEY];
    delete this.root.dataset.flyResourceLimited;
    this.root.querySelector(RESOURCE_STATUS_SELECTOR)?.remove();
    clearBrowserProblem(this);
    return result;
  };

  for (const root of document.querySelectorAll(ROOT_SELECTOR)) {
    installProblemReporter(root[ADAPTER_KEY]);
  }

  Object.defineProperty(Adapter.prototype, "__flyResourceGuardInstalled", {
    configurable: false,
    enumerable: false,
    value: true,
    writable: false,
  });
})();
