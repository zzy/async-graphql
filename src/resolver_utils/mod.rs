//! Utilities for implementing
//! [`OutputValueType::resolve`](trait.OutputValueType.html#tymethod.resolve).

mod container;
mod list;
mod scalar;

pub use container::*;
pub use list::*;
pub use scalar::*;
