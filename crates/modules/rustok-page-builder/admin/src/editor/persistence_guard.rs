//! Protects unsaved editor work and exposes the persistence status to the consumer host.
//!
//! - **Status mirror.** A host that provides a [`PageBuilderEditorStatusSignal`] context before
//!   mounting the editor receives live `dirty / saving / failed` updates. Pages uses it to block
//!   publishing a stale server revision while the editor holds unsaved changes.
//! - **Autosave.** When enabled by [`PageBuilderAutosavePolicy`], a debounced save is requested
//!   after the author stops editing. Autosave never retries a failed save on its own (a revision
//!   conflict must not loop) and never saves while blocking diagnostics are present.
//! - **Navigation guard.** While changes are unsaved or a save is in flight, closing or reloading
//!   the tab asks the browser to confirm.

use crate::editor::AdminEditorRuntime;
use leptos::prelude::*;

/// Persistence status of the mounted editor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PageBuilderEditorStatus {
    /// The document has changes that the host store has not acknowledged.
    pub dirty: bool,
    /// A save request is in flight.
    pub save_in_progress: bool,
    /// The last save request failed.
    pub save_failed: bool,
    /// Whether the current editor capabilities allow saving at all.
    pub can_save: bool,
}

impl PageBuilderEditorStatus {
    /// True when leaving the editor now would lose work.
    pub const fn has_unsaved_changes(self) -> bool {
        self.dirty || self.save_in_progress
    }
}

/// Host-provided signal mirroring [`PageBuilderEditorStatus`]. Provide it as a Leptos context
/// before mounting `PageBuilderAdmin` to observe the editor from the surrounding host UI.
#[derive(Debug, Clone, Copy)]
pub struct PageBuilderEditorStatusSignal(pub RwSignal<PageBuilderEditorStatus>);

impl PageBuilderEditorStatusSignal {
    pub fn new() -> Self {
        Self(RwSignal::new(PageBuilderEditorStatus::default()))
    }
}

impl Default for PageBuilderEditorStatusSignal {
    fn default() -> Self {
        Self::new()
    }
}

/// Debounced autosave policy. Provide it as a Leptos context to override the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageBuilderAutosavePolicy {
    pub enabled: bool,
    /// Quiet period after the last edit before a save is requested.
    pub debounce_ms: u32,
}

/// Lower bound for the debounce window so autosave cannot hammer the store.
pub const PAGE_BUILDER_AUTOSAVE_MIN_DEBOUNCE_MS: u32 = 500;
pub const PAGE_BUILDER_AUTOSAVE_DEFAULT_DEBOUNCE_MS: u32 = 2_500;

impl Default for PageBuilderAutosavePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            debounce_ms: PAGE_BUILDER_AUTOSAVE_DEFAULT_DEBOUNCE_MS,
        }
    }
}

impl PageBuilderAutosavePolicy {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            debounce_ms: PAGE_BUILDER_AUTOSAVE_DEFAULT_DEBOUNCE_MS,
        }
    }

    pub fn effective_debounce_ms(self) -> u32 {
        self.debounce_ms.max(PAGE_BUILDER_AUTOSAVE_MIN_DEBOUNCE_MS)
    }
}

/// Inputs to the autosave decision, extracted from the editor state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutosaveInputs {
    pub dirty: bool,
    pub save_in_progress: bool,
    pub save_failed: bool,
    pub can_save: bool,
    pub blocking_diagnostics: bool,
}

/// Pure autosave decision, kept separate from the browser timer so it is testable on any target.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn should_autosave(policy: PageBuilderAutosavePolicy, inputs: AutosaveInputs) -> bool {
    policy.enabled
        && inputs.dirty
        && inputs.can_save
        && !inputs.save_in_progress
        && !inputs.save_failed
        && !inputs.blocking_diagnostics
}

fn read_status(runtime: &AdminEditorRuntime) -> (PageBuilderEditorStatus, AutosaveInputs, u64) {
    runtime.controller.with(|controller| {
        let state = &controller.ui().state;
        let status = PageBuilderEditorStatus {
            dirty: state.dirty.dirty,
            save_in_progress: state.dirty.save_in_progress,
            save_failed: state.dirty.save_failed,
            can_save: state.capabilities.publish,
        };
        let inputs = AutosaveInputs {
            dirty: state.dirty.dirty,
            save_in_progress: state.dirty.save_in_progress,
            save_failed: state.dirty.save_failed,
            can_save: state.capabilities.publish,
            blocking_diagnostics: state.has_blocking_diagnostics(),
        };
        (status, inputs, state.dirty.command_sequence)
    })
}

#[component]
pub fn EditorPersistenceGuard(runtime: AdminEditorRuntime) -> impl IntoView {
    let sink = use_context::<PageBuilderEditorStatusSignal>();
    let policy = use_context::<PageBuilderAutosavePolicy>().unwrap_or_default();

    let mirror_runtime = runtime.clone();
    Effect::new(move |_| {
        let (status, _, _) = read_status(&mirror_runtime);
        if let Some(sink) = sink
            && sink.0.try_get_untracked() != Some(status)
        {
            let _ = sink.0.try_set(status);
        }
    });

    #[cfg(target_arch = "wasm32")]
    {
        use std::time::Duration;

        let pending = StoredValue::new_local(None::<TimeoutHandle>);
        let autosave_runtime = runtime.clone();
        Effect::new(move |_| {
            let (_, inputs, sequence) = read_status(&autosave_runtime);
            if let Some(handle) = pending.try_get_value().flatten() {
                handle.clear();
            }
            pending.set_value(None);
            if !should_autosave(policy, inputs) {
                return;
            }
            let runtime = autosave_runtime.clone();
            let handle = set_timeout_with_handle(
                move || {
                    // Re-check at fire time: the author may have saved manually or kept typing.
                    let still_due = runtime
                        .controller
                        .try_with_untracked(|controller| {
                            let state = &controller.ui().state;
                            state.dirty.command_sequence == sequence
                                && should_autosave(
                                    policy,
                                    AutosaveInputs {
                                        dirty: state.dirty.dirty,
                                        save_in_progress: state.dirty.save_in_progress,
                                        save_failed: state.dirty.save_failed,
                                        can_save: state.capabilities.publish,
                                        blocking_diagnostics: state.has_blocking_diagnostics(),
                                    },
                                )
                        })
                        .unwrap_or(false);
                    if still_due {
                        runtime.dispatch(fly_ui::UiIntent::RequestSave);
                    }
                },
                Duration::from_millis(u64::from(policy.effective_debounce_ms())),
            )
            .ok();
            pending.set_value(handle);
        });

        let unload_runtime = runtime.clone();
        let listener = window_event_listener(leptos::ev::beforeunload, move |event| {
            let unsaved = unload_runtime
                .controller
                .try_with_untracked(|controller| {
                    let dirty = &controller.ui().state.dirty;
                    dirty.dirty || dirty.save_in_progress
                })
                .unwrap_or(false);
            if unsaved {
                event.prevent_default();
                event.set_return_value("");
            }
        });
        on_cleanup(move || listener.remove());
    }

    let status_runtime = runtime;
    view! {
        <span
            class="sr-only"
            data-fly-persistence-guard="true"
            data-fly-autosave=if policy.enabled { "on" } else { "off" }
            data-fly-unsaved=move || {
                if read_status(&status_runtime).0.has_unsaved_changes() { "true" } else { "false" }
            }
        ></span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> AutosaveInputs {
        AutosaveInputs {
            dirty: true,
            save_in_progress: false,
            save_failed: false,
            can_save: true,
            blocking_diagnostics: false,
        }
    }

    #[test]
    fn autosaves_only_clean_dirty_documents() {
        let policy = PageBuilderAutosavePolicy::default();
        assert!(should_autosave(policy, inputs()));
        assert!(!should_autosave(
            PageBuilderAutosavePolicy::disabled(),
            inputs()
        ));
        assert!(!should_autosave(
            policy,
            AutosaveInputs {
                dirty: false,
                ..inputs()
            }
        ));
        assert!(!should_autosave(
            policy,
            AutosaveInputs {
                can_save: false,
                ..inputs()
            }
        ));
        assert!(!should_autosave(
            policy,
            AutosaveInputs {
                save_in_progress: true,
                ..inputs()
            }
        ));
        // A failed save (for example a revision conflict) must not be retried in a loop.
        assert!(!should_autosave(
            policy,
            AutosaveInputs {
                save_failed: true,
                ..inputs()
            }
        ));
        assert!(!should_autosave(
            policy,
            AutosaveInputs {
                blocking_diagnostics: true,
                ..inputs()
            }
        ));
    }

    #[test]
    fn debounce_has_a_floor() {
        let policy = PageBuilderAutosavePolicy {
            enabled: true,
            debounce_ms: 10,
        };
        assert_eq!(
            policy.effective_debounce_ms(),
            PAGE_BUILDER_AUTOSAVE_MIN_DEBOUNCE_MS
        );
    }

    #[test]
    fn unsaved_changes_include_in_flight_saves() {
        assert!(
            PageBuilderEditorStatus {
                save_in_progress: true,
                ..PageBuilderEditorStatus::default()
            }
            .has_unsaved_changes()
        );
        assert!(!PageBuilderEditorStatus::default().has_unsaved_changes());
    }
}
