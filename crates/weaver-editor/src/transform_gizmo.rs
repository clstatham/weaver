use weaver::prelude::*;

#[derive(Debug, Clone, Copy, Reflect, Resource)]
pub struct TransformGizmo {
    #[reflect(ignore)]
    pub focus: Option<Entity>,
    pub size: f32,
    pub axis_size: f32,
    pub handle_size: f32,
    pub middle_color: Color,
    pub x_color: Color,
    pub y_color: Color,
    pub z_color: Color,
    pub extra_scaling: f32,
    pub desired_pixel_size: f32,
}

impl TransformGizmo {
    pub fn draw(&self, gizmos: &Gizmos, focus_transform: &Transform) {
        let focus_translation = focus_transform.translation;

        let handle_size = self.extra_scaling * self.handle_size;
        let axis_size = self.extra_scaling * self.axis_size;
        let size = self.extra_scaling * self.size;

        let x_axis = Vec3A::X;
        let y_axis = Vec3A::Y;
        let z_axis = Vec3A::Z;

        let x_start = focus_translation + x_axis * handle_size;
        let y_start = focus_translation + y_axis * handle_size;
        let z_start = focus_translation + z_axis * handle_size;

        let x_end = focus_translation + x_axis * size;
        let y_end = focus_translation + y_axis * size;
        let z_end = focus_translation + z_axis * size;

        let x_middle = (x_start + x_end) / 2.0 - x_axis * handle_size / 2.0;
        let y_middle = (y_start + y_end) / 2.0 - y_axis * handle_size / 2.0;
        let z_middle = (z_start + z_end) / 2.0 - z_axis * handle_size / 2.0;

        let x_scale = Vec3A::new(size, axis_size, axis_size);
        let y_scale = Vec3A::new(axis_size, size, axis_size);
        let z_scale = Vec3A::new(axis_size, axis_size, size);

        gizmos.solid_cube_no_depth(
            Transform {
                translation: focus_translation,
                rotation: Quat::IDENTITY,
                scale: Vec3A::splat(handle_size),
            },
            self.middle_color,
        );

        // draw a thin cuboid for the x axis
        gizmos.solid_cube_no_depth(
            Transform {
                translation: x_middle,
                rotation: Quat::IDENTITY,
                scale: x_scale,
            },
            self.x_color,
        );
        // draw the x axis handle
        gizmos.solid_cube_no_depth(
            Transform {
                translation: x_end,
                rotation: Quat::IDENTITY,
                scale: Vec3A::splat(handle_size),
            },
            self.x_color,
        );

        // draw a thin cuboid for the y axis
        gizmos.solid_cube_no_depth(
            Transform {
                translation: y_middle,
                rotation: Quat::IDENTITY,
                scale: y_scale,
            },
            self.y_color,
        );
        // draw the y axis handle
        gizmos.solid_cube_no_depth(
            Transform {
                translation: y_end,
                rotation: Quat::IDENTITY,
                scale: Vec3A::splat(handle_size),
            },
            self.y_color,
        );

        // draw a thin cuboid for the z axis
        gizmos.solid_cube_no_depth(
            Transform {
                translation: z_middle,
                rotation: Quat::IDENTITY,
                scale: z_scale,
            },
            self.z_color,
        );
        // draw the z axis handle
        gizmos.solid_cube_no_depth(
            Transform {
                translation: z_end,
                rotation: Quat::IDENTITY,
                scale: Vec3A::splat(handle_size),
            },
            self.z_color,
        );
    }
}

pub fn draw_transform_gizmo(
    gizmos: ResMut<Gizmos>,
    transform_gizmo: Res<TransformGizmo>,
    transforms: Query<&Transform>,
) -> Result<()> {
    if let Some(focus_transform) = transform_gizmo
        .focus
        .and_then(|entity| transforms.get(entity))
    {
        transform_gizmo.draw(&gizmos, &focus_transform);
    }

    Ok(())
}
