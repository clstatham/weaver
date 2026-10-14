//! Render-world copies of entity transforms.

use encase::ShaderType;
use weaver_app::{App, plugin::Plugin};
use weaver_core::transform::Transform;
use weaver_ecs::query::{Changed, QueryableItem};
use weaver_util::prelude::*;

use crate::extract::{ExtractComponent, ExtractComponentPlugin};

impl ExtractComponent for Transform {
    type Data = &'static Transform;
    type Filter = Changed<&'static Transform>;
    type Out = GpuTransform;

    fn extract(transform: QueryableItem<'_, Self::Data>) -> Option<Self::Out> {
        Some(GpuTransform {
            matrix: transform.matrix(),
        })
    }
}

/// A mesh entity's model matrix in the render world.
#[derive(Copy, Clone, Debug, ShaderType)]
#[repr(C)]
pub struct GpuTransform {
    /// Local-to-world matrix.
    pub matrix: glam::Mat4,
}

impl GpuTransform {
    /// The identity transform.
    pub const IDENTITY: Self = Self {
        matrix: glam::Mat4::IDENTITY,
    };
}

/// Extracts [`Transform`] components as [`GpuTransform`]s whenever they change.
pub struct TransformPlugin;

impl Plugin for TransformPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(ExtractComponentPlugin::<Transform>::default())?;
        Ok(())
    }
}
