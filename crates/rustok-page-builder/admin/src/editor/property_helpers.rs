#![allow(dead_code)]

use crate::editor::AdminEditorRuntime;
use fly::{ComponentPatch, EditorCommand};
use fly_ui::UiIntent;
use leptos::prelude::With;
use serde_json::Value;

pub(crate) fn selected_patch(
    runtime: &AdminEditorRuntime,
    patch: ComponentPatch,
) -> Result<UiIntent, String> {
    let component_id = runtime
        .controller
        .with(|controller| controller.ui().state.selection.component_id.clone())
        .ok_or_else(|| "select a component before editing properties".to_string())?;
    Ok(UiIntent::execute(EditorCommand::Patch {
        component_id,
        patch,
    }))
}

pub(crate) fn selected_style_value(runtime: &AdminEditorRuntime, property: &str) -> String {
    runtime.controller.with(|controller| {
        controller
            .selected_component_view()
            .and_then(|selected| selected.style)
            .and_then(|style| style.as_object().cloned())
            .and_then(|style| style.get(property).and_then(scalar_string))
            .unwrap_or_default()
    })
}

pub(crate) fn parse_scalar(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

fn scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}
