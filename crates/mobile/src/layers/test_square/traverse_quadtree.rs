pub enum TileAction {
    Enter,  // Subdivide into 4 children
    Drop,   // Skip this tile
    Return, // Include in results, don't subdivide
}

#[derive(Debug, Clone, Copy)]
pub struct Tile {
    pub zoom: u8,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Tile {
    pub const WORLD: Tile = Tile {
        zoom: 0,
        x0: 0.0,
        y0: 0.0,
        x1: 1.0,
        y1: 1.0,
    };

    fn children(&self) -> [Tile; 4] {
        let mx = (self.x0 + self.x1) / 2.0;
        let my = (self.y0 + self.y1) / 2.0;

        [
            // Bottom-left
            Tile {
                zoom: self.zoom + 1,
                x0: self.x0,
                y0: my,
                x1: mx,
                y1: self.y1,
            },
            // Bottom-right
            Tile {
                zoom: self.zoom + 1,
                x0: mx,
                y0: my,
                x1: self.x1,
                y1: self.y1,
            },
            // Top-right
            Tile {
                zoom: self.zoom + 1,
                x0: mx,
                y0: self.y0,
                x1: self.x1,
                y1: my,
            },
            // Top-left
            Tile {
                zoom: self.zoom + 1,
                x0: self.x0,
                y0: self.y0,
                x1: mx,
                y1: my,
            },
        ]
    }
}

pub fn traverse_quadtree<F>(root: Tile, mut decide: F) -> Vec<Tile>
where
    F: FnMut(&Tile) -> TileAction,
{
    let mut results = Vec::new();
    let mut stack = vec![root];

    while let Some(tile) = stack.pop() {
        match decide(&tile) {
            TileAction::Enter => {
                // Push children in reverse order so they're processed in order
                results.push(tile);
                stack.extend(tile.children().into_iter().rev());
            }
            TileAction::Return => {
                results.push(tile);
            }
            TileAction::Drop => {
                // Do nothing
            }
        }
    }

    results
}
