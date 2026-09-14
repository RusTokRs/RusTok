/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use serde::{Deserialize, Serialize};

/// Framework-agnostic pagination state for table and list views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiPaginationState {
    pub page: u32,
    pub per_page: u32,
    pub total_items: Option<u64>,
}

impl Default for UiPaginationState {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 20,
            total_items: None,
        }
    }
}

impl UiPaginationState {
    pub fn new(page: u32, per_page: u32, total_items: Option<u64>) -> Self {
        Self {
            page: page.max(1),
            per_page: per_page.max(1),
            total_items,
        }
    }

    pub fn offset(&self) -> u64 {
        ((self.page - 1) as u64) * (self.per_page as u64)
    }

    pub fn total_pages(&self) -> Option<u32> {
        self.total_items.map(|total| {
            if total == 0 {
                1
            } else {
                ((total + (self.per_page as u64) - 1) / (self.per_page as u64)) as u32
            }
        })
    }

    pub fn has_next(&self) -> bool {
        match self.total_pages() {
            Some(total) => self.page < total,
            None => true,
        }
    }

    pub fn has_previous(&self) -> bool {
        self.page > 1
    }

    pub fn next_page(&self) -> Option<Self> {
        if self.has_next() {
            Some(Self {
                page: self.page + 1,
                ..*self
            })
        } else {
            None
        }
    }

    pub fn previous_page(&self) -> Option<Self> {
        if self.has_previous() {
            Some(Self {
                page: self.page - 1,
                ..*self
            })
        } else {
            None
        }
    }
}

/// Framework-agnostic sort direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSortDirection {
    #[default]
    Asc,
    Desc,
}

impl UiSortDirection {
    pub fn toggle(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

/// Framework-agnostic sort state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSortState {
    pub field: Option<String>,
    pub direction: UiSortDirection,
}

impl UiSortState {
    pub fn new(field: impl Into<String>, direction: UiSortDirection) -> Self {
        Self {
            field: Some(field.into()),
            direction,
        }
    }

    pub fn sort_by(&mut self, field: &str) {
        if let Some(current) = &self.field
            && current == field
        {
            self.direction = self.direction.toggle();
        } else {
            self.field = Some(field.to_string());
            self.direction = UiSortDirection::Asc;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_calculates_offset_and_pages() {
        let state = UiPaginationState::new(1, 10, Some(25));
        assert_eq!(state.offset(), 0);
        assert_eq!(state.total_pages(), Some(3));
        assert!(state.has_next());
        assert!(!state.has_previous());

        let page2 = state.next_page().expect("has page 2");
        assert_eq!(page2.page, 2);
        assert_eq!(page2.offset(), 10);
        assert!(page2.has_next());
        assert!(page2.has_previous());

        let page3 = page2.next_page().expect("has page 3");
        assert_eq!(page3.page, 3);
        assert_eq!(page3.offset(), 20);
        assert!(!page3.has_next());
        assert!(page3.has_previous());
        assert_eq!(page3.next_page(), None);
    }

    #[test]
    fn pagination_handles_empty_dataset() {
        let state = UiPaginationState::new(1, 20, Some(0));
        assert_eq!(state.offset(), 0);
        assert_eq!(state.total_pages(), Some(1));
        assert!(!state.has_next());
        assert!(!state.has_previous());
    }

    #[test]
    fn sort_state_toggles_direction_when_same_field() {
        let mut sort = UiSortState::default();
        assert_eq!(sort.field, None);
        assert_eq!(sort.direction, UiSortDirection::Asc);

        sort.sort_by("created_at");
        assert_eq!(sort.field.as_deref(), Some("created_at"));
        assert_eq!(sort.direction, UiSortDirection::Asc);

        sort.sort_by("created_at");
        assert_eq!(sort.direction, UiSortDirection::Desc);

        sort.sort_by("name");
        assert_eq!(sort.field.as_deref(), Some("name"));
        assert_eq!(sort.direction, UiSortDirection::Asc);
    }
}
