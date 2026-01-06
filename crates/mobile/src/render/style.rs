use jet_lag_core::shape::types::Centimeters;

pub enum Size {
    WorldSpace(Centimeters),
    ScreenSpace { pixels: f32 },
    Maximum(Box<Size>, Box<Size>),
}

pub enum Rotation {
    Fixed(f32),
    FieldOffset(f32),
}

pub enum Pattern {
    SolidColor(palette::Srgba<f32>),
    Stripes {
        stripes: Vec<(Size, palette::Srgba<f32>)>,
        rotation: Rotation,
    },
}

pub struct Style {
    border_color: palette::Srgba<f32>,
    border_width: Size,
    fill: Option<Pattern>,
}

impl Style {
    pub fn transparent() -> Self {
        Self {
            border_color: palette::Srgba::new(0.0, 0.0, 0.0, 0.0),
            border_width: Size::ScreenSpace { pixels: 0.0 },
            fill: None,
        }
    }

    pub fn solid_color(fill_color: palette::Srgba<f32>) -> Self {
        Self {
            border_color: palette::Srgba::new(0.0, 0.0, 0.0, 0.0),
            border_width: Size::ScreenSpace { pixels: 0.0 },
            fill: Some(Pattern::SolidColor(fill_color)),
        }
    }

    pub fn striped(rotation: Rotation, stripes: Vec<(Size, palette::Srgba<f32>)>) -> Self {
        Self {
            border_color: palette::Srgba::new(0.0, 0.0, 0.0, 0.0),
            border_width: Size::ScreenSpace { pixels: 0.0 },
            fill: Some(Pattern::Stripes { stripes, rotation }),
        }
    }

    pub fn with_border(mut self, border_width: Size, border_color: palette::Srgba<f32>) -> Self {
        self.border_color = border_color;
        self.border_width = border_width;
        self
    }
}
