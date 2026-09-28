use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaginationMode {
    Paged,
    Infinite,
}

impl Default for PaginationMode {
    fn default() -> Self {
        Self::Paged
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridPagination {
    pub page: usize,
    pub page_size: usize,
    pub total: u64,
    pub has_next: bool,
    pub mode: PaginationMode,
}

impl Default for GridPagination {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 20,
            total: 0,
            has_next: false,
            mode: PaginationMode::Paged,
        }
    }
}

impl GridPagination {
    pub fn new(page: usize, page_size: usize, total: u64) -> Self {
        let has_next = (page * page_size) < total as usize;
        Self {
            page,
            page_size,
            total,
            has_next,
            mode: PaginationMode::Paged,
        }
    }

    pub fn total_pages(&self) -> usize {
        if self.page_size == 0 {
            return 1;
        }
        ((self.total as f64) / (self.page_size as f64)).ceil() as usize
    }

    pub fn has_previous(&self) -> bool {
        self.page > 1
    }

    pub fn from_index(&self) -> usize {
        if self.total == 0 {
            0
        } else if self.mode == PaginationMode::Infinite {
            1
        } else {
            (self.page - 1) * self.page_size + 1
        }
    }

    pub fn to_index(&self) -> usize {
        let max_in_page = self.page * self.page_size;
        if max_in_page > self.total as usize {
            self.total as usize
        } else {
            max_in_page
        }
    }

    pub fn set_page(&mut self, next_page: usize) {
        let max = self.total_pages().max(1);
        self.page = next_page.clamp(1, max);
        self.has_next = (self.page * self.page_size) < self.total as usize;
    }

    pub fn set_page_size(&mut self, size: usize) {
        self.page_size = size.max(1);
        self.set_page(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_bounds() {
        let mut p = GridPagination::new(1, 20, 55);
        assert_eq!(p.total_pages(), 3);
        assert_eq!(p.from_index(), 1);
        assert_eq!(p.to_index(), 20);
        assert!(p.has_next);
        assert!(!p.has_previous());

        p.set_page(3);
        assert_eq!(p.from_index(), 41);
        assert_eq!(p.to_index(), 55);
        assert!(!p.has_next);
        assert!(p.has_previous());
    }
}
