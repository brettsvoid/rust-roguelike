//! Screenshot harness: `cargo run -- --screenshot out.png [--seed N] [--depth N]
//! [--reveal-map] [--frames N]` starts a seeded new game, waits, saves a PNG
//! of the window and exits. Every visual change is verified with it.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot, ScreenshotCaptured};
use bevy::window::{CursorOptions, PrimaryWindow};

use crate::debug::DebugState;
use crate::levels;
use crate::map::Map;
use crate::resources::UiFont;
use crate::rng::GameRng;
use crate::RunState;

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
        app.insert_resource(GameRng::seeded(args.seed))
            .insert_resource(args)
            .add_systems(Startup, ignore_mouse)
            .add_systems(
                Update,
                (
                    start_game.run_if(in_state(RunState::MainMenu)),
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
}

/// `Ok(None)` when `--screenshot` is absent, so the game starts normally.
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<ScreenshotArgs>, String> {
    let mut path = None;
    let mut seed = 42;
    let mut depth = 1;
    let mut reveal_map = false;
    let mut frames = 60;

    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        if flag == "--reveal-map" {
            reveal_map = true;
            continue;
        }
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        let bad = |_| format!("bad value for {flag}: {value}");
        match flag.as_str() {
            "--screenshot" => path = Some(PathBuf::from(&value)),
            "--seed" => seed = value.parse().map_err(bad)?,
            "--depth" => depth = value.parse().map_err(bad)?,
            "--frames" => frames = value.parse().map_err(bad)?,
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
    mut next_state: ResMut<NextState<RunState>>,
) {
    levels::spawn_new_game(&mut commands, &mut map, &mut rng, &font, args.depth);
    if args.reveal_map {
        debug_state.reveal_map = true;
        debug_state.no_fog = true;
    }
    next_state.set_if_different(RunState::PreRun);
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
            parse("--screenshot a.png --seed 7 --depth 3 --reveal-map --frames 10"),
            Ok(Some(ScreenshotArgs {
                path: PathBuf::from("a.png"),
                seed: 7,
                depth: 3,
                reveal_map: true,
                frames: 10,
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
    }
}
