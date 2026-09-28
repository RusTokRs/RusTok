/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

#[cfg(test)]
mod tests {
    use dioxus::prelude::*;
    use rustok_ui::*;

    use crate::alert::*;
    use crate::avatar::*;
    use crate::badge::*;
    use crate::button::*;
    use crate::card::*;
    use crate::dialog::*;
    use crate::skeleton::*;
    use crate::tabs::*;

    #[test]
    fn test_dioxus_button_instantiation() {
        let mut dom = VirtualDom::new(|| {
            rsx! {
                Button {
                    variant: ButtonVariant::Destructive,
                    size: Size::Sm,
                    "Delete"
                }
            }
        });
        dom.rebuild_in_place();
    }

    #[test]
    fn test_dioxus_alert_instantiation() {
        let mut dom = VirtualDom::new(|| {
            rsx! {
                Alert {
                    variant: AlertVariant::Warning,
                    title: "Warning".to_string(),
                    "Watch out"
                }
            }
        });
        dom.rebuild_in_place();
    }

    #[test]
    fn test_dioxus_badge_instantiation() {
        let mut dom = VirtualDom::new(|| {
            rsx! {
                Badge {
                    variant: BadgeVariant::Success,
                    "Active"
                }
            }
        });
        dom.rebuild_in_place();
    }

    #[test]
    fn test_dioxus_avatar_and_skeleton() {
        let mut dom = VirtualDom::new(|| {
            rsx! {
                Avatar {
                    fallback: "John Doe".to_string(),
                }
                Skeleton {
                    variant: SkeletonVariant::Circular,
                }
            }
        });
        dom.rebuild_in_place();
    }

    #[test]
    fn test_dioxus_card_and_tabs_and_dialog() {
        let mut dom = VirtualDom::new(|| {
            rsx! {
                Card {
                    CardHeader {
                        CardTitle { "Title" }
                        CardDescription { "Desc" }
                    }
                    CardContent { "Content" }
                }
                Tabs {
                    TabsList {
                        TabsTrigger { active: true, "T1" }
                    }
                    TabsContent { active: true, "C1" }
                }
                Dialog {
                    open: true,
                    DialogTitle { "Dialog Title" }
                }
            }
        });
        dom.rebuild_in_place();
    }
}
