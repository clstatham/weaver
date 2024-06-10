use weaver_util::prelude::{impl_downcast, Downcast};

pub trait Component: Downcast {}
impl_downcast!(Component);
