#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    _padding: u32,
}

impl Color {
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        _padding: 0,
    };

    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        _padding: 0,
    };

    pub const RED: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        _padding: 0,
    };

    pub const GREEN: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        _padding: 0,
    };

    pub const BLUE: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        _padding: 0,
    };

    pub const YELLOW: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        _padding: 0,
    };

    pub const CYAN: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        _padding: 0,
    };

    pub const MAGENTA: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        _padding: 0,
    };

    /// Creates a new [Color] from the given RGB values.
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self {
            r,
            g,
            b,
            _padding: 0,
        }
    }

    /// Creates a new [Color] from the given hex value.
    #[inline]
    pub fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 24) & 0xff) as f32 / 255.0,
            g: ((hex >> 16) & 0xff) as f32 / 255.0,
            b: ((hex >> 8) & 0xff) as f32 / 255.0,
            _padding: 0,
        }
    }

    /// Returns the RGB values of the [Color] as a tuple.
    #[inline]
    pub fn rgb(&self) -> (f32, f32, f32) {
        (self.r, self.g, self.b)
    }

    /// Returns the RGB values of the [Color] as a tuple of integers.
    #[inline]
    pub fn rgb_int(&self) -> (u8, u8, u8) {
        (
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
        )
    }

    /// Returns the RGB values of the [Color] as a RGBA hex value with the alpha value set to 0xff.
    #[inline]
    pub fn hex(&self) -> u32 {
        ((self.r.clamp(0.0, 1.0) * 255.0) as u32) << 24
            | ((self.g.clamp(0.0, 1.0) * 255.0) as u32) << 16
            | ((self.b.clamp(0.0, 1.0) * 255.0) as u32) << 8
            | 0xff
    }

    /// Returns the [Color] as a [glam::Vec3].
    #[inline]
    pub fn vec3(&self) -> glam::Vec3 {
        glam::Vec3::new(self.r, self.g, self.b)
    }

    /// Returns the [Color] as a [glam::Vec4] with the alpha value set to 1.0.
    #[inline]
    pub fn vec4(&self) -> glam::Vec4 {
        glam::Vec4::new(self.r, self.g, self.b, 1.0)
    }

    #[inline]
    pub fn gamma_corrected(&self, gamma: f32) -> Self {
        Self::new(self.r.powf(gamma), self.g.powf(gamma), self.b.powf(gamma))
    }

    #[inline]
    pub fn clamp(&self, min: f32, max: f32) -> Self {
        Self::new(
            self.r.clamp(min, max),
            self.g.clamp(min, max),
            self.b.clamp(min, max),
        )
    }
}

impl std::ops::Add for Color {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.r + rhs.r, self.g + rhs.g, self.b + rhs.b)
    }
}

impl std::ops::AddAssign for Color {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = Self::new(self.r + rhs.r, self.g + rhs.g, self.b + rhs.b);
    }
}

impl std::ops::Add<f32> for Color {
    type Output = Self;

    #[inline]
    fn add(self, rhs: f32) -> Self {
        Self::new(self.r + rhs, self.g + rhs, self.b + rhs)
    }
}

impl std::ops::AddAssign<f32> for Color {
    #[inline]
    fn add_assign(&mut self, rhs: f32) {
        *self = Self::new(self.r + rhs, self.g + rhs, self.b + rhs);
    }
}

impl std::ops::Sub for Color {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.r - rhs.r, self.g - rhs.g, self.b - rhs.b)
    }
}

impl std::ops::SubAssign for Color {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self::new(self.r - rhs.r, self.g - rhs.g, self.b - rhs.b);
    }
}

impl std::ops::Sub<f32> for Color {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: f32) -> Self {
        Self::new(self.r - rhs, self.g - rhs, self.b - rhs)
    }
}

impl std::ops::SubAssign<f32> for Color {
    #[inline]
    fn sub_assign(&mut self, rhs: f32) {
        *self = Self::new(self.r - rhs, self.g - rhs, self.b - rhs);
    }
}

impl std::ops::Mul for Color {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self::new(self.r * rhs.r, self.g * rhs.g, self.b * rhs.b)
    }
}

impl std::ops::MulAssign for Color {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = Self::new(self.r * rhs.r, self.g * rhs.g, self.b * rhs.b);
    }
}

impl std::ops::Mul<f32> for Color {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.r * rhs, self.g * rhs, self.b * rhs)
    }
}

impl std::ops::MulAssign<f32> for Color {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        *self = Self::new(self.r * rhs, self.g * rhs, self.b * rhs);
    }
}

impl std::ops::Div for Color {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self {
        Self::new(self.r / rhs.r, self.g / rhs.g, self.b / rhs.b)
    }
}

impl std::ops::DivAssign for Color {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        *self = Self::new(self.r / rhs.r, self.g / rhs.g, self.b / rhs.b);
    }
}

impl std::ops::Div<f32> for Color {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self {
        Self::new(self.r / rhs, self.g / rhs, self.b / rhs)
    }
}

impl std::ops::DivAssign<f32> for Color {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        *self = Self::new(self.r / rhs, self.g / rhs, self.b / rhs);
    }
}
