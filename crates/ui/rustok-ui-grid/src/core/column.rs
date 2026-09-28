use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColumnId(pub String);

impl ColumnId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<S: Into<String>> From<S> for ColumnId {
    fn from(s: S) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for ColumnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnAlign {
    Left,
    Center,
    Right,
}

impl Default for ColumnAlign {
    fn default() -> Self {
        Self::Left
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnWidth {
    pub min: u32,
    pub max: u32,
    pub current: u32,
}

impl Default for ColumnWidth {
    fn default() -> Self {
        Self {
            min: 50,
            max: 800,
            current: 150,
        }
    }
}

impl ColumnWidth {
    pub fn fixed(px: u32) -> Self {
        Self {
            min: px,
            max: px,
            current: px,
        }
    }

    pub fn resizable(default_px: u32, min_px: u32, max_px: u32) -> Self {
        Self {
            min: min_px,
            max: max_px,
            current: default_px.clamp(min_px, max_px),
        }
    }

    pub fn clamp(&self, px: u32) -> u32 {
        px.clamp(self.min, self.max)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinnedSide {
    Left,
    Right,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GridColumnDef {
    pub id: ColumnId,
    pub title: String,
    pub align: ColumnAlign,
    pub width: ColumnWidth,
    pub sortable: bool,
    pub resizable: bool,
    pub filterable: bool,
    pub visible: bool,
    pub pinned: Option<PinnedSide>,
    pub filter_type: Option<super::filter::GridFilterType>,
}

impl GridColumnDef {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: ColumnId::new(id),
            title: title.into(),
            align: ColumnAlign::Left,
            width: ColumnWidth::default(),
            sortable: true,
            resizable: true,
            filterable: true,
            visible: true,
            pinned: None,
            filter_type: None,
        }
    }

    pub fn checkbox() -> Self {
        Self {
            id: ColumnId::new("__checkbox"),
            title: String::new(),
            align: ColumnAlign::Center,
            width: ColumnWidth::fixed(44),
            sortable: false,
            resizable: false,
            filterable: false,
            visible: true,
            pinned: Some(PinnedSide::Left),
            filter_type: None,
        }
    }

    pub fn actions(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: ColumnId::new(id),
            title: title.into(),
            align: ColumnAlign::Right,
            width: ColumnWidth::fixed(80),
            sortable: false,
            resizable: false,
            filterable: false,
            visible: true,
            pinned: Some(PinnedSide::Right),
            filter_type: None,
        }
    }

    pub fn width(mut self, px: u32) -> Self {
        self.width = ColumnWidth {
            min: 50,
            max: 1200,
            current: px,
        };
        self
    }

    pub fn min_width(mut self, min_px: u32) -> Self {
        self.width.min = min_px;
        if self.width.current < min_px {
            self.width.current = min_px;
        }
        self
    }

    pub fn max_width(mut self, max_px: u32) -> Self {
        self.width.max = max_px;
        if self.width.current > max_px {
            self.width.current = max_px;
        }
        self
    }

    pub fn not_resizable(mut self) -> Self {
        self.resizable = false;
        self
    }

    pub fn not_sortable(mut self) -> Self {
        self.sortable = false;
        self
    }

    pub fn align(mut self, align: ColumnAlign) -> Self {
        self.align = align;
        self
    }

    pub fn filter(mut self, filter_type: super::filter::GridFilterType) -> Self {
        self.filterable = true;
        self.filter_type = Some(filter_type);
        self
    }

    pub fn not_filterable(mut self) -> Self {
        self.filterable = false;
        self.filter_type = None;
        self
    }

    pub fn pin(mut self, side: PinnedSide) -> Self {
        self.pinned = Some(side);
        self
    }
}
