use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::components::{AreaOfEffect, Name, Targeting, WantsToUseItem};
use crate::distance::DistanceAlg;
use crate::map::{Map, Position, TileType};
use crate::monsters::Monster;
use crate::picking::{tile_edge, update_hovered_tile, HoveredTile, TileMarks, FULL_TILE};
use crate::party::{ActiveUnit, PartyMember};
use crate::RunState;

use super::components::{RangeIndicator, TargetBorder, TargetHighlight};
use super::panel::{close_panels, show, spawn_panel, BodyLine, Hint, Panel, PanelSpec, Placement};

/// What the active unit is currently aiming: set when a ranged item is selected
/// from the inventory or a targeted ability's hotkey is pressed (`item` is
/// then the ability entity), read while the targeting overlay is up.
#[derive(Resource, Default)]
pub struct TargetingInfo {
    pub range: i32,
    pub item: Option<Entity>,
}

pub struct TargetingPlugin;

impl Plugin for TargetingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TargetingInfo>().add_systems(
            OnEnter(RunState::ShowTargeting),
            (spawn_targeting_ui, spawn_target_borders, spawn_range_indicator),
        )
        .add_systems(OnExit(RunState::ShowTargeting), (despawn_targeting_ui, close_panels))
        .add_systems(
            Update,
            (update_target_highlight, handle_targeting, write_targeting_banner)
                .chain()
                .after(update_hovered_tile)
                .run_if(in_state(RunState::ShowTargeting)),
        );
    }
}

fn spawn_targeting_ui(mut commands: Commands) {
    spawn_panel(&mut commands, Placement::Banner);
}

/// The banner over the world: what is aimed, its range, what to click.
fn targeting_spec(name: &str, range: i32, targeting: Targeting) -> PanelSpec {
    let what = match targeting {
        Targeting::SingleEntity => "a foe",
        Targeting::Ally => "a hero",
        Targeting::Tile => "a tile",
    };
    PanelSpec {
        title: name.to_string(),
        aside: format!("Range {range}"),
        body: vec![BodyLine::Text(format!("Click {what} in range."))],
        hints: vec![Hint::new("Esc", "Cancel", KeyCode::Escape)],
        ..default()
    }
}

fn write_targeting_banner(
    targeting_info: Res<TargetingInfo>,
    items: Query<(&Name, Option<&Targeting>)>,
    mut panels: Query<(&Panel, &mut PanelSpec)>,
) {
    let (name, targeting) = targeting_info
        .item
        .and_then(|item| items.get(item).ok())
        .map_or(("Select a target", Targeting::Tile), |(name, t)| (name.name.as_str(), t.copied().unwrap_or_default()));
    show(&mut panels, targeting_spec(name, targeting_info.range, targeting));
}

fn despawn_targeting_ui(
    mut commands: Commands,
    highlight_query: Query<Entity, With<TargetHighlight>>,
    border_query: Query<Entity, With<TargetBorder>>,
    range_query: Query<Entity, With<RangeIndicator>>,
) {
    for entity in &highlight_query {
        commands.entity(entity).despawn();
    }
    for entity in &border_query {
        commands.entity(entity).despawn();
    }
    for entity in &range_query {
        commands.entity(entity).despawn();
    }
}

fn spawn_target_borders(
    mut commands: Commands,
    marks: Res<TileMarks>,
    map: Res<Map>,
    targeting_info: Res<TargetingInfo>,
    active: Res<ActiveUnit>,
    party: Query<&Position, With<PartyMember>>,
    monster_query: Query<&Position, With<Monster>>,
    targeting_query: Query<&Targeting>,
) {
    // Only show borders for items that require an entity target
    let Some(item) = targeting_info.item else {
        return;
    };
    let candidates: Vec<&Position> = match targeting_query.get(item).copied().unwrap_or_default() {
        Targeting::SingleEntity => monster_query.iter().collect(),
        Targeting::Ally => party.iter().collect(),
        Targeting::Tile => return,
    };

    let Some(player_pos) = active.0.and_then(|unit| party.get(unit).ok()) else {
        return;
    };

    // Find all candidates within range and on visible tiles
    for monster_pos in candidates {
        let idx = map.xy_idx(monster_pos.x, monster_pos.y);
        if !map.visible_tiles[idx] {
            continue;
        }
        if distance(player_pos, monster_pos.x, monster_pos.y) <= targeting_info.range as f32 {
            for (dx, dy) in CARDINALS {
                commands.spawn((
                    marks.mark(&marks.target, &map, (monster_pos.x, monster_pos.y), tile_edge(dx, dy, 0.08), 0.01),
                    TargetBorder,
                ));
            }
        }
    }
}

const CARDINALS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

fn distance(from: &Position, x: i32, y: i32) -> f32 {
    DistanceAlg::Euclidean.distance2d(Vec2::new(from.x as f32, from.y as f32), Vec2::new(x as f32, y as f32))
}

fn spawn_range_indicator(
    mut commands: Commands,
    marks: Res<TileMarks>,
    map: Res<Map>,
    targeting_info: Res<TargetingInfo>,
    active: Res<ActiveUnit>,
    party: Query<&Position, With<PartyMember>>,
) {
    let Some(player_pos) = active.0.and_then(|unit| party.get(unit).ok()) else {
        return;
    };
    let range = targeting_info.range;
    let in_map = |x: i32, y: i32| x >= 0 && x < map.width && y >= 0 && y < map.height;
    let is_floor = |x: i32, y: i32| in_map(x, y) && map.tiles[map.xy_idx(x, y)] == TileType::Floor;

    // Outline the edge of range: visible floor in range that borders floor out of range.
    for dy in -range..=range {
        for dx in -range..=range {
            let (tile_x, tile_y) = (player_pos.x + dx, player_pos.y + dy);
            if !is_floor(tile_x, tile_y)
                || !map.visible_tiles[map.xy_idx(tile_x, tile_y)]
                || distance(player_pos, tile_x, tile_y) > range as f32
            {
                continue;
            }
            for (nx, ny) in CARDINALS {
                let (x, y) = (tile_x + nx, tile_y + ny);
                if is_floor(x, y) && distance(player_pos, x, y) > range as f32 {
                    commands.spawn((
                        marks.mark(&marks.range, &map, (tile_x, tile_y), tile_edge(nx, ny, 0.05), 0.0),
                        RangeIndicator,
                    ));
                }
            }
        }
    }
}

fn update_target_highlight(
    mut commands: Commands,
    marks: Res<TileMarks>,
    hovered: Res<HoveredTile>,
    map: Res<Map>,
    targeting_info: Res<TargetingInfo>,
    active: Res<ActiveUnit>,
    party: Query<&Position, With<PartyMember>>,
    monster_query: Query<&Position, With<Monster>>,
    highlight_query: Query<Entity, With<TargetHighlight>>,
    aoe_query: Query<&AreaOfEffect>,
    targeting_query: Query<&Targeting>,
) {
    for entity in &highlight_query {
        commands.entity(entity).despawn();
    }

    let Some(player_pos) = active.0.and_then(|unit| party.get(unit).ok()) else {
        return;
    };
    let Some((map_x, map_y)) = valid_target(&hovered, &map, player_pos, &targeting_info) else {
        return;
    };
    let Some(item) = targeting_info.item else {
        return;
    };

    match targeting_query.get(item).copied().unwrap_or_default() {
        targeting @ (Targeting::SingleEntity | Targeting::Ally) => {
            if holds_target(targeting, (map_x, map_y), &party, &monster_query) {
                commands.spawn((
                    marks.mark(&marks.target, &map, (map_x, map_y), FULL_TILE, 0.01),
                    TargetHighlight,
                ));
            }
        }
        Targeting::Tile => {
            // Show AoE radius if item has AreaOfEffect
            if let Ok(aoe) = aoe_query.get(item) {
                let radius = aoe.radius;
                let centre = Position { x: map_x, y: map_y };
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        let (x, y) = (map_x + dx, map_y + dy);
                        if x < 0 || x >= map.width || y < 0 || y >= map.height {
                            continue;
                        }
                        if distance(&centre, x, y) <= radius as f32 {
                            commands.spawn((marks.mark(&marks.area, &map, (x, y), FULL_TILE, 0.01), TargetHighlight));
                        }
                    }
                }
            }
        }
    }
}

/// Whether `cell` holds what an item aims at: a single-entity item needs a
/// monster on it, an ally item a hero; a tile item takes any.
fn holds_target(
    targeting: Targeting,
    cell: (i32, i32),
    party: &Query<&Position, With<PartyMember>>,
    monsters: &Query<&Position, With<Monster>>,
) -> bool {
    let on = |pos: &Position| (pos.x, pos.y) == cell;
    match targeting {
        Targeting::Tile => true,
        Targeting::SingleEntity => monsters.iter().any(on),
        Targeting::Ally => party.iter().any(on),
    }
}

/// The hovered tile when it is a visible tile within the item's range.
fn valid_target(
    hovered: &HoveredTile,
    map: &Map,
    player_pos: &Position,
    targeting_info: &TargetingInfo,
) -> Option<(i32, i32)> {
    hovered.tile.filter(|&(x, y)| {
        map.visible_tiles[map.xy_idx(x, y)] && distance(player_pos, x, y) <= targeting_info.range as f32
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_banner_names_the_item_its_range_and_what_to_click() {
        let spec = targeting_spec("Fireball Scroll", 6, Targeting::Tile);
        assert_eq!((spec.title.as_str(), spec.aside.as_str()), ("Fireball Scroll", "Range 6"));
        assert_eq!(spec.body, [BodyLine::Text("Click a tile in range.".into())]);
        assert_eq!(targeting_spec("Heal", 1, Targeting::Ally).body, [BodyLine::Text("Click a hero in range.".into())]);
    }
}

pub fn handle_targeting(
    mut commands: Commands,
    mut evr_kbd: MessageReader<KeyboardInput>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredTile>,
    map: Res<Map>,
    targeting_info: Res<TargetingInfo>,
    mut next_state: ResMut<NextState<RunState>>,
    active: Res<ActiveUnit>,
    party: Query<&Position, With<PartyMember>>,
    monster_query: Query<&Position, With<Monster>>,
    targeting_query: Query<&Targeting>,
) {
    // Handle escape to cancel
    for ev in evr_kbd.read() {
        if ev.state == ButtonState::Pressed && ev.key_code == KeyCode::Escape {
            next_state.set_if_different(RunState::AwaitingInput);
            return;
        }
    }

    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }
    let Some((player_entity, player_pos)) = active.0.and_then(|unit| Some((unit, party.get(unit).ok()?))) else {
        return;
    };
    let Some((map_x, map_y)) = valid_target(&hovered, &map, player_pos, &targeting_info) else {
        return;
    };
    let Some(item) = targeting_info.item else {
        return;
    };

    let targeting = targeting_query.get(item).copied().unwrap_or_default();
    if !holds_target(targeting, (map_x, map_y), &party, &monster_query) {
        return;
    }
    commands.entity(player_entity).insert(WantsToUseItem {
        item,
        target: Some((map_x, map_y)),
    });
    next_state.set_if_different(RunState::PartyAction);
}
