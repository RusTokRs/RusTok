/// Product-wide maximum number of SKUs accepted by the owner and complete projections.
pub const MAX_PRODUCT_VARIANTS: usize = 256;

pub mod product;
pub mod variant;
pub mod variant_axis;

pub use product::*;
pub use variant::*;
pub use variant_axis::*;
