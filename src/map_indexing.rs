use bevy::prelude::*;

use crate::components::BlocksTile;
use crate::map::{Map, Position};

pub fn map_indexing_system(
    mut map: ResMut<Map>,
    query: Query<(Entity, &Position, Option<&BlocksTile>)>,
) {
    map.populate_blocked();
    map.clear_content_index();

    for (entity, position, blocks) in &query {
        let idx = map.xy_idx(position.x, position.y);

        // If they block, update the blocking list
        if blocks.is_some() {
            map.blocked_tiles[idx] = true;
        }

        // Push the entity to the appropriate index slot
        map.tile_content[idx].push(entity);
    }
}
