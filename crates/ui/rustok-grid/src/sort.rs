use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub fn opposite(&self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortState {
    pub column_id: Option<String>,
    pub direction: Option<SortDirection>,
}

impl SortState {
    pub fn new(column_id: impl Into<String>, direction: SortDirection) -> Self {
        Self {
            column_id: Some(column_id.into()),
            direction: Some(direction),
        }
    }

    pub fn is_sorted_by(&self, column_id: &str) -> Option<SortDirection> {
        if self.column_id.as_deref() == Some(column_id) {
            self.direction
        } else {
            None
        }
    }

    /// Cycles through: None -> Asc -> Desc -> None
    pub fn toggle(&mut self, column_id: &str) {
        if self.column_id.as_deref() == Some(column_id) {
            match self.direction {
                Some(SortDirection::Asc) => {
                    self.direction = Some(SortDirection::Desc);
                }
                Some(SortDirection::Desc) => {
                    self.column_id = None;
                    self.direction = None;
                }
                None => {
                    self.direction = Some(SortDirection::Asc);
                }
            }
        } else {
            self.column_id = Some(column_id.to_string());
            self.direction = Some(SortDirection::Asc);
        }
    }

    pub fn clear(&mut self) {
        self.column_id = None;
        self.direction = None;
    }
}
