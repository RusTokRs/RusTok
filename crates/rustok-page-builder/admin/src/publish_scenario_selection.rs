use fly::{RuntimeContextScenario, RuntimeScenarioReleaseBaseline};
use thiserror::Error;

pub const PAGE_BUILDER_PUBLISH_SCENARIO_SELECTION_FORMAT: &str =
    "page_builder_publish_scenario_selection_v1";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PublishScenarioSelectionError {
    #[error("the promoted Page Builder baseline has no runtime scenarios")]
    NoScenarios,
    #[error(
        "the promoted Page Builder baseline contains {count} runtime scenarios; select one explicitly before publish"
    )]
    SelectionRequired { count: usize },
    #[error("runtime scenario `{scenario_id}` is not part of the promoted Page Builder baseline")]
    ScenarioNotFound { scenario_id: String },
    #[error("Page Builder publish scenario browser storage failed: {0}")]
    BrowserStorage(String),
}

pub fn publish_scenario_selection_key(page_id: &str, baseline_hash: &str) -> String {
    format!(
        "{PAGE_BUILDER_PUBLISH_SCENARIO_SELECTION_FORMAT}:{}:{}",
        page_id.trim(),
        baseline_hash.trim()
    )
}

pub fn resolve_publish_scenario<'a>(
    baseline: &'a RuntimeScenarioReleaseBaseline,
    selected_scenario_id: Option<&str>,
) -> Result<&'a RuntimeContextScenario, PublishScenarioSelectionError> {
    let selected_scenario_id = selected_scenario_id
        .map(str::trim)
        .filter(|scenario_id| !scenario_id.is_empty());

    match baseline.scenarios.as_slice() {
        [] => Err(PublishScenarioSelectionError::NoScenarios),
        [scenario] => match selected_scenario_id {
            None => Ok(scenario),
            Some(selected) if selected == scenario.id => Ok(scenario),
            Some(selected) => Err(PublishScenarioSelectionError::ScenarioNotFound {
                scenario_id: selected.to_string(),
            }),
        },
        scenarios => {
            let selected =
                selected_scenario_id.ok_or(PublishScenarioSelectionError::SelectionRequired {
                    count: scenarios.len(),
                })?;
            scenarios
                .iter()
                .find(|scenario| scenario.id == selected)
                .ok_or_else(|| PublishScenarioSelectionError::ScenarioNotFound {
                    scenario_id: selected.to_string(),
                })
        }
    }
}

pub fn load_publish_scenario_selection(
    page_id: &str,
    baseline_hash: &str,
) -> Result<Option<String>, PublishScenarioSelectionError> {
    #[cfg(all(target_arch = "wasm32", feature = "browser-js"))]
    {
        let key = publish_scenario_selection_key(page_id, baseline_hash);
        let storage = web_sys::window()
            .ok_or_else(|| {
                PublishScenarioSelectionError::BrowserStorage(
                    "browser window is unavailable".to_string(),
                )
            })?
            .session_storage()
            .map_err(browser_storage_error)?
            .ok_or_else(|| {
                PublishScenarioSelectionError::BrowserStorage(
                    "session storage is unavailable".to_string(),
                )
            })?;
        return storage
            .get_item(&key)
            .map_err(browser_storage_error)
            .map(|value| {
                value.and_then(|scenario_id| {
                    let scenario_id = scenario_id.trim();
                    (!scenario_id.is_empty()).then(|| scenario_id.to_string())
                })
            });
    }

    #[cfg(not(all(target_arch = "wasm32", feature = "browser-js")))]
    {
        let _ = (page_id, baseline_hash);
        Ok(None)
    }
}

pub fn save_publish_scenario_selection(
    page_id: &str,
    baseline_hash: &str,
    scenario_id: Option<&str>,
) -> Result<(), PublishScenarioSelectionError> {
    #[cfg(all(target_arch = "wasm32", feature = "browser-js"))]
    {
        let key = publish_scenario_selection_key(page_id, baseline_hash);
        let storage = web_sys::window()
            .ok_or_else(|| {
                PublishScenarioSelectionError::BrowserStorage(
                    "browser window is unavailable".to_string(),
                )
            })?
            .session_storage()
            .map_err(browser_storage_error)?
            .ok_or_else(|| {
                PublishScenarioSelectionError::BrowserStorage(
                    "session storage is unavailable".to_string(),
                )
            })?;
        match scenario_id.map(str::trim).filter(|value| !value.is_empty()) {
            Some(scenario_id) => storage
                .set_item(&key, scenario_id)
                .map_err(browser_storage_error),
            None => storage.remove_item(&key).map_err(browser_storage_error),
        }
    }

    #[cfg(not(all(target_arch = "wasm32", feature = "browser-js")))]
    {
        let _ = (page_id, baseline_hash, scenario_id);
        Ok(())
    }
}

#[cfg(all(target_arch = "wasm32", feature = "browser-js"))]
fn browser_storage_error(value: wasm_bindgen::JsValue) -> PublishScenarioSelectionError {
    PublishScenarioSelectionError::BrowserStorage(format!("{value:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::{PageSelection, ProjectDocument, RenderPolicy, RuntimeContextScenario};
    use serde_json::json;

    fn baseline(scenarios: Vec<RuntimeContextScenario>) -> RuntimeScenarioReleaseBaseline {
        RuntimeScenarioReleaseBaseline::capture(
            "baseline",
            &ProjectDocument::new(fly::GrapesProject::default()),
            &PageSelection::Index(0),
            &RenderPolicy::default(),
            &scenarios,
        )
    }

    #[test]
    fn one_scenario_is_selected_without_browser_state() {
        let baseline = baseline(vec![RuntimeContextScenario::new(
            "production",
            "Production",
            json!({}),
        )]);
        assert_eq!(
            resolve_publish_scenario(&baseline, None).unwrap().id,
            "production"
        );
    }

    #[test]
    fn multiple_scenarios_require_an_explicit_selection() {
        let baseline = baseline(vec![
            RuntimeContextScenario::new("empty", "Empty", json!({})),
            RuntimeContextScenario::new("customer", "Customer", json!({})),
        ]);
        assert!(matches!(
            resolve_publish_scenario(&baseline, None),
            Err(PublishScenarioSelectionError::SelectionRequired { count: 2 })
        ));
        assert_eq!(
            resolve_publish_scenario(&baseline, Some("customer"))
                .unwrap()
                .id,
            "customer"
        );
    }
}
