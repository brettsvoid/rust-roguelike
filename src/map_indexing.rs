use bevy::prelude::*;

use crate::components::{BlocksTile, BlocksVisibility};
use crate::cover::Cover;
use crate::map::{Map, Position};

pub fn map_indexing_system(
    mut map: ResMut<Map>,
    query: Query<(Entity, &Position, Option<&BlocksTile>, Option<&BlocksVisibility>, Option<&Cover>)>,
) {
    map.populate_blocked();
    map.clear_content_index();
    map.view_blocked.clear();
    map.prop_cover.clear();

    for (entity, position, blocks_tile, blocks_visibility, cover) in &query {
        let idx = map.xy_idx(position.x, position.y);

        // If they block movement, update the blocking list
        if blocks_tile.is_some() {
            map.blocked_tiles[idx] = true;
        }

        // If they block visibility, update the view_blocked set
        if blocks_visibility.is_some() {
            map.view_blocked.insert(idx);
        }

        if let Some(&cover) = cover {
            map.prop_cover.insert(idx, cover);
        }

        // Push the entity to the appropriate index slot
        map.tile_content[idx].push(entity);
    }
}
