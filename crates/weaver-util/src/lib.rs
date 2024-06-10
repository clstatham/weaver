pub mod lock;

pub mod prelude {
    pub use crate::lock::*;
    pub use anyhow::{anyhow, bail, ensure, Error, Result};
    pub use downcast_rs::{impl_downcast, Downcast};
}
