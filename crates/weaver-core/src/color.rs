use encase::ShaderType;
use weaver_ecs::prelude::Reflect;

#[derive(Debug, Clone, Copy, PartialEq, ShaderType, Reflect)]
#[repr(C)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    pub const YELLOW: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    pub const CYAN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub const MAGENTA: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_hex(hex: u32) -> Self {
        let r = ((hex >> 24) & 0xff) as f32 / 255.0;
        let g = ((hex >> 16) & 0xff) as f32 / 255.0;
        let b = ((hex >> 8) & 0xff) as f32 / 255.0;
        let a = (hex & 0xff) as f32 / 255.0;

        Self { r, g, b, a }
    }

    pub fn to_hex(&self) -> u32 {
        ((self.r * 255.0) as u32) << 24
            | ((self.g * 255.0) as u32) << 16
            | ((self.b * 255.0) as u32) << 8
            | (self.a * 255.0) as u32
    }

    pub const fn to_rgba(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub const fn to_rgb(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }

    pub fn to_rgba_u8(&self) -> [u8; 4] {
        [
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        ]
    }

    pub fn to_rgb_u8(&self) -> [u8; 3] {
        [
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
        ]
    }

    pub fn from_rgba_u8(rgba: [u8; 4]) -> Self {
        Self {
            r: rgba[0] as f32 / 255.0,
            g: rgba[1] as f32 / 255.0,
            b: rgba[2] as f32 / 255.0,
            a: rgba[3] as f32 / 255.0,
        }
    }

    pub fn from_rgb_u8(rgb: [u8; 3]) -> Self {
        Self {
            r: rgb[0] as f32 / 255.0,
            g: rgb[1] as f32 / 255.0,
            b: rgb[2] as f32 / 255.0,
            a: 1.0,
        }
    }

    pub const fn from_rgba(rgba: [f32; 4]) -> Self {
        Self {
            r: rgba[0],
            g: rgba[1],
            b: rgba[2],
            a: rgba[3],
        }
    }

    pub const fn from_rgb(rgb: [f32; 3]) -> Self {
        Self {
            r: rgb[0],
            g: rgb[1],
            b: rgb[2],
            a: 1.0,
        }
    }

    pub fn lerp(a: Self, b: Self, t: f32) -> Self {
        Self {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
            a: a.a + (b.a - a.a) * t,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

impl From<[f32; 4]> for Color {
    fn from(rgba: [f32; 4]) -> Self {
        Self::from_rgba(rgba)
    }
}

impl From<[f32; 3]> for Color {
    fn from(rgb: [f32; 3]) -> Self {
        Self::from_rgb(rgb)
    }
}

impl From<[u8; 4]> for Color {
    fn from(rgba: [u8; 4]) -> Self {
        Self::from_rgba_u8(rgba)
    }
}

impl From<[u8; 3]> for Color {
    fn from(rgb: [u8; 3]) -> Self {
        Self::from_rgb_u8(rgb)
    }
}

impl From<u32> for Color {
    fn from(hex: u32) -> Self {
        Self::from_hex(hex)
    }
}

impl From<Color> for [f32; 4] {
    fn from(color: Color) -> [f32; 4] {
        color.to_rgba()
    }
}

impl From<Color> for [f32; 3] {
    fn from(color: Color) -> [f32; 3] {
        color.to_rgb()
    }
}

impl From<Color> for [u8; 4] {
    fn from(color: Color) -> [u8; 4] {
        color.to_rgba_u8()
    }
}

impl From<Color> for [u8; 3] {
    fn from(color: Color) -> [u8; 3] {
        color.to_rgb_u8()
    }
}

impl From<Color> for u32 {
    fn from(color: Color) -> u32 {
        color.to_hex()
    }
}

impl std::ops::Add<Self> for Color {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            r: self.r + rhs.r,
            g: self.g + rhs.g,
            b: self.b + rhs.b,
            a: self.a + rhs.a,
        }
    }
}

impl std::ops::Sub<Self> for Color {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            r: self.r - rhs.r,
            g: self.g - rhs.g,
            b: self.b - rhs.b,
            a: self.a - rhs.a,
        }
    }
}

impl std::ops::Mul<f32> for Color {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self {
            r: self.r * rhs,
            g: self.g * rhs,
            b: self.b * rhs,
            a: self.a * rhs,
        }
    }
}

impl std::ops::Div<f32> for Color {
    type Output = Self;

    fn div(self, rhs: f32) -> Self {
        Self {
            r: self.r / rhs,
            g: self.g / rhs,
            b: self.b / rhs,
            a: self.a / rhs,
        }
    }
}

impl std::ops::Mul<Self> for Color {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self {
            r: self.r * rhs.r,
            g: self.g * rhs.g,
            b: self.b * rhs.b,
            a: self.a * rhs.a,
        }
    }
}

impl std::ops::Div<Self> for Color {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        Self {
            r: self.r / rhs.r,
            g: self.g / rhs.g,
            b: self.b / rhs.b,
            a: self.a / rhs.a,
        }
    }
}

impl std::ops::AddAssign<Self> for Color {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::SubAssign<Self> for Color {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl std::ops::MulAssign<f32> for Color {
    fn mul_assign(&mut self, rhs: f32) {
        *self = *self * rhs;
    }
}

impl std::ops::DivAssign<f32> for Color {
    fn div_assign(&mut self, rhs: f32) {
        *self = *self / rhs;
    }
}

impl std::ops::MulAssign<Self> for Color {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl std::ops::DivAssign<Self> for Color {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:08x}", u32::from(*self))
    }
}

impl std::fmt::LowerHex for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08x}", u32::from(*self))
    }
}

impl std::fmt::UpperHex for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08X}", u32::from(*self))
    }
}
