use std::{path::PathBuf, sync::Arc};

use parking_lot::RwLock;
use petgraph::stable_graph::NodeIndex;

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
    registry.get_static::<PathBuf>();
    registry.get_static::<NodeIndex>();

    glam::Vec3::register_vtable(registry);
    glam::Vec2::register_vtable(registry);
    glam::Vec4::register_vtable(registry);
    glam::Mat4::register_vtable(registry);
    glam::Quat::register_vtable(registry);
    PathBuf::register_vtable(registry);
    NodeIndex::register_vtable(registry);
}

impl Component for () {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "()"
    }
}
impl Component for bool {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "bool"
    }
}
impl Component for u8 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "u8"
    }
}
impl Component for u16 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "u16"
    }
}
impl Component for u32 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "u32"
    }
}
impl Component for u64 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "u64"
    }
}
impl Component for u128 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "u128"
    }
}
impl Component for usize {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "usize"
    }
}
impl Component for i8 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "i8"
    }
}
impl Component for i16 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "i16"
    }
}
impl Component for i32 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "i32"
    }
}
impl Component for i64 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "i64"
    }
}
impl Component for i128 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "i128"
    }
}
impl Component for isize {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "isize"
    }
}
impl Component for f32 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "f32"
    }
}
impl Component for f64 {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "f64"
    }
}
impl Component for String {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "String"
    }
}

impl Component for PathBuf {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "PathBuf"
    }
}

impl<T: Component> Component for RwLock<T> {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "RwLock"
    }
}

impl<T: Component> Component for Arc<T> {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "Arc"
    }
}

impl Component for NodeIndex {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "NodeIndex"
    }
}

impl<T: Component> Component for Vec<T> {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "Vec"
    }

    fn fields(&self, _registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        vec![]
    }

    fn register_vtable(registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.vtable_registered(id) {
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

        registry.register_vtable(id, methods);
    }
}

impl<T: Component> Component for Option<T> {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "Option"
    }

    fn fields(&self, registry: &Arc<Registry>) -> Vec<Data>
    where
        Self: Sized,
    {
        match self {
            Some(t) => t.fields(registry),
            None => vec![],
        }
    }

    fn set_field_by_name(&mut self, field_name: &str, value: Data) -> anyhow::Result<()> {
        match self {
            Some(t) => t.set_field_by_name(field_name, value),
            None => Ok(()),
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

    fn set_field_by_name(&mut self, field_name: &str, value: Data) -> anyhow::Result<()> {
        match field_name {
            "x" => {
                let x = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.x = *x;
                Ok(())
            }
            "y" => {
                let y = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.y = *y;
                Ok(())
            }
            "z" => {
                let z = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.z = *z;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Invalid field name")),
        }
    }

    fn register_vtable(registry: &Arc<Registry>) {
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
        registry.register_vtable(id, methods);
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

    fn set_field_by_name(&mut self, field_name: &str, value: Data) -> anyhow::Result<()> {
        match field_name {
            "x" => {
                let x = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.x = *x;
                Ok(())
            }
            "y" => {
                let y = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.y = *y;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Invalid field name")),
        }
    }

    fn register_vtable(registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.vtable_registered(id) {
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
        registry.register_vtable(id, methods);
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

    fn set_field_by_name(&mut self, field_name: &str, value: Data) -> anyhow::Result<()> {
        match field_name {
            "x" => {
                let x = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.x = *x;
                Ok(())
            }
            "y" => {
                let y = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.y = *y;
                Ok(())
            }
            "z" => {
                let z = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.z = *z;
                Ok(())
            }
            "w" => {
                let w = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.w = *w;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Invalid field name")),
        }
    }

    fn register_vtable(registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.vtable_registered(id) {
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
        registry.register_vtable(id, methods);
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

    fn set_field_by_name(&mut self, field_name: &str, value: Data) -> anyhow::Result<()> {
        match field_name {
            "x" => {
                let x = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.x = *x;
                Ok(())
            }
            "y" => {
                let y = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.y = *y;
                Ok(())
            }
            "z" => {
                let z = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.z = *z;
                Ok(())
            }
            "w" => {
                let w = value
                    .get_as::<f32>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.w = *w;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Invalid field name")),
        }
    }

    fn register_vtable(registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.vtable_registered(id) {
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

        registry.register_vtable(id, methods);
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

    fn set_field_by_name(&mut self, field_name: &str, value: Data) -> anyhow::Result<()> {
        match field_name {
            "x_axis" => {
                let x_axis = value
                    .get_as::<glam::Vec4>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.x_axis = *x_axis;
                Ok(())
            }
            "y_axis" => {
                let y_axis = value
                    .get_as::<glam::Vec4>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.y_axis = *y_axis;
                Ok(())
            }
            "z_axis" => {
                let z_axis = value
                    .get_as::<glam::Vec4>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.z_axis = *z_axis;
                Ok(())
            }
            "w_axis" => {
                let w_axis = value
                    .get_as::<glam::Vec4>()
                    .ok_or(anyhow::anyhow!("Invalid field type"))?;
                self.w_axis = *w_axis;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Invalid field name")),
        }
    }

    fn register_vtable(registry: &Arc<Registry>) {
        let id = registry.get_static::<Self>();
        if registry.vtable_registered(id) {
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

        registry.register_vtable(id, methods);
    }
}

impl<
        N: Send + Sync + 'static,
        E: Send + Sync + 'static,
        Ty: Send + Sync + 'static,
        Ix: Send + Sync + 'static,
    > Component for petgraph::Graph<N, E, Ty, Ix>
{
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "Graph"
    }
}

impl<
        N: Send + Sync + 'static,
        E: Send + Sync + 'static,
        Ty: Send + Sync + 'static,
        Ix: Send + Sync + 'static,
    > Component for petgraph::stable_graph::StableGraph<N, E, Ty, Ix>
{
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        "StableGraph"
    }
}
