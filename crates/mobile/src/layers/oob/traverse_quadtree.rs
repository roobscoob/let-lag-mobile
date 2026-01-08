use jet_lag_core::map::tile::Tile;

pub enum TileAction {
    Enter,  // Subdivide into 4 children
    Drop,   // Skip this tile
    Return, // Include in results, don't subdivide
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
