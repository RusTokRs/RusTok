(() => {
  const protocol = __FLY_PROTOCOL__;
  const instanceId = __FLY_INSTANCE__;
  let sequence = 0;
  let measureScheduled = false;
  let pointerScheduled = false;
  let dragScheduled = false;
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

  const reportViewport = () => send('viewport_changed', {
    width: Math.max(0, Math.round(window.innerWidth)),
    height: Math.max(0, Math.round(window.innerHeight)),
    scroll_x: window.scrollX,
    scroll_y: window.scrollY,
    zoom: 1,
  });

  const measure = () => {
    measureScheduled = false;
    const components = Array.from(document.querySelectorAll('[data-fly-component-id]')).map((element) => {
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

  const scheduleDragPoint = (position) => {
    lastDragPoint = position;
    if (dragScheduled) return;
    dragScheduled = true;
    requestAnimationFrame(() => {
      dragScheduled = false;
      if (!lastDragPoint) return;
      send('drag_moved', { position: lastDragPoint });
    });
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
    if (pointerScheduled) return;
    pointerScheduled = true;
    requestAnimationFrame(() => {
      pointerScheduled = false;
      const kind = ['mouse', 'touch', 'pen'].includes(event.pointerType)
        ? event.pointerType
        : 'unknown';
      const position = point(event);
      send('pointer_moved', {
        sample: {
          pointer_id: event.pointerId,
          kind,
          position,
          buttons: event.buttons,
          primary: event.isPrimary,
        },
      });
      scheduleDragPoint(position);
    });
  }, { passive: true });
  document.addEventListener('pointerup', (event) => {
    send('drop_requested', { position: point(event) });
  }, { passive: true });

  document.addEventListener('dragover', (event) => {
    event.preventDefault();
    scheduleDragPoint(point(event));
  });
  document.addEventListener('drop', (event) => {
    event.preventDefault();
    send('drop_requested', { position: point(event) });
  });
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') send('cancel_drag_requested');
  });

  const observer = new ResizeObserver(scheduleMeasure);
  observer.observe(document.documentElement);
  document.querySelectorAll('[data-fly-component-id]').forEach((node) => observer.observe(node));
  window.addEventListener('resize', () => { reportViewport(); scheduleMeasure(); }, { passive: true });
  window.addEventListener('scroll', () => { reportViewport(); scheduleMeasure(); }, { passive: true });

  announce();
  setTimeout(announce, 0);
  setTimeout(announce, 100);
})();
