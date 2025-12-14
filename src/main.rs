use std::time::Duration;

use bevy::prelude::*;
use map::{MapPlugin, GRID_PX, MAP_WIDTH};
use monsters::MonstersPlugin;
use player::PlayerPlugin;
use resources::ResourcesPlugin;
use viewshed::ViewshedPlugin;

mod combat;
mod components;
mod distance;
mod gamelog;
mod gui;
mod inventory;
mod map;
mod map_indexing;
mod monsters;
mod pathfinding;
mod player;
mod resources;
mod rng;
mod saveload;
mod shapes;
mod spawner;
mod viewshed;

const SCREEN_HEIGHT: usize = 50;
const RESOLUTION: Vec2 = Vec2 {
    x: MAP_WIDTH as f32 * GRID_PX.x,
    y: SCREEN_HEIGHT as f32 * GRID_PX.y,
};

#[derive(States, Clone, Copy, Default, Eq, PartialEq, Debug, Hash)]
pub enum RunState {
    #[default]
    MainMenu,
    PreRun,
    AwaitingInput,
    PlayerTurn,
    MonsterTurn,
    ShowInventory,
    ShowDropItem,
    ShowTargeting,
    NextLevel,
}

#[derive(Resource, Default)]
pub struct TargetingInfo {
    pub range: i32,
    pub item: Option<Entity>,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Rust Roguelike".into(),
                resolution: RESOLUTION.into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<RunState>()
        .init_resource::<gamelog::GameLog>()
        .init_resource::<rng::GameRng>()
        .init_resource::<TargetingInfo>()
        .add_event::<AppExit>()
        .add_plugins((
            ResourcesPlugin,
            PlayerPlugin,
            ViewshedPlugin,
            MapPlugin,
            MonstersPlugin,
            gui::GuiPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (map_indexing::map_indexing_system, handle_exit))
        // PreRun: run systems then transition to AwaitingInput
        .add_systems(
            Update,
            transition_to_awaiting_input.run_if(in_state(RunState::PreRun)),
        )
        // PlayerTurn: run combat and item systems then transition to MonsterTurn
        .add_systems(
            Update,
            (
                inventory::item_collection_system,
                inventory::item_use_system,
                inventory::item_drop_system,
                combat::melee_combat_system,
                combat::damage_system,
                combat::delete_the_dead,
                transition_to_monster_turn,
            )
                .chain()
                .run_if(in_state(RunState::PlayerTurn)),
        )
        // MonsterTurn: run monster AI then transition to AwaitingInput
        .add_systems(
            Update,
            (
                monsters::monster_ai,
                combat::melee_combat_system,
                combat::damage_system,
                combat::delete_the_dead,
                transition_to_awaiting_input,
            )
                .chain()
                .run_if(in_state(RunState::MonsterTurn)),
        )
        // NextLevel: generate new level and transition to PreRun
        .add_systems(
            Update,
            go_next_level.run_if(in_state(RunState::NextLevel)),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn handle_exit(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut exit: EventWriter<AppExit>,
    state: Res<State<RunState>>,
    map: Res<map::Map>,
    game_log: Res<gamelog::GameLog>,
    player_query: Query<
        (Entity, &map::Position, &components::Name, &combat::CombatStats, &viewshed::Viewshed),
        With<player::Player>,
    >,
    monster_query: Query<
        (&map::Position, &components::Name, &combat::CombatStats, &viewshed::Viewshed, &Text2d, Option<&components::Confusion>),
        With<monsters::Monster>,
    >,
    item_query: Query<
        (
            Entity,
            &components::Name,
            &Text2d,
            &TextColor,
            Option<&map::Position>,
            Option<&components::InBackpack>,
            Option<&components::Consumable>,
            Option<&components::ProvidesHealing>,
            Option<&components::Ranged>,
            Option<&components::InflictsDamage>,
            Option<&components::AreaOfEffect>,
            Option<&components::Targeting>,
            Option<&components::CausesConfusion>,
        ),
        With<components::Item>,
    >,
) {
    if keyboard.just_released(KeyCode::KeyQ) {
        // Only save if we're in-game (not in MainMenu) and player is alive
        if *state.get() != RunState::MainMenu {
            let player_alive = player_query
                .get_single()
                .map(|(_, _, _, stats, _)| stats.hp > 0)
                .unwrap_or(false);

            if player_alive {
                saveload::save_game(
                    map,
                    game_log,
                    player_query,
                    monster_query,
                    item_query,
                );
            }
        }
        exit.send(AppExit::Success);
    }
}

fn transition_to_awaiting_input(mut next_state: ResMut<NextState<RunState>>) {
    next_state.set(RunState::AwaitingInput);
}

fn transition_to_monster_turn(mut next_state: ResMut<NextState<RunState>>) {
    next_state.set(RunState::MonsterTurn);
}

fn go_next_level(
    mut commands: Commands,
    mut map: ResMut<map::Map>,
    mut gamelog: ResMut<gamelog::GameLog>,
    mut next_state: ResMut<NextState<RunState>>,
    font: Res<resources::UiFont>,
    mut rng: ResMut<rng::GameRng>,
    mut player_query: Query<(Entity, &mut combat::CombatStats), With<player::Player>>,
    backpack_query: Query<(Entity, &components::InBackpack)>,
    entities_to_delete: Query<
        Entity,
        Or<(With<monsters::Monster>, With<map::Tile>, With<components::Item>)>,
    >,
) {
    // Get player entity and items in their backpack
    let Ok((player_entity, mut player_stats)) = player_query.get_single_mut() else {
        return;
    };
    let player_items: Vec<Entity> = backpack_query
        .iter()
        .filter(|(_, backpack)| backpack.owner == player_entity)
        .map(|(entity, _)| entity)
        .collect();

    // Delete all entities except player and their backpack items
    for entity in &entities_to_delete {
        if entity != player_entity && !player_items.contains(&entity) {
            commands.entity(entity).despawn_recursive();
        }
    }

    // Generate new map with increased depth
    let new_map = map::Map::new_map_rooms_and_corridors();
    let new_depth = map.depth + 1;
    *map = new_map;
    map.depth = new_depth;

    // Spawn new map tiles
    let text_font = TextFont {
        font: font.0.clone(),
        font_size: map::FONT_SIZE,
        ..default()
    };

    let mut y = 0;
    let mut x = 0;
    for tile in map.tiles.iter() {
        match tile {
            map::TileType::Floor => {
                commands.spawn((
                    map::Tile,
                    map::Position { x, y },
                    Text2d::new("."),
                    text_font.clone(),
                    TextColor(Color::srgb(0.5, 0.5, 0.5)),
                    map::Revealed(map::RevealedState::Hidden),
                ));
            }
            map::TileType::Wall => {
                commands.spawn((
                    map::Tile,
                    map::Position { x, y },
                    Text2d::new("#"),
                    text_font.clone(),
                    TextColor(Color::srgb(0.0, 1.0, 0.0)),
                    map::Revealed(map::RevealedState::Hidden),
                ));
            }
            map::TileType::DownStairs => {
                commands.spawn((
                    map::Tile,
                    map::Position { x, y },
                    Text2d::new(">"),
                    text_font.clone(),
                    TextColor(Color::srgb(0.0, 1.0, 1.0)),
                    map::Revealed(map::RevealedState::Hidden),
                ));
            }
        }

        x += 1;
        if x > map::MAP_WIDTH as i32 - 1 {
            x = 0;
            y += 1;
        }
    }

    // Spawn monsters and items in new rooms (skip first room - player starts there)
    let mut monster_id: usize = 0;
    for room in map.rooms.iter().skip(1) {
        spawner::spawn_room(&mut commands, &mut rng, &text_font, room, &mut monster_id);
    }

    // Move player to first room center
    let (player_x, player_y) = map.rooms[0].center();
    commands
        .entity(player_entity)
        .insert(map::Position { x: player_x, y: player_y });

    // Heal player (restore up to 50% of max HP)
    let heal_amount = player_stats.max_hp / 2;
    player_stats.hp = (player_stats.hp + heal_amount).min(player_stats.max_hp);

    // Mark player's viewshed as dirty to recalculate visibility
    commands
        .entity(player_entity)
        .insert(viewshed::Viewshed {
            range: 8,
            visible_tiles: Vec::new(),
            dirty: true,
        });

    gamelog.entries.push(format!(
        "You descend to level {}. You feel slightly rejuvenated.",
        new_depth
    ));

    next_state.set(RunState::PreRun);
}

fn run_loop(mut app: App) -> AppExit {
    //let mut exit_event_reader = app.world().resource_mut::<Events<AppExit>>().get_cursor();

    loop {
        //let run_state = app.world().resource::<State<RunState>>();
        //if run_state.get() == &RunState::Running {
        app.update();
        //}

        // Check if we got an exit event, etc...
        if let Some(exit) = app.should_exit() {
            return exit;
        }

        // Check if we should exit
        {
            //let exit_events = app.world().resource::<Events<AppExit>>();
            //for exit in exit_event_reader.read(exit_events) {
            //    return exit.clone();
            //}

            //if !exit_events.is_empty() {
            //    // Return the first exit event found.
            //    if let Some(exit) = exit_events.iter().next() {
            //        return exit.clone();
            //    }
            //}
        }

        // Small sleep to avoid busy-looping (adjust as needed)
        std::thread::sleep(Duration::from_millis(16));
    }
}
