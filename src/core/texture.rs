use super::color::Color;

#[derive(Clone)]
pub struct Texture {
    pub width: usize,
    pub height: usize,
    pub data: Vec<Color>,
}

impl Texture {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![Color::BLACK; width * height],
        }
    }

    pub fn from_data(width: usize, height: usize, data: Vec<Color>) -> Self {
        Self {
            width,
            height,
            data,
        }
    }

    pub fn from_data_r8g8b8(width: usize, height: usize, data: &[u8]) -> Self {
        let mut texture = Self::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let index = (y * width + x) * 3;
                let r = data[index] as f32 / 255.0;
                let g = data[index + 1] as f32 / 255.0;
                let b = data[index + 2] as f32 / 255.0;
                texture.set(x, y, Color::new(r, g, b));
            }
        }

        texture
    }

    pub fn get(&self, x: usize, y: usize) -> Option<Color> {
        let index = y * self.width + x;
        self.data.get(index).copied()
    }

    pub fn set(&mut self, x: usize, y: usize, color: Color) {
        let index = y * self.width + x;
        self.data[index] = color;
    }

    pub fn get_uv(&self, u: f32, v: f32) -> Option<Color> {
        let x = (u * self.width as f32) as usize;
        let y = (v * self.height as f32) as usize;
        self.get(x, y)
    }
}
