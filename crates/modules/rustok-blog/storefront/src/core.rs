pub mod comments_pagination;
pub mod discussion;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub mod presentation;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub mod reading_time;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub mod routing;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub mod search;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub mod sharing;

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use presentation::*;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use reading_time::*;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use routing::*;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use search::*;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use sharing::*;

#[cfg(all(test, any(feature = "ssr", not(feature = "comment-island"))))]
mod tests;
