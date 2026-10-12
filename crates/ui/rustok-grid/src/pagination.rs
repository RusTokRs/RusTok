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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct GridPagination {
    pub page: usize,
    pub page_size: usize,
    pub total: u64,
    pub has_next: bool,
    pub mode: PaginationMode,
}

impl<'de> Deserialize<'de> for GridPagination {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct GridPaginationData {
            #[serde(default = "default_page")]
            page: usize,
            #[serde(default = "default_page_size")]
            page_size: usize,
            #[serde(default)]
            total: u64,
            #[serde(default)]
            has_next: bool,
            #[serde(default)]
            mode: PaginationMode,
        }

        let data = GridPaginationData::deserialize(deserializer)?;
        let mut pagination = Self {
            page: data.page,
            page_size: data.page_size,
            total: data.total,
            has_next: data.has_next,
            mode: data.mode,
        };
        pagination.normalize();
        Ok(pagination)
    }
}

/// Fallback page size used when a caller passes `0`.
pub const DEFAULT_PAGE_SIZE: usize = 20;

/// Page sizes a grid offers in its "rows per page" control when the owner does not choose its own.
///
/// The adapters used to each carry a copy of this list; one owner means the picker offers the same
/// choices on every surface, and [`GridPagination::normalize_page_size_options`] keeps an
/// out-of-list page size selectable instead of silently resetting a user's preference.
pub const DEFAULT_PAGE_SIZE_OPTIONS: [usize; 4] = [10, 20, 50, 100];

fn default_page() -> usize {
    1
}

fn default_page_size() -> usize {
    DEFAULT_PAGE_SIZE
}

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
        // Quotient + remainder computes ceil(total / page_size) without overflowing when
        // `total` is close to u64::MAX. A data set may exceed usize on 32-bit wasm targets, so
        // narrow only after computing the exact u64 page count.
        let whole_pages = self.total / page_size;
        let pages = if self.total.is_multiple_of(page_size) {
            whole_pages
        } else {
            whole_pages.saturating_add(1)
        };
        usize::try_from(pages).unwrap_or(usize::MAX).max(1)
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

    /// Standard formatted range description (e.g. "1–20 of 100", or "0 of 0").
    pub fn range_label(&self) -> String {
        if self.total == 0 {
            "0 of 0".to_string()
        } else {
            format!(
                "{}–{} of {}",
                self.from_index(),
                self.to_index(),
                self.total
            )
        }
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
        // A mode change is a mutation boundary: repair public fields and refresh `has_next`.
        self.normalize();
    }

    /// Zero-based index the current *fetch* window starts at, for a server that pages on request.
    ///
    /// Unlike [`Self::slice_bounds`] this is mode-independent: in infinite mode the window being
    /// appended is still page `N`, even though the grid renders pages `1..=N`.
    pub fn offset(&self) -> usize {
        self.page
            .saturating_sub(1)
            .saturating_mul(self.page_size.max(1))
    }

    /// The rows to offer in a "rows per page" picker: `choices` plus the current size if it is not
    /// already in the list, deduplicated and ascending. An empty choice list falls back to
    /// [`DEFAULT_PAGE_SIZE_OPTIONS`].
    pub fn normalize_page_size_options(choices: &[usize], current: usize) -> Vec<usize> {
        let source: &[usize] = if choices.is_empty() {
            DEFAULT_PAGE_SIZE_OPTIONS.as_slice()
        } else {
            choices
        };
        let mut options: Vec<usize> = source.iter().copied().filter(|size| *size > 0).collect();
        options.push(current.max(1));
        options.sort_unstable();
        options.dedup();
        options
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

    /// The rows this state renders, for a grid whose owner hands over the whole data set.
    ///
    /// Borrowing rather than cloning is deliberate: an adapter draws these rows and nothing else, so
    /// a page of an already-materialized list must not be copied per render.
    pub fn slice_items<'a, T>(&self, items: &'a [T]) -> &'a [T] {
        let (start, end) = self.slice_bounds(items.len());
        &items[start..end]
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
    fn deserialization_restores_pagination_invariants() {
        let pagination: GridPagination = serde_json::from_str(
            r#"{"page":99,"page_size":0,"total":21,"has_next":true,"mode":"Paged"}"#,
        )
        .expect("valid pagination JSON");
        assert_eq!(pagination.page, 21);
        assert_eq!(pagination.page_size, 1);
        assert!(!pagination.has_next);
    }

    #[test]
    fn page_count_ceil_does_not_saturate_for_large_totals() {
        let pagination = GridPagination::new(1, 20, u64::MAX);
        let whole_pages = u64::MAX / 20;
        let expected = if u64::MAX.is_multiple_of(20) {
            whole_pages
        } else {
            whole_pages + 1
        };
        assert_eq!(
            pagination.total_pages(),
            usize::try_from(expected).unwrap_or(usize::MAX)
        );
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

    #[test]
    fn range_label_formats_correctly() {
        let empty = GridPagination::new(1, 10, 0);
        assert_eq!(empty.range_label(), "0 of 0");

        let p1 = GridPagination::new(1, 20, 55);
        assert_eq!(p1.range_label(), "1–20 of 55");

        let p3 = GridPagination::new(3, 20, 55);
        assert_eq!(p3.range_label(), "41–55 of 55");
    }

    #[test]
    fn offset_reports_the_fetch_window_and_slice_items_the_render_window() {
        let paged = GridPagination::new(3, 10, 45);
        assert_eq!(paged.offset(), 20);
        let rows: Vec<u32> = (1..=45).collect();
        assert_eq!(paged.slice_items(&rows), &rows[20..30]);

        // Infinite mode renders everything up to and including the newest page, so the borrow is
        // the prefix while the offset still points at the page that was just fetched.
        let infinite = {
            let mut state = GridPagination::new(3, 10, 45);
            state.set_mode(PaginationMode::Infinite);
            state
        };
        assert_eq!(infinite.offset(), 20);
        assert_eq!(infinite.slice_items(&rows), &rows[0..30]);
    }

    #[test]
    fn page_size_options_keep_the_current_size_selectable() {
        assert_eq!(
            GridPagination::normalize_page_size_options(&[], 25),
            vec![10, 20, 25, 50, 100]
        );
        assert_eq!(
            GridPagination::normalize_page_size_options(&[25, 0, 5, 25], 5),
            vec![5, 25]
        );
    }

    #[test]
    fn changing_mode_restores_pagination_invariants() {
        let mut pagination = GridPagination::new(3, 10, 100);
        pagination.page = 0;
        pagination.page_size = 0;
        pagination.has_next = false;

        pagination.set_mode(PaginationMode::Paged);

        assert_eq!(pagination.page, 1);
        assert_eq!(pagination.page_size, 1);
        assert!(pagination.has_next);
    }
}
