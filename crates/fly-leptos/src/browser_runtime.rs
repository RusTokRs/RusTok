use crate::{BrowserRect, IframeBridgeEnvelope, IframeBridgeMessage};
use js_sys::{Array, Object};
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    Element, Event, EventTarget, HtmlIFrameElement, MessageEvent, ResizeObserver,
    ResizeObserverEntry, Window,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserRuntimeError {
    InvalidTargetOrigin,
    MissingContentWindow,
    Serialization(String),
    Browser(String),
}

impl std::fmt::Display for BrowserRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTargetOrigin => {
                formatter.write_str("iframe target origin must be explicit and must not be `*`")
            }
            Self::MissingContentWindow => formatter.write_str("iframe content window is unavailable"),
            Self::Serialization(message) => write!(formatter, "iframe message serialization failed: {message}"),
            Self::Browser(message) => write!(formatter, "browser operation failed: {message}"),
        }
    }
}

impl std::error::Error for BrowserRuntimeError {}

fn browser_error(value: JsValue) -> BrowserRuntimeError {
    BrowserRuntimeError::Browser(
        value
            .as_string()
            .unwrap_or_else(|| format!("{value:?}")),
    )
}

/// RAII event listener. Dropping the handle unregisters the exact callback from the target.
pub struct EventListenerHandle {
    target: EventTarget,
    event_name: String,
    callback: Closure<dyn FnMut(Event)>,
}

impl EventListenerHandle {
    pub fn new<E>(
        target: &EventTarget,
        event_name: impl Into<String>,
        mut handler: impl FnMut(E) + 'static,
    ) -> Result<Self, BrowserRuntimeError>
    where
        E: JsCast + 'static,
    {
        let event_name = event_name.into();
        let callback = Closure::wrap(Box::new(move |event: Event| {
            if let Ok(event) = event.dyn_into::<E>() {
                handler(event);
            }
        }) as Box<dyn FnMut(Event)>);
        target
            .add_event_listener_with_callback(&event_name, callback.as_ref().unchecked_ref())
            .map_err(browser_error)?;
        Ok(Self {
            target: target.clone(),
            event_name,
            callback,
        })
    }
}

impl Drop for EventListenerHandle {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback(
            &self.event_name,
            self.callback.as_ref().unchecked_ref(),
        );
    }
}

/// ResizeObserver wrapper that emits browser-space rectangles and disconnects on drop.
pub struct ResizeObserverHandle {
    observer: ResizeObserver,
    element: Element,
    _callback: Closure<dyn FnMut(Array, ResizeObserver)>,
}

impl ResizeObserverHandle {
    pub fn observe(
        element: &Element,
        mut handler: impl FnMut(Vec<BrowserRect>) + 'static,
    ) -> Result<Self, BrowserRuntimeError> {
        let callback = Closure::wrap(Box::new(move |entries: Array, _observer: ResizeObserver| {
            let rectangles = entries
                .iter()
                .filter_map(|value| value.dyn_into::<ResizeObserverEntry>().ok())
                .map(|entry| {
                    let rect = entry.content_rect();
                    BrowserRect {
                        left: rect.x(),
                        top: rect.y(),
                        width: rect.width(),
                        height: rect.height(),
                    }
                })
                .collect::<Vec<_>>();
            handler(rectangles);
        }) as Box<dyn FnMut(Array, ResizeObserver)>);
        let observer = ResizeObserver::new(callback.as_ref().unchecked_ref()).map_err(browser_error)?;
        observer.observe(element);
        Ok(Self {
            observer,
            element: element.clone(),
            _callback: callback,
        })
    }
}

impl Drop for ResizeObserverHandle {
    fn drop(&mut self) {
        self.observer.unobserve(&self.element);
        self.observer.disconnect();
    }
}

/// Validated parent-window message subscription for one Fly iframe instance.
pub struct WindowMessageSubscription {
    _listener: EventListenerHandle,
    last_sequence: Rc<Cell<Option<u64>>>,
}

impl WindowMessageSubscription {
    pub fn subscribe(
        window: &Window,
        expected_source: Option<&Window>,
        expected_origin: impl Into<String>,
        expected_instance_id: impl Into<String>,
        mut handler: impl FnMut(IframeBridgeEnvelope) + 'static,
    ) -> Result<Self, BrowserRuntimeError> {
        let expected_origin = expected_origin.into();
        if expected_origin.trim().is_empty() || expected_origin == "*" {
            return Err(BrowserRuntimeError::InvalidTargetOrigin);
        }
        let expected_source = expected_source.map(|window| JsValue::from(window.clone()));
        let expected_instance_id = expected_instance_id.into();
        let last_sequence = Rc::new(Cell::new(None));
        let callback_sequence = Rc::clone(&last_sequence);
        let target: EventTarget = window.clone().unchecked_into();
        let listener = EventListenerHandle::new::<MessageEvent>(&target, "message", move |event| {
            if event.origin() != expected_origin {
                return;
            }
            if let Some(expected_source) = expected_source.as_ref() {
                let Some(actual_source) = event.source() else {
                    return;
                };
                if !Object::is(actual_source.as_ref(), expected_source) {
                    return;
                }
            }
            let Some(payload) = event.data().as_string() else {
                return;
            };
            let Ok(envelope) = serde_json::from_str::<IframeBridgeEnvelope>(&payload) else {
                return;
            };
            if !envelope.is_accepted(&expected_instance_id, callback_sequence.get()) {
                return;
            }
            callback_sequence.set(Some(envelope.sequence));
            handler(envelope);
        })?;
        Ok(Self {
            _listener: listener,
            last_sequence,
        })
    }

    pub fn last_sequence(&self) -> Option<u64> {
        self.last_sequence.get()
    }
}

/// Outbound iframe port. Messages are JSON strings with an explicit origin and monotonic sequence.
pub struct IframeMessagePort {
    instance_id: String,
    target_origin: String,
    next_sequence: u64,
}

impl IframeMessagePort {
    pub fn new(
        instance_id: impl Into<String>,
        target_origin: impl Into<String>,
    ) -> Result<Self, BrowserRuntimeError> {
        let target_origin = target_origin.into();
        if target_origin.trim().is_empty() || target_origin == "*" {
            return Err(BrowserRuntimeError::InvalidTargetOrigin);
        }
        Ok(Self {
            instance_id: instance_id.into(),
            target_origin,
            next_sequence: 1,
        })
    }

    pub fn post(
        &mut self,
        iframe: &HtmlIFrameElement,
        message: IframeBridgeMessage,
    ) -> Result<u64, BrowserRuntimeError> {
        let sequence = self.next_sequence;
        let envelope = IframeBridgeEnvelope::new(&self.instance_id, sequence, message);
        let payload = serde_json::to_string(&envelope)
            .map_err(|error| BrowserRuntimeError::Serialization(error.to_string()))?;
        iframe
            .content_window()
            .ok_or(BrowserRuntimeError::MissingContentWindow)?
            .post_message(&JsValue::from_str(&payload), &self.target_origin)
            .map_err(browser_error)?;
        self.next_sequence = self.next_sequence.saturating_add(1);
        Ok(sequence)
    }

    pub fn teardown(
        &mut self,
        iframe: &HtmlIFrameElement,
    ) -> Result<u64, BrowserRuntimeError> {
        self.post(iframe, IframeBridgeMessage::Teardown)
    }
}

pub fn set_pointer_capture(
    element: &Element,
    pointer_id: i32,
) -> Result<(), BrowserRuntimeError> {
    element
        .set_pointer_capture(pointer_id)
        .map_err(browser_error)
}

pub fn release_pointer_capture(
    element: &Element,
    pointer_id: i32,
) -> Result<(), BrowserRuntimeError> {
    element
        .release_pointer_capture(pointer_id)
        .map_err(browser_error)
}
