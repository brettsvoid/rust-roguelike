//! Screenshot harness: `cargo run -- --screenshot out.png [--seed N] [--depth N]
//! [--reveal-map] [--frames N] [--console CMD]... [--ledger] [--cursor X,Y]
//! [--target ITEM] [--keys KEYS]` starts a seeded new game, waits, saves a PNG
//! of the window and exits. `--console` runs a debug console command once play
//! begins (repeatable); `town ...` commands run before the level is built
//! instead, e.g. "town level smithing 4"; `--ledger` then opens the town ledger.
//! `--cursor` fakes the mouse at window point X,Y (logical pixels); `--target`
//! enters targeting with the named item (e.g. `--console "spawn fireball"
//! --target "Fireball Scroll"`). `--keys hjkl` presses one key per turn through
//! the real input system (h j k l y u n m g and `.`); a key that takes no turn
//! stalls the rest. Every visual change is verified with it.

use std::path::PathBuf;

use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot, ScreenshotCaptured};
use bevy::window::{CursorOptions, PrimaryWindow};

use crate::combat::CombatStats;
use crate::components::{Name, Ranged};
use crate::debug::{DebugState, GodMode};
use crate::economy::town::TownState;
use crate::levels;
use crate::map::{Map, Position};
use crate::monsters::Monster;
use crate::picking::FakeCursor;
use crate::player::Player;
use crate::resources::UiFont;
use crate::rng::GameRng;
use crate::{RunState, TargetingInfo};

/// Does nothing unless the command line asks for a screenshot.
pub struct ScreenshotPlugin;

impl Plugin for ScreenshotPlugin {
    fn build(&self, app: &mut App) {
        let args = parse_args(std::env::args().skip(1)).unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(2);
        });
        let Some(args) = args else {
            return;
        };
        if let Some(cursor) = args.cursor {
            app.insert_resource(FakeCursor(cursor));
        }
        app.insert_resource(GameRng::seeded(args.seed))
            .insert_resource(args)
            .add_systems(Startup, ignore_mouse)
            .add_systems(OnEnter(RunState::AwaitingInput), press_next_key)
            .add_systems(
                Update,
                (
                    start_game.run_if(in_state(RunState::MainMenu)),
                    (prepare_capture, enter_targeting).run_if(in_state(RunState::AwaitingInput)),
                    capture_after_frames.run_if(not(in_state(RunState::MainMenu))),
                ),
            );
    }
}

#[derive(Resource, Debug, PartialEq)]
pub struct ScreenshotArgs {
    pub path: PathBuf,
    pub seed: u64,
    pub depth: i32,
    pub reveal_map: bool,
    pub frames: u32,
    pub console: Vec<String>,
    pub ledger: bool,
    pub cursor: Option<Vec2>,
    pub target: Option<String>,
    pub keys: Vec<KeyCode>,
}

fn key_code(c: char) -> Option<KeyCode> {
    Some(match c {
        'h' => KeyCode::KeyH,
        'j' => KeyCode::KeyJ,
        'k' => KeyCode::KeyK,
        'l' => KeyCode::KeyL,
        'y' => KeyCode::KeyY,
        'u' => KeyCode::KeyU,
        'n' => KeyCode::KeyN,
        'm' => KeyCode::KeyM,
        'g' => KeyCode::KeyG,
        '.' => KeyCode::Period,
        _ => return None,
    })
}

/// `Ok(None)` when `--screenshot` is absent, so the game starts normally.
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<ScreenshotArgs>, String> {
    let mut path = None;
    let mut seed = 42;
    let mut depth = 1;
    let mut reveal_map = false;
    let mut frames = 60;
    let mut console = Vec::new();
    let mut ledger = false;
    let mut cursor = None;
    let mut target = None;
    let mut keys = Vec::new();

    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        if flag == "--reveal-map" {
            reveal_map = true;
            continue;
        }
        if flag == "--ledger" {
            ledger = true;
            continue;
        }
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        let bad = || format!("bad value for {flag}: {value}");
        match flag.as_str() {
            "--screenshot" => path = Some(PathBuf::from(&value)),
            "--seed" => seed = value.parse().map_err(|_| bad())?,
            "--depth" => depth = value.parse().map_err(|_| bad())?,
            "--frames" => frames = value.parse().map_err(|_| bad())?,
            "--console" => console.push(value),
            "--cursor" => {
                let (x, y) = value.split_once(',').ok_or_else(bad)?;
                cursor = Some(Vec2::new(x.parse().map_err(|_| bad())?, y.parse().map_err(|_| bad())?));
            }
            "--target" => target = Some(value),
            "--keys" => {
                keys = value
                    .chars()
                    .map(|c| key_code(c).ok_or(format!("--keys: no key for {c:?}")))
                    .collect::<Result<_, _>>()?
            }
            _ => return Err(format!("unknown argument: {flag}")),
        }
    }

    if depth < 1 {
        return Err("--depth must be 1 or more".into());
    }
    if frames < 1 {
        return Err("--frames must be 1 or more".into());
    }
    Ok(path.map(|path| ScreenshotArgs {
        path,
        seed,
        depth,
        reveal_map,
        frames,
        console,
        ledger,
        cursor,
        target,
        keys,
    }))
}

/// A cursor resting over the window would pop up tooltips and change the picture.
fn ignore_mouse(mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor.hit_test = false;
}

/// Skip the main menu: build the requested level as a new game.
fn start_game(
    mut commands: Commands,
    args: Res<ScreenshotArgs>,
    mut map: ResMut<Map>,
    mut rng: ResMut<GameRng>,
    font: Res<UiFont>,
    mut debug_state: ResMut<DebugState>,
    mut town: ResMut<TownState>,
    mut next_state: ResMut<NextState<RunState>>,
) {
    // Town commands run before the level is built, so the town shows them.
    for cmd in args.console.iter().filter(|c| is_town_command(c)) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        info!("console> {cmd}: {}", super::commands::cmd_town(&parts, &mut town));
    }
    levels::spawn_new_game(&mut commands, &mut map, &mut rng, &font, &town, args.depth);
    if args.reveal_map {
        debug_state.reveal_map = true;
        debug_state.no_fog = true;
    }
    next_state.set_if_different(RunState::PreRun);
}

fn is_town_command(cmd: &str) -> bool {
    cmd.split_whitespace().next() == Some("town")
}

/// Once play begins: run the other `--console` commands, then open the ledger.
#[allow(clippy::too_many_arguments)]
fn prepare_capture(
    mut commands: Commands,
    args: Res<ScreenshotArgs>,
    mut done: Local<bool>,
    mut debug_state: ResMut<DebugState>,
    mut god_mode: ResMut<GodMode>,
    mut player_query: Query<(Entity, &mut Position, &mut CombatStats), With<Player>>,
    monster_query: Query<Entity, With<Monster>>,
    mut town: ResMut<TownState>,
    mut next_state: ResMut<NextState<RunState>>,
) {
    if std::mem::replace(&mut *done, true) {
        return;
    }
    for cmd in args.console.iter().filter(|c| !is_town_command(c)) {
        let out = super::commands::execute_command(
            cmd,
            &mut commands,
            &mut debug_state,
            &mut god_mode,
            &mut player_query,
            &monster_query,
            &mut town,
        );
        info!("console> {cmd}: {out}");
    }
    if args.ledger {
        next_state.set_if_different(RunState::ShowLedger);
    }
}

/// Aims the `--target` item once it exists (a `--console` spawn lands a frame later).
fn enter_targeting(
    args: Res<ScreenshotArgs>,
    items: Query<(Entity, &Name, &Ranged)>,
    mut targeting_info: ResMut<TargetingInfo>,
    mut next_state: ResMut<NextState<RunState>>,
) {
    let Some(target) = &args.target else {
        return;
    };
    if let Some((item, _, ranged)) = items.iter().find(|(_, name, _)| &name.name == target) {
        targeting_info.range = ranged.range;
        targeting_info.item = Some(item);
        next_state.set_if_different(RunState::ShowTargeting);
    }
}

/// Each time the game waits for input, press the next `--keys` key. Runs on
/// entering the state, so the key lands in this frame's input handling and
/// the next waits for the turn to come round.
fn press_next_key(
    args: Res<ScreenshotArgs>,
    mut next: Local<usize>,
    window: Single<Entity, With<PrimaryWindow>>,
    mut keyboard: MessageWriter<KeyboardInput>,
) {
    let Some(&key_code) = args.keys.get(*next) else {
        return;
    };
    *next += 1;
    keyboard.write(KeyboardInput {
        key_code,
        logical_key: Key::Unidentified(NativeKey::Unidentified),
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: *window,
    });
}

/// Counts frames after the game starts, then captures the window once.
fn capture_after_frames(mut commands: Commands, args: Res<ScreenshotArgs>, mut frame: Local<u32>) {
    *frame += 1;
    if *frame != args.frames {
        return;
    }
    let mut save = save_to_disk(args.path.clone());
    commands.spawn(Screenshot::primary_window()).observe(
        move |captured: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            save(captured);
            exit.write(AppExit::Success);
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Result<Option<ScreenshotArgs>, String> {
        parse_args(line.split_whitespace().map(String::from))
    }

    #[test]
    fn no_flags_means_normal_game() {
        assert_eq!(parse(""), Ok(None));
    }

    #[test]
    fn all_flags() {
        assert_eq!(
            parse("--screenshot a.png --seed 7 --depth 3 --reveal-map --frames 10 --console heal --console godmode --ledger --cursor 640,400.5 --target Dagger --keys hl."),
            Ok(Some(ScreenshotArgs {
                path: PathBuf::from("a.png"),
                seed: 7,
                depth: 3,
                reveal_map: true,
                frames: 10,
                console: vec!["heal".into(), "godmode".into()],
                ledger: true,
                cursor: Some(Vec2::new(640.0, 400.5)),
                target: Some("Dagger".into()),
                keys: vec![KeyCode::KeyH, KeyCode::KeyL, KeyCode::Period],
            }))
        );
    }

    #[test]
    fn defaults() {
        assert_eq!(
            parse("--screenshot a.png"),
            Ok(Some(ScreenshotArgs {
                path: PathBuf::from("a.png"),
                seed: 42,
                depth: 1,
                reveal_map: false,
                frames: 60,
                console: vec![],
                ledger: false,
                cursor: None,
                target: None,
                keys: vec![],
            }))
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("--screenshot a.png --depth 0").is_err());
        assert!(parse("--screenshot a.png --frames 0").is_err());
        assert!(parse("--screenshot a.png --seed x").is_err());
        assert!(parse("--screenshot").is_err());
        assert!(parse("--bogus 1").is_err());
        assert!(parse("--screenshot a.png --cursor 640").is_err());
        assert!(parse("--screenshot a.png --cursor 6x,4").is_err());
        assert!(parse("--screenshot a.png --keys hx").is_err());
    }
}
