use fly_leptos::{BrowserRect, PointerSample, FLY_IFRAME_PROTOCOL_V1};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasComponentGeometry {
    pub component_id: String,
    pub parent_component_id: Option<String>,
    pub index: usize,
    pub rect: BrowserRect,
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
    },
    PointerMoved {
        sample: PointerSample,
    },
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
        self.protocol == FLY_IFRAME_PROTOCOL_V1
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

    #[test]
    fn decoder_rejects_replay_and_cross_instance_messages() {
        let payload = serde_json::to_string(&CanvasBridgeEnvelope {
            protocol: FLY_IFRAME_PROTOCOL_V1.to_string(),
            instance_id: "canvas-a".to_string(),
            sequence: 4,
            message: CanvasBridgeMessage::Ready,
        })
        .expect("serialize envelope");

        assert!(decode_canvas_message(&payload, "canvas-a", Some(3)).is_some());
        assert!(decode_canvas_message(&payload, "canvas-a", Some(4)).is_none());
        assert!(decode_canvas_message(&payload, "canvas-b", None).is_none());
    }
}
