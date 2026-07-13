use crate::editor::{AdminCanvas, AdminShell};
use crate::AdminCanvasController;
use leptos::prelude::*;
use rustok_page_builder::dto::PageBuilderCapabilityRequest;

#[component]
pub fn PageBuilderAdmin(
    controller: AdminCanvasController,
    #[prop(optional)] on_request: Option<Callback<PageBuilderCapabilityRequest>>,
) -> impl IntoView {
    let title = format!("Edit {}", controller.page_id());
    let subtitle = Some(
        "Full Fly authoring surface. Persistence remains owned by the consumer module facade."
            .to_string(),
    );

    view! {
        <AdminShell title subtitle>
            <AdminCanvas controller on_request />
        </AdminShell>
    }
}
