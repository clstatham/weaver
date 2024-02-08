use fabricate::prelude::Atom;

#[derive(Debug, Clone, Copy, PartialEq, Atom, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub const RED: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub const GREEN: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    pub const BLUE: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    pub const YELLOW: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    pub const CYAN: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub const MAGENTA: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    /// Creates a new [Color] from the given RGBA values.
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Returns the RGBA values of the [Color] as a tuple.
    #[inline]
    pub fn rgba(&self) -> (f32, f32, f32, f32) {
        (self.r, self.g, self.b, self.a)
    }

    /// Returns the RGBA values of the [Color] as a tuple of integers.
    #[inline]
    pub fn rgba_int(&self) -> (u8, u8, u8, u8) {
        (
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        )
    }

    pub fn from_rgba_int(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    /// Returns the RGBA values of the [Color] as a RGBA hex value.
    #[inline]
    pub fn hex(&self) -> u32 {
        ((self.r.clamp(0.0, 1.0) * 255.0) as u32) << 24
            | ((self.g.clamp(0.0, 1.0) * 255.0) as u32) << 16
            | ((self.b.clamp(0.0, 1.0) * 255.0) as u32) << 8
            | (self.a.clamp(0.0, 1.0) * 255.0) as u32
    }

    /// Returns the [Color] as a [glam::Vec3], discarding the alpha value.
    #[inline]
    pub fn vec3(&self) -> glam::Vec3 {
        glam::Vec3::new(self.r, self.g, self.b)
    }

    /// Returns the [Color] as a [glam::Vec4].
    #[inline]
    pub fn vec4(&self) -> glam::Vec4 {
        glam::Vec4::new(self.r, self.g, self.b, self.a)
    }

    #[inline]
    pub fn gamma_corrected(&self, gamma: f32) -> Self {
        Self::new(
            self.r.powf(gamma),
            self.g.powf(gamma),
            self.b.powf(gamma),
            self.a,
        )
    }

    #[inline]
    pub fn clamp(&self, min: f32, max: f32) -> Self {
        Self::new(
            self.r.clamp(min, max),
            self.g.clamp(min, max),
            self.b.clamp(min, max),
            self.a.clamp(min, max),
        )
    }
}

impl std::ops::Add for Color {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(
            self.r + rhs.r,
            self.g + rhs.g,
            self.b + rhs.b,
            self.a + rhs.a,
        )
    }
}

impl std::ops::AddAssign for Color {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = Self::new(
            self.r + rhs.r,
            self.g + rhs.g,
            self.b + rhs.b,
            self.a + rhs.a,
        );
    }
}

impl std::ops::Sub for Color {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(
            self.r - rhs.r,
            self.g - rhs.g,
            self.b - rhs.b,
            self.a - rhs.a,
        )
    }
}

impl std::ops::SubAssign for Color {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self::new(
            self.r - rhs.r,
            self.g - rhs.g,
            self.b - rhs.b,
            self.a - rhs.a,
        );
    }
}

impl std::ops::Mul for Color {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.r * rhs.r,
            self.g * rhs.g,
            self.b * rhs.b,
            self.a * rhs.a,
        )
    }
}

impl std::ops::MulAssign for Color {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = Self::new(
            self.r * rhs.r,
            self.g * rhs.g,
            self.b * rhs.b,
            self.a * rhs.a,
        );
    }
}

impl std::ops::Mul<f32> for Color {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.r * rhs, self.g * rhs, self.b * rhs, self.a * rhs)
    }
}

impl std::ops::MulAssign<f32> for Color {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        *self = Self::new(self.r * rhs, self.g * rhs, self.b * rhs, self.a * rhs);
    }
}

impl std::ops::Div for Color {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self {
        Self::new(
            self.r / rhs.r,
            self.g / rhs.g,
            self.b / rhs.b,
            self.a / rhs.a,
        )
    }
}

impl std::ops::DivAssign for Color {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        *self = Self::new(
            self.r / rhs.r,
            self.g / rhs.g,
            self.b / rhs.b,
            self.a / rhs.a,
        );
    }
}

impl std::ops::Div<f32> for Color {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self {
        Self::new(self.r / rhs, self.g / rhs, self.b / rhs, self.a / rhs)
    }
}

impl std::ops::DivAssign<f32> for Color {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        *self = Self::new(self.r / rhs, self.g / rhs, self.b / rhs, self.a / rhs);
    }
}

#[derive(Debug, Clone)]
pub struct ColorArray {
    pub colors: Vec<Color>,
}
