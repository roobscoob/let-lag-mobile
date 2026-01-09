use crate::shape::compiled::shader::{TileBounds, argument::COORD_SCALE};

#[derive(Debug, Clone, Copy)]
pub struct Tile {
    pub zoom: u8,
    pub tile_x: u32,
    pub tile_y: u32,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Tile {
    pub const WORLD: Tile = Tile {
        zoom: 0,
        tile_x: 0,
        tile_y: 0,
        x0: 0.0,
        y0: 0.0,
        x1: 1.0,
        y1: 1.0,
    };

    pub fn children(&self) -> [Tile; 4] {
        let new_tile_x = self.tile_x * 2;
        let new_tile_y = self.tile_y * 2;
        let mx = (self.x0 + self.x1) / 2.0;
        let my = (self.y0 + self.y1) / 2.0;

        [
            // Bottom-left
            Tile {
                zoom: self.zoom + 1,
                tile_x: new_tile_x,
                tile_y: new_tile_y + 1,
                x0: self.x0,
                y0: my,
                x1: mx,
                y1: self.y1,
            },
            // Bottom-right
            Tile {
                zoom: self.zoom + 1,
                tile_x: new_tile_x + 1,
                tile_y: new_tile_y + 1,
                x0: mx,
                y0: my,
                x1: self.x1,
                y1: self.y1,
            },
            // Top-right
            Tile {
                zoom: self.zoom + 1,
                tile_x: new_tile_x + 1,
                tile_y: new_tile_y,
                x0: mx,
                y0: self.y0,
                x1: self.x1,
                y1: my,
            },
            // Top-left
            Tile {
                zoom: self.zoom + 1,
                tile_x: new_tile_x,
                tile_y: new_tile_y,
                x0: self.x0,
                y0: self.y0,
                x1: mx,
                y1: my,
            },
        ]
    }

    pub fn into_bounds(&self) -> TileBounds {
        use std::f64::consts::PI;

        // Convert normalized x coordinates to longitude (linear mapping)
        let min_lon_deg = (self.x0 * 360.0 - 180.0) as f32;
        let max_lon_deg = (self.x1 * 360.0 - 180.0) as f32;

        // Convert normalized y coordinates to latitude using inverse Mercator projection
        // In Web Mercator: y=0 is north (max latitude), y=1 is south (min latitude)
        let max_lat_deg = ((PI - 2.0 * PI * self.y0).sinh().atan() * 180.0 / PI) as f32;
        let min_lat_deg = ((PI - 2.0 * PI * self.y1).sinh().atan() * 180.0 / PI) as f32;

        TileBounds {
            min_lat_deg: (COORD_SCALE as f64 * min_lat_deg as f64) as i32,
            min_lon_deg: (COORD_SCALE as f64 * min_lon_deg as f64) as i32,
            lat_span_deg: (COORD_SCALE as f64 * (max_lat_deg - min_lat_deg) as f64) as i32,
            lon_span_deg: (COORD_SCALE as f64 * (max_lon_deg - min_lon_deg) as f64) as i32,
        }
    }
}
