//! Internal implementation details.

pub(crate) mod circular;
pub(crate) mod dispose_bag;

pub use circular::CircularPanic;
pub(crate) use circular::with_circular_catch;
pub(crate) use dispose_bag::{DisposeBag, BoxFutureUnit};