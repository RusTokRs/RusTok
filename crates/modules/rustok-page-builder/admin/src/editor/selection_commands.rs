use crate::AdminCanvasController;
use fly::{AssetCatalog, EditorCommand};
use fly_ui::UiIntent;

impl AdminCanvasController {
    pub fn move_selected_up_intent(&self) -> Result<UiIntent, String> {
        self.move_selected_relative_intent(-1)
    }

    pub fn move_selected_down_intent(&self) -> Result<UiIntent, String> {
        self.move_selected_relative_intent(1)
    }

    pub fn apply_asset_to_selected_intent(
        &self,
        asset_id: &str,
        source_attribute: &str,
    ) -> Result<UiIntent, String> {
        let selected = self
            .selected_component_view()
            .ok_or_else(|| "select a component before assigning an asset".to_string())?;
        let catalog = AssetCatalog::from_document(self.editor().document());
        let asset = catalog
            .get(asset_id)
            .ok_or_else(|| format!("asset `{asset_id}` does not exist"))?;
        let patch = asset
            .component_patch(source_attribute)
            .map_err(|error| error.to_string())?;
        Ok(UiIntent::execute(EditorCommand::Patch {
            component_id: selected.id,
            patch,
        }))
    }

    fn move_selected_relative_intent(&self, direction: isize) -> Result<UiIntent, String> {
        let selected = self
            .selected_component_view()
            .ok_or_else(|| "select a component before reordering it".to_string())?;
        if selected.is_root {
            return Err("the page root cannot be reordered".to_string());
        }
        let location = self
            .editor()
            .document()
            .component_location(&selected.id)
            .ok_or_else(|| "selected component has no location".to_string())?;
        let child_count = self
            .editor()
            .document()
            .child_count_for_parent(location.parent_component_id.as_deref())
            .ok_or_else(|| "selected component parent is missing or opaque".to_string())?;

        let index = match direction.cmp(&0) {
            std::cmp::Ordering::Less => {
                if location.index == 0 {
                    return Err("selected component is already first".to_string());
                }
                location.index - 1
            }
            std::cmp::Ordering::Greater => {
                if location.index + 1 >= child_count {
                    return Err("selected component is already last".to_string());
                }
                location.index + 2
            }
            std::cmp::Ordering::Equal => {
                return Err("reorder direction must not be zero".to_string());
            }
        };

        Ok(UiIntent::execute(EditorCommand::Move {
            component_id: selected.id,
            new_parent_id: location.parent_component_id,
            index,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_ui::UiIntent;
    use serde_json::json;

    fn controller() -> AdminCanvasController {
        let mut controller = AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [
                            { "id": "a", "type": "section" },
                            { "id": "b", "type": "section" },
                            { "id": "c", "type": "section" }
                        ]
                    }
                }]
            }),
        )
        .expect("controller");
        controller
            .dispatch(UiIntent::Select(Some("b".to_string())))
            .expect("select");
        controller
    }

    #[test]
    fn down_intent_accounts_for_remove_then_insert_index() {
        let controller = controller();
        let intent = controller.move_selected_down_intent().expect("move");
        let UiIntent::Execute(command) = intent else {
            panic!("expected move command");
        };
        let EditorCommand::Move { index, .. } = *command else {
            panic!("expected move command");
        };
        assert_eq!(index, 3);
    }
}
