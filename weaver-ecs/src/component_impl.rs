use std::sync::Arc;

use crate::{component::Data, id::Registry, prelude::*};

impl<T: Component> Component for Vec<T> {
    fn fields(&self, _registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        vec![]
    }
}

impl<T: Component> Component for Option<T> {
    fn fields(&self, registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        match self {
            Some(t) => t.fields(registry),
            None => vec![],
        }
    }
}

impl Component for glam::Vec3 {
    fn fields(&self, registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        vec![
            Data::new(self.x, Some("x"), registry),
            Data::new(self.y, Some("y"), registry),
            Data::new(self.z, Some("z"), registry),
        ]
    }
}

impl Component for glam::Vec2 {
    fn fields(&self, registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        vec![
            Data::new(self.x, Some("x"), registry),
            Data::new(self.y, Some("y"), registry),
        ]
    }
}

impl Component for glam::Vec4 {
    fn fields(&self, registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        vec![
            Data::new(self.x, Some("x"), registry),
            Data::new(self.y, Some("y"), registry),
            Data::new(self.z, Some("z"), registry),
            Data::new(self.w, Some("w"), registry),
        ]
    }
}

impl Component for glam::Quat {
    fn fields(&self, registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        vec![
            Data::new(self.x, Some("x"), registry),
            Data::new(self.y, Some("y"), registry),
            Data::new(self.z, Some("z"), registry),
            Data::new(self.w, Some("w"), registry),
        ]
    }
}

impl Component for glam::Mat4 {
    fn fields(&self, registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        vec![
            Data::new(self.x_axis, Some("x_axis"), registry),
            Data::new(self.y_axis, Some("y_axis"), registry),
            Data::new(self.z_axis, Some("z_axis"), registry),
            Data::new(self.w_axis, Some("w_axis"), registry),
        ]
    }
}
