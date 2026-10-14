//! RGBA colors.

use encase::ShaderType;

/// A color as four `f32` channels in the 0.0 to 1.0 range, red, green, blue, alpha.
///
/// The channels are plain numbers: nothing here converts between sRGB and linear. Laid out so it
/// can be passed to shaders directly.
#[derive(Debug, Clone, Copy, PartialEq, ShaderType)]
#[repr(C)]
pub struct Color {
    /// Red.
    pub r: f32,
    /// Green.
    pub g: f32,
    /// Blue.
    pub b: f32,
    /// Alpha (1.0 is opaque).
    pub a: f32,
}

impl Color {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Opaque black.
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Opaque white.
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    /// Opaque red.
    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Opaque green.
    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    /// Opaque blue.
    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    /// Opaque yellow.
    pub const YELLOW: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    /// Opaque cyan.
    pub const CYAN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    /// Opaque magenta.
    pub const MAGENTA: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    /// Creates a color from its four channels.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates a color from a packed `0xRRGGBBAA` value.
    pub fn from_hex(hex: u32) -> Self {
        let r = ((hex >> 24) & 0xff) as f32 / 255.0;
        let g = ((hex >> 16) & 0xff) as f32 / 255.0;
        let b = ((hex >> 8) & 0xff) as f32 / 255.0;
        let a = (hex & 0xff) as f32 / 255.0;

        Self { r, g, b, a }
    }

    /// Packs the color into `0xRRGGBBAA`.
    ///
    /// Channels are scaled to 0..=255 and truncated, not rounded.
    pub fn to_hex(&self) -> u32 {
        (((self.r * 255.0) as u32) << 24)
            | (((self.g * 255.0) as u32) << 16)
            | (((self.b * 255.0) as u32) << 8)
            | (self.a * 255.0) as u32
    }

    /// Returns the channels as `[r, g, b, a]`.
    pub const fn to_rgba(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Returns the channels as `[r, g, b]`, dropping alpha.
    pub const fn to_rgb(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }

    /// Returns `[r, g, b, a]` scaled to 0..=255 (truncated, and clamped by the cast).
    pub fn to_rgba_u8(&self) -> [u8; 4] {
        [
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        ]
    }

    /// Returns `[r, g, b]` scaled to 0..=255, dropping alpha.
    pub fn to_rgb_u8(&self) -> [u8; 3] {
        [
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
        ]
    }

    /// Creates a color from `[r, g, b, a]` bytes.
    pub fn from_rgba_u8(rgba: [u8; 4]) -> Self {
        Self {
            r: rgba[0] as f32 / 255.0,
            g: rgba[1] as f32 / 255.0,
            b: rgba[2] as f32 / 255.0,
            a: rgba[3] as f32 / 255.0,
        }
    }

    /// Creates an opaque color from `[r, g, b]` bytes.
    pub fn from_rgb_u8(rgb: [u8; 3]) -> Self {
        Self {
            r: rgb[0] as f32 / 255.0,
            g: rgb[1] as f32 / 255.0,
            b: rgb[2] as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Creates a color from `[r, g, b, a]`.
    pub const fn from_rgba(rgba: [f32; 4]) -> Self {
        Self {
            r: rgba[0],
            g: rgba[1],
            b: rgba[2],
            a: rgba[3],
        }
    }

    /// Creates an opaque color from `[r, g, b]`.
    pub const fn from_rgb(rgb: [f32; 3]) -> Self {
        Self {
            r: rgb[0],
            g: rgb[1],
            b: rgb[2],
            a: 1.0,
        }
    }

    /// Linearly interpolates every channel from `a` (at `t = 0`) to `b` (at `t = 1`).
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
