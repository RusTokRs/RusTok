(() => {
  const protocol = __FLY_PROTOCOL__;
  const instanceId = __FLY_INSTANCE__;
  const configuredGeometryLimit = __FLY_MAX_GEOMETRY_COMPONENTS__;
  const maxGeometryComponents = Number.isSafeInteger(configuredGeometryLimit)
    && configuredGeometryLimit > 0
    ? configuredGeometryLimit
    : 4096;
  let sequence = 0;
  let measureScheduled = false;
  let pointerScheduled = false;
  let pendingPointerSample = null;
  let dragFrame = null;
  let dragEpoch = 0;
  let lastDragPoint = null;

  const send = (type, payload = {}) => {
    parent.postMessage(JSON.stringify({
      protocol,
      instance_id: instanceId,
      sequence: ++sequence,
      message: { type, ...payload },
    }), '*');
  };

  const componentAt = (target) => target instanceof Element
    ? target.closest('[data-fly-component-id]')
    : null;

  const point = (event) => ({ x: event.clientX, y: event.clientY });

  const acceptsTextInput = (target) => {
    if (!(target instanceof Element)) return false;
    const tag = target.tagName.toLowerCase();
    return ['input', 'textarea', 'select'].includes(tag)
      || target.isContentEditable
      || Boolean(target.closest('[contenteditable="true"]'));
  };

  const editorShortcut = (event, editingText) => {
    const key = event.key.toLowerCase();
    const primary = event.ctrlKey || event.metaKey;
    if (key === 'escape') return true;
    if (primary && key === 's') return true;
    if (editingText) return false;
    if (primary && ['z', 'y', 'c', 'x', 'v', 'd'].includes(key)) return true;
    if (!primary && ['delete', 'backspace'].includes(key)) return true;
    return event.altKey && ['arrowup', 'arrowdown'].includes(key);
  };

  const reportViewport = () => send('viewport_changed', {
    width: Math.max(0, Math.round(window.innerWidth)),
    height: Math.max(0, Math.round(window.innerHeight)),
    scroll_x: window.scrollX,
    scroll_y: window.scrollY,
    zoom: 1,
  });

  const measure = () => {
    measureScheduled = false;
    const nodes = document.querySelectorAll('[data-fly-component-id]');
    if (nodes.length > maxGeometryComponents) {
      send('geometry_snapshot', {
        components: [],
        resource_limit: {
          kind: 'geometry_components',
          limit: maxGeometryComponents,
          observed: nodes.length,
        },
      });
      return;
    }
    const components = Array.from(nodes).map((element) => {
      const rect = element.getBoundingClientRect();
      const parentElement = element.parentElement?.closest('[data-fly-component-id]');
      return {
        component_id: element.dataset.flyComponentId,
        parent_component_id: parentElement?.dataset.flyComponentId ?? null,
        index: Number.parseInt(element.dataset.flyIndex ?? '0', 10) || 0,
        rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
      };
    });
    send('geometry_snapshot', { components });
  };

  const scheduleMeasure = () => {
    if (measureScheduled) return;
    measureScheduled = true;
    requestAnimationFrame(measure);
  };

  const cancelScheduledDrag = () => {
    dragEpoch += 1;
    lastDragPoint = null;
    if (dragFrame !== null) {
      cancelAnimationFrame(dragFrame);
      dragFrame = null;
    }
  };

  const scheduleDragPoint = (position, epoch = dragEpoch) => {
    if (epoch !== dragEpoch) return;
    lastDragPoint = { position, epoch };
    if (dragFrame !== null) return;
    dragFrame = requestAnimationFrame(() => {
      dragFrame = null;
      const sample = lastDragPoint;
      lastDragPoint = null;
      if (!sample || sample.epoch !== dragEpoch) return;
      send('drag_moved', { position: sample.position });
    });
  };

  const finishDrag = (position) => {
    cancelScheduledDrag();
    send('drop_requested', { position });
  };

  const announce = () => {
    reportViewport();
    scheduleMeasure();
    send('ready');
  };

  document.addEventListener('click', (event) => {
    const component = componentAt(event.target);
    document.querySelectorAll('[data-fly-selected]')
      .forEach((node) => node.removeAttribute('data-fly-selected'));
    if (component) component.setAttribute('data-fly-selected', 'true');
    send('focus_requested', { component_id: component?.dataset.flyComponentId ?? null });
  });

  document.addEventListener('pointerover', (event) => {
    const component = componentAt(event.target);
    send('hover_requested', { component_id: component?.dataset.flyComponentId ?? null });
  });
  document.addEventListener('pointerleave', () => {
    send('hover_requested', { component_id: null });
  });
  document.addEventListener('pointermove', (event) => {
    pendingPointerSample = {
      drag_epoch: dragEpoch,
      pointer_id: event.pointerId,
      kind: ['mouse', 'touch', 'pen'].includes(event.pointerType)
        ? event.pointerType
        : 'unknown',
      position: point(event),
      buttons: event.buttons,
      primary: event.isPrimary,
    };
    if (pointerScheduled) return;
    pointerScheduled = true;
    requestAnimationFrame(() => {
      pointerScheduled = false;
      const pending = pendingPointerSample;
      pendingPointerSample = null;
      if (!pending) return;
      const { drag_epoch: epoch, ...sample } = pending;
      send('pointer_moved', { sample });
      scheduleDragPoint(sample.position, epoch);
    });
  }, { passive: true });
  document.addEventListener('pointerup', (event) => {
    finishDrag(point(event));
  }, { passive: true });
  document.addEventListener('pointercancel', () => {
    cancelScheduledDrag();
    send('cancel_drag_requested');
  }, { passive: true });

  document.addEventListener('dragover', (event) => {
    event.preventDefault();
    scheduleDragPoint(point(event));
  });
  document.addEventListener('drop', (event) => {
    event.preventDefault();
    finishDrag(point(event));
  });
  document.addEventListener('keydown', (event) => {
    const editingText = acceptsTextInput(event.target);
    if (editorShortcut(event, editingText)) event.preventDefault();
    send('key_stroke', {
      stroke: {
        key: event.key,
        code: event.code || null,
        modifiers: {
          shift: event.shiftKey,
          alt: event.altKey,
          control: event.ctrlKey,
          meta: event.metaKey,
        },
        repeat: event.repeat,
        editing_text: editingText,
      },
    });
  });

  const observer = new ResizeObserver(scheduleMeasure);
  observer.observe(document.documentElement);
  document.querySelectorAll('[data-fly-component-id]').forEach((node) => observer.observe(node));
  document.addEventListener('scroll', scheduleMeasure, { capture: true, passive: true });
  window.addEventListener('resize', () => { reportViewport(); scheduleMeasure(); }, { passive: true });
  window.addEventListener('scroll', () => { reportViewport(); scheduleMeasure(); }, { passive: true });

  announce();
  setTimeout(announce, 0);
  setTimeout(announce, 100);
})();
