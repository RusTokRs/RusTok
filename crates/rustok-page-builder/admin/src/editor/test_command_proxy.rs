use crate::AdminCanvasController;
use fly::{ComponentPatch, EditorCommand};
use fly_ui::UiIntent;
use serde_json::Value;

/// Test-only compatibility surface that seeds component extensions through the
/// same validated command pipeline used by the editor UI.
pub(crate) struct EditorCommandTestProxy<'a> {
    controller: &'a mut AdminCanvasController,
}

pub(crate) struct DocumentCommandTestProxy<'a> {
    controller: &'a mut AdminCanvasController,
}

pub(crate) struct ComponentCommandTestProxy<'a> {
    pub(crate) extensions: ExtensionCommandTestProxy<'a>,
}

pub(crate) struct ExtensionCommandTestProxy<'a> {
    controller: &'a mut AdminCanvasController,
    component_id: String,
}

impl AdminCanvasController {
    pub(crate) fn editor_mut_for_tests(&mut self) -> EditorCommandTestProxy<'_> {
        EditorCommandTestProxy { controller: self }
    }
}

impl<'a> EditorCommandTestProxy<'a> {
    pub(crate) fn document_mut_for_tests(self) -> DocumentCommandTestProxy<'a> {
        DocumentCommandTestProxy {
            controller: self.controller,
        }
    }
}

impl<'a> DocumentCommandTestProxy<'a> {
    pub(crate) fn component_mut(self, component_id: &str) -> Option<ComponentCommandTestProxy<'a>> {
        self.controller
            .editor()
            .document()
            .component(component_id)?;
        Some(ComponentCommandTestProxy {
            extensions: ExtensionCommandTestProxy {
                controller: self.controller,
                component_id: component_id.to_string(),
            },
        })
    }
}

impl ExtensionCommandTestProxy<'_> {
    pub(crate) fn insert(self, name: String, value: Value) -> Option<Value> {
        let previous = self
            .controller
            .editor()
            .document()
            .component(&self.component_id)
            .and_then(|component| component.extensions.get(&name))
            .cloned();
        self.controller
            .dispatch(UiIntent::execute(EditorCommand::Patch {
                component_id: self.component_id,
                patch: ComponentPatch::default().set_field(name, value),
            }))
            .expect("test extension seed must pass the editor command pipeline");
        previous
    }
}
