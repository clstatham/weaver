use std::sync::Arc;

use crate::{
    component::{Data, MethodArgType, MethodWrapper},
    prelude::*,
    registry::Registry,
};

pub fn register_all(registry: &Arc<Registry>) {
    registry.get_static::<()>();
    registry.get_static::<bool>();
    registry.get_static::<u8>();
    registry.get_static::<u16>();
    registry.get_static::<u32>();
    registry.get_static::<u64>();
    registry.get_static::<u128>();
    registry.get_static::<usize>();
    registry.get_static::<i8>();
    registry.get_static::<i16>();
    registry.get_static::<i32>();
    registry.get_static::<i64>();
    registry.get_static::<i128>();
    registry.get_static::<isize>();
    registry.get_static::<f32>();
    registry.get_static::<f64>();
    registry.get_static::<String>();

    // register methods

    glam::Vec3::register_methods(registry);
    glam::Vec2::register_methods(registry);
    glam::Vec4::register_methods(registry);
    glam::Mat4::register_methods(registry);
    glam::Quat::register_methods(registry);
}

impl Component for () {}
impl Component for bool {}
impl Component for u8 {}
impl Component for u16 {}
impl Component for u32 {}
impl Component for u64 {}
impl Component for u128 {}
impl Component for usize {}
impl Component for i8 {}
impl Component for i16 {}
impl Component for i32 {}
impl Component for i64 {}
impl Component for i128 {}
impl Component for isize {}
impl Component for f32 {}
impl Component for f64 {}
impl Component for String {}

impl<T: Component> Component for Vec<T> {
    fn fields(&self, _registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        vec![]
    }

    fn register_methods(registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.methods_registered(id) {
            return;
        }
        let registry_clone = registry.clone();
        let methods = vec![MethodWrapper::from_method(
            "len",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<usize>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<Vec<T>>().unwrap();
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
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "Vec3"
    }

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

    fn register_methods(registry: &Arc<Registry>) {
        let mut methods = vec![];
        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<f32>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec3>().unwrap();
                let result = this.length();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length_squared",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<f32>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec3>().unwrap();
                let result = this.length_squared();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize",
            [MethodArgType::Owned(registry.get_static::<Self>())],
            Some(MethodArgType::Owned(registry.get_static::<Self>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec3>().unwrap();
                let result = this.normalize();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize_or_zero",
            [MethodArgType::Owned(registry.get_static::<Self>())],
            Some(MethodArgType::Owned(registry.get_static::<Self>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec3>().unwrap();
                let result = this.normalize_or_zero();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let id = registry.get_static::<Self>();
        registry.register_methods(id, methods);
    }
}

impl Component for glam::Vec2 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "Vec2"
    }

    fn fields(&self, registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        vec![
            Data::new(self.x, Some("x"), registry),
            Data::new(self.y, Some("y"), registry),
        ]
    }

    fn register_methods(registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.methods_registered(id) {
            return;
        }
        let mut methods = vec![];
        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<f32>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec2>().unwrap();
                let result = this.length();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length_squared",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<f32>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec2>().unwrap();
                let result = this.length_squared();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<Self>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec2>().unwrap();
                let result = this.normalize();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize_or_zero",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<Self>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec2>().unwrap();
                let result = this.normalize_or_zero();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let id = registry.get_static::<Self>();
        registry.register_methods(id, methods);
    }
}

impl Component for glam::Vec4 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "Vec4"
    }

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

    fn register_methods(registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.methods_registered(id) {
            return;
        }
        let mut methods = vec![];
        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<f32>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec4>().unwrap();
                let result = this.length();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length_squared",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<f32>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec4>().unwrap();
                let result = this.length_squared();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<Self>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec4>().unwrap();
                let result = this.normalize();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize_or_zero",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<Self>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Vec4>().unwrap();
                let result = this.normalize_or_zero();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let id = registry.get_static::<Self>();
        registry.register_methods(id, methods);
    }
}

impl Component for glam::Quat {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "Quat"
    }

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

    fn register_methods(registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.methods_registered(id) {
            return;
        }
        let mut methods = vec![];
        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<f32>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Quat>().unwrap();
                let result = this.length();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "length_squared",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<f32>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Quat>().unwrap();
                let result = this.length_squared();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "normalize",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<Self>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Quat>().unwrap();
                let result = this.normalize();
                Ok(Data::new(result, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "from_axis_angle",
            [
                MethodArgType::Ref(registry.get_static::<glam::Vec3>()),
                MethodArgType::Ref(registry.get_static::<f32>()),
            ],
            Some(MethodArgType::Ref(registry.get_static::<Self>())),
            move |data: &[&Data]| {
                let axis = &data[0];
                let axis = axis.get_as::<glam::Vec3>().unwrap();
                let angle = &data[1];
                let angle = angle.get_as::<f32>().unwrap();
                let data = glam::Quat::from_axis_angle(*axis, *angle);
                Ok(Data::new(data, None, &registry_clone))
            },
        ));

        registry.register_methods(id, methods);
    }
}

impl Component for glam::Mat4 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "Mat4"
    }

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

    fn register_methods(registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.methods_registered(id) {
            return;
        }
        let mut methods = vec![];
        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "determinant",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<f32>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Mat4>().unwrap();
                let data = this.determinant();
                Ok(Data::new(data, None, &registry_clone))
            },
        ));

        let registry_clone = registry.clone();
        methods.push(MethodWrapper::from_method(
            "inverse",
            [MethodArgType::Ref(registry.get_static::<Self>())],
            Some(MethodArgType::Ref(registry.get_static::<Self>())),
            move |data: &[&Data]| {
                let this = &data[0];
                let this = this.get_as::<glam::Mat4>().unwrap();
                let data = this.inverse();
                Ok(Data::new(data, None, &registry_clone))
            },
        ));

        registry.register_methods(id, methods);
    }
}
