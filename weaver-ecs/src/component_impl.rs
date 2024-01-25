use std::sync::Arc;

use crate::{
    component::{Data, MethodWrapper},
    prelude::*,
    registry::Registry,
};

impl<T: Component> Component for Vec<T> {
    fn fields(&self, _registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        vec![]
    }

    fn register_methods(&self, registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.methods_registered(id) {
            return;
        }
        let registry_clone = registry.clone();
        let methods = vec![MethodWrapper::from_method(
            "len",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<Vec<T>>();
                let result = this.len();
                Ok(Data::new(result, None, &registry_clone))
            },
        )];

        registry.register_methods(id, methods);
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

    fn register_methods(&self, registry: &Arc<Registry>) {
        let mut methods = vec![];
        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec3>();
                let result = this.length();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length_squared",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec3>();
                let result = this.length_squared();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec3>();
                let result = this.normalize();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize_or_zero",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec3>();
                let result = this.normalize_or_zero();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let id = registry.get_static::<Self>();
        registry.register_methods(id, methods);
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

    fn register_methods(&self, registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.methods_registered(id) {
            return;
        }
        let mut methods = vec![];
        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec2>();
                let result = this.length();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length_squared",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec2>();
                let result = this.length_squared();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec2>();
                let result = this.normalize();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize_or_zero",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec2>();
                let result = this.normalize_or_zero();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let id = registry.get_static::<Self>();
        registry.register_methods(id, methods);
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

    fn register_methods(&self, registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.methods_registered(id) {
            return;
        }
        let mut methods = vec![];
        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec4>();
                let result = this.length();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length_squared",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec4>();
                let result = this.length_squared();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec4>();
                let result = this.normalize();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize_or_zero",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec4>();
                let result = this.normalize_or_zero();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let id = registry.get_static::<Self>();
        registry.register_methods(id, methods);
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

    fn register_methods(&self, registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.methods_registered(id) {
            return;
        }
        let mut methods = vec![];
        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Quat>();
                let result = this.length();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length_squared",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Quat>();
                let result = this.length_squared();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Quat>();
                let result = this.normalize();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "from_axis_angle",
            2,
            move |data: &[&Data]| {
                let axis = &data[0];
                let axis = axis.get_as::<glam::Vec3>();
                let angle = &data[1];
                let angle = angle.get_as::<f32>();
                let data = glam::Quat::from_axis_angle(*axis, *angle);
                Ok(Data::new(data, None, &registry_clone))
            },
        ));

        registry.register_methods(id, methods);
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

    fn register_methods(&self, registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.methods_registered(id) {
            return;
        }
        let mut methods = vec![];
        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "determinant",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Mat4>();
                let data = this.determinant();
                Ok(Data::new(data, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "inverse",
            1,
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Mat4>();
                let data = this.inverse();
                Ok(Data::new(data, None, &registry_clone))
            },
        ));

        registry.register_methods(id, methods);
    }
}
