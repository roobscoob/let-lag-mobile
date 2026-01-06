use glam::IVec2;

#[derive(Clone, Copy)]
pub struct Centimeters(pub i32);

impl Centimeters {
    pub fn as_millimeters(&self) -> i64 {
        self.0 as i64 * 10
    }

    pub fn from_millimeters(mm: i64) -> Self {
        Centimeters((mm / 10) as i32)
    }

    pub fn as_meters(&self) -> f32 {
        self.0 as f32 / 100.0
    }

    pub fn from_meters(m: f32) -> Self {
        Centimeters((m * 100.0) as i32)
    }
}
