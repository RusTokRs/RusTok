//! Pagination state for both classic paged navigation and infinite scroll.
//!
//! All arithmetic is overflow-safe and integer-only: totals are `u64` (a data
//! set may exceed `usize` on 32-bit wasm targets) and page indexes are
//! one-based for display.

use serde::{Deserialize, Serialize};

/// How the grid consumes pages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaginationMode {
    /// Classic previous/next navigation, one page rendered at a time.
    #[default]
    Paged,
    /// Cumulative rendering: page N shows pages 1..=N.
    Infinite,
}

/// Paging state. `page` is one-based and always within `1..=total_pages()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridPagination {
    pub page: usize,
    pub page_size: usize,
    pub total: u64,
    pub has_next: bool,
    pub mode: PaginationMode,
}

/// Fallback page size used when a caller passes `0`.
pub const DEFAULT_PAGE_SIZE: usize = 20;

impl Default for GridPagination {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: DEFAULT_PAGE_SIZE,
            total: 0,
            has_next: false,
            mode: PaginationMode::Paged,
        }
    }
}

impl GridPagination {
    pub fn new(page: usize, page_size: usize, total: u64) -> Self {
        let mut result = Self {
            page: page.max(1),
            page_size: page_size.max(1),
            total,
            has_next: false,
            mode: PaginationMode::Paged,
        };
        result.normalize();
        result
    }

    /// Same as [`Self::new`] but in infinite-scroll mode.
    pub fn infinite(page_size: usize, total: u64) -> Self {
        let mut result = Self::new(1, page_size, total);
        result.mode = PaginationMode::Infinite;
        result
    }

    /// Number of pages, never zero (an empty grid still has "page 1 of 1").
    pub fn total_pages(&self) -> usize {
        let page_size = self.page_size.max(1) as u64;
        // Integer arithmetic avoids the lossy u64 -> f64 conversion that
        // silently rounds for data sets above 2^53 rows.
        self.total
            .checked_add(page_size - 1)
            .map(|n| n / page_size)
            .map(|pages| usize::try_from(pages).unwrap_or(usize::MAX))
            .unwrap_or(usize::MAX)
            .max(1)
    }

    pub fn has_previous(&self) -> bool {
        self.page > 1
    }

    /// One-based index of the first rendered row (0 when there is no data).
    pub fn from_index(&self) -> usize {
        if self.total == 0 {
            0
        } else if self.mode == PaginationMode::Infinite {
            1
        } else {
            self.page
                .saturating_sub(1)
                .saturating_mul(self.page_size)
                .saturating_add(1)
        }
    }

    /// One-based index of the last rendered row (0 when there is no data).
    pub fn to_index(&self) -> usize {
        let total = usize::try_from(self.total).unwrap_or(usize::MAX);
        self.page.saturating_mul(self.page_size).min(total)
    }

    /// Number of rows rendered for the current page/mode.
    pub fn visible_len(&self) -> usize {
        match self.mode {
            PaginationMode::Paged => self
                .to_index()
                .saturating_sub(self.from_index().saturating_sub(1)),
            PaginationMode::Infinite => self.to_index(),
        }
    }

    /// Clamp an arbitrary (possibly out of range, possibly zero) page number
    /// into the valid range without mutating anything.
    pub fn clamp_page(&self, page: usize) -> usize {
        page.max(1).min(self.total_pages())
    }

    /// Re-establish every invariant after a field was mutated directly.
    pub fn normalize(&mut self) {
        self.page_size = self.page_size.max(1);
        self.page = self.clamp_page(self.page);
        self.has_next = (self.page as u64).saturating_mul(self.page_size as u64) < self.total;
    }

    /// Move to `next_page`, clamped into the valid range.
    pub fn set_page(&mut self, next_page: usize) {
        self.page = self.clamp_page(next_page);
        self.normalize();
    }

    /// Like [`Self::set_page`] but returns the page actually selected, so
    /// callers can report the *effective* page to a server instead of the
    /// out-of-range one a user clicked.
    pub fn go_to_page(&mut self, next_page: usize) -> usize {
        self.set_page(next_page);
        self.page
    }

    /// Advance one page when possible. Returns the effective page.
    pub fn next_page(&mut self) -> usize {
        self.go_to_page(self.page.saturating_add(1))
    }

    /// Go back one page when possible. Returns the effective page.
    pub fn previous_page(&mut self) -> usize {
        self.go_to_page(self.page.saturating_sub(1))
    }

    /// Change the page size and reset to the first page.
    pub fn set_page_size(&mut self, size: usize) {
        self.page_size = size.max(1);
        self.set_page(1);
    }

    /// Update the row count, keeping the current page in range.
    pub fn set_total(&mut self, total: u64) {
        self.total = total;
        self.normalize();
    }

    pub fn set_mode(&mut self, mode: PaginationMode) {
        self.mode = mode;
        // Infinite scrolling always renders from the first page downwards.
        if mode == PaginationMode::Infinite {
            self.normalize();
        }
    }

    /// Zero-based `[start, end)` slice bounds for client-side paging over
    /// `len` locally available rows.
    pub fn slice_bounds(&self, len: usize) -> (usize, usize) {
        match self.mode {
            PaginationMode::Paged => {
                let start = self
                    .page
                    .saturating_sub(1)
                    .saturating_mul(self.page_size)
                    .min(len);
                let end = start.saturating_add(self.page_size).min(len);
                (start, end)
            }
            PaginationMode::Infinite => {
                (0, self.page.max(1).saturating_mul(self.page_size).min(len))
            }
        }
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

    #[test]
    fn pagination_normalizes_invalid_inputs_without_overflow() {
        let mut p = GridPagination::new(0, 0, u64::MAX);
        assert_eq!(p.page, 1);
        assert_eq!(p.page_size, 1);
        p.set_page(usize::MAX);
        assert_eq!(p.page, usize::MAX);
        assert!(!p.has_next);
    }

    #[test]
    fn navigation_is_clamped_and_reports_the_effective_page() {
        let mut p = GridPagination::new(1, 10, 25);
        assert_eq!(p.previous_page(), 1);
        assert_eq!(p.next_page(), 2);
        assert_eq!(p.go_to_page(999), 3);
        assert_eq!(p.next_page(), 3);
        assert_eq!(p.clamp_page(0), 1);
    }

    #[test]
    fn empty_data_sets_still_have_one_page() {
        let p = GridPagination::new(1, 20, 0);
        assert_eq!(p.total_pages(), 1);
        assert_eq!(p.from_index(), 0);
        assert_eq!(p.to_index(), 0);
        assert!(!p.has_next);
    }

    #[test]
    fn shrinking_the_total_pulls_the_page_back_into_range() {
        let mut p = GridPagination::new(5, 10, 100);
        p.set_total(12);
        assert_eq!(p.page, 2);
        assert!(!p.has_next);
    }

    #[test]
    fn slice_bounds_are_safe_for_every_page() {
        let p = GridPagination::new(3, 10, 12);
        // page is clamped to 2 -> rows 10..12
        assert_eq!(p.slice_bounds(12), (10, 12));

        let mut p = GridPagination::new(1, 10, 100);
        p.page = 9; // out of range for a locally available slice of 12 rows
        assert_eq!(p.slice_bounds(12), (12, 12));

        p.set_mode(PaginationMode::Infinite);
        p.set_page(2);
        assert_eq!(p.slice_bounds(55), (0, 20));
    }

    #[test]
    fn page_size_change_resets_to_first_page() {
        let mut p = GridPagination::new(3, 10, 100);
        p.set_page_size(50);
        assert_eq!((p.page, p.page_size, p.total_pages()), (1, 50, 2));
    }
}
