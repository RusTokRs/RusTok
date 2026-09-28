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
        let mut result = Self { page: page.max(1), page_size: page_size.max(1), total, has_next: false, mode: PaginationMode::Paged };
        result.set_page(result.page);
        result
    }

    pub fn total_pages(&self) -> usize {
        if self.page_size == 0 { return 1; }
        // Integer arithmetic avoids lossy u64 -> f64 conversion for large data sets.
        self.total.checked_add(self.page_size as u64 - 1)
            .map(|n| (n / self.page_size as u64) as usize)
            .unwrap_or(usize::MAX)
            .max(1)
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
            self.page.saturating_sub(1).saturating_mul(self.page_size).saturating_add(1)
        }
    }

    pub fn to_index(&self) -> usize {
        let max_in_page = self.page.saturating_mul(self.page_size);
        max_in_page.min(self.total.min(usize::MAX as u64) as usize)
    }

    pub fn set_page(&mut self, next_page: usize) {
        let max = self.total_pages().max(1);
        self.page = next_page.max(1).min(max);
        self.has_next = (self.page as u64).saturating_mul(self.page_size as u64) < self.total;
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

#[cfg(test)]
mod edge_tests {
    use super::*;

    #[test]
    fn pagination_normalizes_invalid_inputs_without_overflow() {
        let mut p = GridPagination::new(0, 0, u64::MAX);
        assert_eq!(p.page, 1);
        assert_eq!(p.page_size, 1);
        p.set_page(usize::MAX);
        assert_eq!(p.page, usize::MAX);
        assert!(!p.has_next);
    }
}
