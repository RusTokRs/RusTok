use crate::editor::CanvasComponentGeometry;
use fly_leptos::{BrowserPoint, FLY_IFRAME_PROTOCOL, PointerSample};
use fly_ui::KeyStroke;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CanvasBridgeMessage {
    Ready,
    ViewportChanged {
        width: u32,
        height: u32,
        scroll_x: f64,
        scroll_y: f64,
        zoom: f64,
    },
    GeometrySnapshot {
        components: Vec<CanvasComponentGeometry>,
    },
    PointerMoved {
        sample: PointerSample,
    },
    DragMoved {
        position: BrowserPoint,
    },
    DropRequested {
        position: BrowserPoint,
    },
    KeyStroke {
        stroke: KeyStroke,
    },
    CancelDragRequested,
    FocusRequested {
        component_id: Option<String>,
    },
    HoverRequested {
        component_id: Option<String>,
    },
    Teardown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasBridgeEnvelope {
    pub protocol: String,
    pub instance_id: String,
    pub sequence: u64,
    pub message: CanvasBridgeMessage,
}

impl CanvasBridgeEnvelope {
    pub fn is_accepted(&self, expected_instance_id: &str, last_sequence: Option<u64>) -> bool {
        self.protocol == FLY_IFRAME_PROTOCOL
            && self.instance_id == expected_instance_id
            && last_sequence.is_none_or(|sequence| self.sequence > sequence)
    }
}

pub fn decode_canvas_message(
    payload: &str,
    expected_instance_id: &str,
    last_sequence: Option<u64>,
) -> Option<(u64, CanvasBridgeMessage)> {
    let envelope = serde_json::from_str::<CanvasBridgeEnvelope>(payload).ok()?;
    if !envelope.is_accepted(expected_instance_id, last_sequence) {
        return None;
    }
    Some((envelope.sequence, envelope.message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_ui::ModifierState;

    #[test]
    fn decoder_rejects_replay_and_cross_instance_messages() {
        let payload = serde_json::to_string(&CanvasBridgeEnvelope {
            protocol: FLY_IFRAME_PROTOCOL.to_string(),
            instance_id: "canvas-a".to_string(),
            sequence: 4,
            message: CanvasBridgeMessage::Ready,
        })
        .expect("serialize envelope");

        assert!(decode_canvas_message(&payload, "canvas-a", Some(3)).is_some());
        assert!(decode_canvas_message(&payload, "canvas-a", Some(4)).is_none());
        assert!(decode_canvas_message(&payload, "canvas-b", None).is_none());
    }

    #[test]
    fn drag_messages_round_trip() {
        let payload = serde_json::to_string(&CanvasBridgeEnvelope {
            protocol: FLY_IFRAME_PROTOCOL.to_string(),
            instance_id: "canvas-a".to_string(),
            sequence: 5,
            message: CanvasBridgeMessage::DropRequested {
                position: BrowserPoint { x: 12.0, y: 24.0 },
            },
        })
        .expect("serialize envelope");
        let (_, message) = decode_canvas_message(&payload, "canvas-a", Some(4)).expect("decode");
        assert!(matches!(message, CanvasBridgeMessage::DropRequested { .. }));
    }

    #[test]
    fn keyboard_messages_round_trip() {
        let payload = serde_json::to_string(&CanvasBridgeEnvelope {
            protocol: FLY_IFRAME_PROTOCOL.to_string(),
            instance_id: "canvas-a".to_string(),
            sequence: 6,
            message: CanvasBridgeMessage::KeyStroke {
                stroke: KeyStroke {
                    key: "s".to_string(),
                    code: Some("KeyS".to_string()),
                    modifiers: ModifierState {
                        control: true,
                        ..ModifierState::default()
                    },
                    repeat: false,
                    editing_text: false,
                },
            },
        })
        .expect("serialize envelope");
        let (_, message) = decode_canvas_message(&payload, "canvas-a", Some(5)).expect("decode");
        assert!(matches!(message, CanvasBridgeMessage::KeyStroke { .. }));
    }
}
