use crate::editor::CanvasComponentGeometry;
use fly_ui::KeyStroke;
use fly_web::{BrowserPoint, FLY_IFRAME_PROTOCOL, MAX_IFRAME_MESSAGE_BYTES, PointerSample};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanvasResourceLimit {
    pub kind: String,
    pub limit: usize,
    pub observed: usize,
}

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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resource_limit: Option<CanvasResourceLimit>,
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
    if payload.len() > MAX_IFRAME_MESSAGE_BYTES {
        return None;
    }
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
    fn geometry_snapshot_preserves_resource_limit_metadata() {
        let payload = serde_json::json!({
            "type": "geometry_snapshot",
            "components": [],
            "resource_limit": {
                "kind": "geometry_components",
                "limit": 4096,
                "observed": 4097
            }
        });
        let message: CanvasBridgeMessage =
            serde_json::from_value(payload).expect("geometry snapshot");
        assert_eq!(
            message,
            CanvasBridgeMessage::GeometrySnapshot {
                components: Vec::new(),
                resource_limit: Some(CanvasResourceLimit {
                    kind: "geometry_components".to_string(),
                    limit: 4096,
                    observed: 4097,
                }),
            }
        );
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

    #[test]
    fn decoder_rejects_oversized_messages_before_deserialization() {
        let envelope = CanvasBridgeEnvelope {
            protocol: FLY_IFRAME_PROTOCOL.to_string(),
            instance_id: "canvas-a".to_string(),
            sequence: 1,
            message: CanvasBridgeMessage::Ready,
        };
        let payload = serde_json::to_string(&envelope).expect("serialize envelope");
        // serde_json accepts trailing whitespace, so without the byte guard this remains valid.
        let oversized = format!(
            "{payload}{}",
            " ".repeat(MAX_IFRAME_MESSAGE_BYTES.saturating_sub(payload.len()) + 1)
        );
        assert!(oversized.len() > MAX_IFRAME_MESSAGE_BYTES);
        assert!(decode_canvas_message(&oversized, "canvas-a", None).is_none());
    }
}
