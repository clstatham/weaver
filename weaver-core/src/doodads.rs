use fabricate::{lock::SharedLock, prelude::Atom};

use crate::color::Color;

pub const MAX_DOODADS: usize = 1024;

#[derive(Default, Clone, Atom)]
pub struct Doodads {
    pub(crate) doodads: SharedLock<Vec<Doodad>>,
    pub locked: bool,
}

impl Doodads {
    pub fn new() -> Self {
        Self {
            doodads: SharedLock::new(Vec::new()),
            locked: false,
        }
    }

    pub fn lock(&mut self) {
        self.locked = true;
    }

    pub fn unlock(&mut self) {
        self.locked = false;
    }

    pub fn push(&mut self, doodad: Doodad) {
        let mut doodads = self.doodads.write();
        if !self.locked {
            doodads.push(doodad);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Doodad {
    Line(Line),
    Cube(Cube),
    WireCube(Cube),
    Cone(Cone),
}

#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub start: glam::Vec3,
    pub end: glam::Vec3,
    pub color: Color,
}

impl Line {
    pub const fn new(start: glam::Vec3, end: glam::Vec3, color: Color) -> Self {
        Self { start, end, color }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Cube {
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
    pub color: Color,
}

impl Cube {
    pub const fn new(
        position: glam::Vec3,
        rotation: glam::Quat,
        scale: glam::Vec3,
        color: Color,
    ) -> Self {
        Self {
            position,
            rotation,
            scale,
            color,
        }
    }
}

impl Default for Cube {
    fn default() -> Self {
        Self {
            position: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::ONE,
            color: Color::WHITE,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Cone {
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
    pub color: Color,
}

impl Cone {
    pub const fn new(
        position: glam::Vec3,
        rotation: glam::Quat,
        scale: glam::Vec3,
        color: Color,
    ) -> Self {
        Self {
            position,
            rotation,
            scale,
            color,
        }
    }
}

impl Default for Cone {
    fn default() -> Self {
        Self {
            position: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::ONE,
            color: Color::WHITE,
        }
    }
}
