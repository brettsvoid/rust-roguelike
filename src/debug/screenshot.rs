//! Screenshot harness: `cargo run -- --screenshot out.png [--seed N] [--depth N] [--menu]
//! [--reveal-map] [--frames N] [--every N] [--console CMD]... [--ledger] [--cursor X,Y]
//! [--target ITEM] [--keys KEYS]... [--click X,Y]... [--press X,Y]... [--shop VENDOR]...` starts
//! a seeded new game, waits, saves a PNG of the window and exits. `--depth` 0 (the
//! default) is the town, 1 to 12 the dungeon. `--console` runs a debug console
//! command once play begins (repeatable); `town ...` commands run before the
//! level is built instead, e.g. "town level smithing 4"; `--ledger` then opens
//! the town ledger. `--cursor` fakes the mouse at window point X,Y (logical
//! pixels of the 1600x1000 capture); `--target` enters targeting with the named item (e.g. `--console
//! "spawn fireball" --target "Fireball Scroll"`). `--keys`, `--click`, `--press` and `--shop` queue
//! input in the order given, fed through the real input systems one at a time
//! while the game waits for input: `--keys` presses keys (`g`, `.`, `i` for the
//! inventory, `r` to remove equipment, `t` for the town ledger, `n` for Tab to select the next hero,
//! space to wait, `e` to end the party phase in combat, `o` overwatch and `h` hunker down,
//! `1` and `2` the active hero's abilities, `w` `a` `s` `d` to pan, each held for
//! `PAN_HOLD_FRAMES` before the next input, `c` to recentre, `+` and `-` scroll the
//! wheel a notch to zoom in and out; the camera eases, so a capture taken soon
//! after shows it mid-motion); `--click X,Y` left-clicks grid cell X,Y as if the mouse hovered it (a click on another
//! hero selects them; in the targeting screen it picks that cell as the target), and the next
//! input waits until the walk it starts ends;
//! `--press X,Y` left-clicks capture point X,Y (pixels of the 1600x1000
//! image) through the HUD's own click handling, e.g. a party card or an action
//! button. In the party phase with the default HUD the buttons sit on
//! y = 944 at x = 406 (Attack), 498 and 590 (the abilities), 682 (Overwatch),
//! 774 (Hunker), 866 (Use Item), 958 (Wait) and 1050 (End Turn); exploring, the
//! abilities, Use Item and Wait sit at 590, 682, 774 and 866; the party cards are
//! at x = 150, y = 889 + 30 per slot;
//! `--shop VENDOR` clicks that vendor (e.g. `blacksmith`), so the active hero
//! walks beside them and their shop opens (the vendor's tile must be explored,
//! so pair it with `--reveal-map`). Inputs are also fed while a shop is open:
//! `n` (Tab) switches buy and sell, the letters `a` to `j` trade that line and
//! `!` (Escape) closes it. Inputs are fed into the inventory too: `i` then `a`
//! uses the first item, e.g. `--console "spawn portal" --keys gia` reads a Town
//! Portal (`spawn heart` drops the Heart of the Abyss to test the refusal).
//! `--do CMD` queues a debug console command with the inputs, run in its
//! turn (e.g. `--keys g --do "depth 1" --keys .` takes an item, jumps to a fresh
//! depth 1 and takes its up stairs).
//! `--every N` also saves every Nth frame before the last as `out-FFFF.png`
//! next to `out.png`, a sequence to look at motion with.
//! `--reload`, queued with the inputs, saves the game, loads it back and logs
//! "Reload: ..." with the mode and whether the loaded game snapshots the same
//! as the saved one (`saveload::snapshot`). `--load` takes Continue at the main
//! menu instead of starting a new game (a scene's `files` can put a
//! `savegame.json` beside it); without a save that loads, the capture shows
//! the main menu.
//! `--menu` stays on the main menu and captures it instead of starting a game.
//! `--visualise N` opens the Map Visualiser on builder N of its menu (0 is the first)
//! instead, so a capture shows the flipbook's glyphs mid-generation.
//! Every visual change is verified with it.

use std::path::PathBuf;
use std::time::Duration;

use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::input::ButtonState;
use bevy::camera::{ImageRenderTarget, RenderTarget, ShadowLodOrigin};
use bevy::prelude::*;
use bevy::render::render_resource::{PollType, TextureFormat};
use bevy::render::view::screenshot::{save_to_disk, Screenshot, ScreenshotCaptured};
use bevy::time::TimeUpdateStrategy;
use bevy::window::{CursorOptions, PrimaryWindow};

use crate::bystander::Bystander;
use crate::combat::CombatStats;
use crate::components::{Name, Ranged};
use crate::debug::{DebugState, GodMode};
use crate::economy::town::{TownState, Vendor};
use crate::gamelog::GameLog;
use crate::levels;
use crate::map::{Map, Position};
use crate::mapgen_viz::{start_preview, MapGenBuilderName, MapGenHistory};
use crate::models::AvailableModels;
use crate::monsters::Monster;
use crate::picking::{self, FakeCursor, HoveredTile};
use crate::party::{ActiveUnit, PartyMember};
use crate::player::{self, WalkPath};
use crate::presentation::Presentation;
use crate::resources::UiFont;
use crate::ui::menus::{inventory, shop};
use crate::ui::targeting;
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
        let gpu_wait = GpuWait::default();
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app
                .insert_resource(gpu_wait.clone())
                .add_systems(bevy::render::Render, pace_gpu.in_set(bevy::render::RenderSystems::Cleanup));
        }
        app.insert_resource(gpu_wait)
            .insert_resource(GameRng::seeded(args.seed))
            // Fixed steps make anything driven by `Time` (flickering lights)
            // the same every run.
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
            .insert_resource(args)
            .add_systems(Startup, ignore_mouse)
            .add_systems(PostStartup, render_to_image)
            .add_systems(
                Update,
                (
                    feed_next_input
                        .run_if(
                            in_state(RunState::AwaitingInput)
                                .or_else(in_state(RunState::ShowShop))
                                .or_else(in_state(RunState::ShowInventory))
                                .or_else(in_state(RunState::ShowTargeting)),
                        )
                        .after(picking::update_hovered_tile)
                        .before(shop::click_vendor)
                        .before(shop::handle_shop_input)
                        .before(inventory::handle_inventory_input)
                        .before(targeting::handle_targeting)
                        .before(crate::ui::hud::click_hud)
                        .before(player::handle_player_input),
                    // Models load async; starting only once they are in keeps the
                    // picture the same run to run.
                    start_game
                        .run_if(in_state(RunState::MainMenu))
                        .run_if(not(resource_exists::<Started>))
                        .run_if(models_loaded),
                    (prepare_capture, enter_targeting).run_if(in_state(RunState::AwaitingInput)),
                    reveal_each_level.run_if(in_state(RunState::AwaitingInput)).before(feed_next_input),
                    // A `--load` that fails stays at the main menu, which is then the picture.
                    capture_after_frames.run_if(not(in_state(RunState::MainMenu)).or_else(resource_exists::<Started>)),
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
    /// Capture the main menu rather than start a game.
    pub menu: bool,
    /// Open the Map Visualiser on this builder rather than start a game.
    pub visualise: Option<usize>,
    pub frames: u32,
    pub every: Option<u32>,
    pub console: Vec<String>,
    pub ledger: bool,
    pub cursor: Option<Vec2>,
    pub target: Option<String>,
    pub inputs: Vec<HarnessInput>,
    /// Continue a saved game instead of starting a new one.
    pub load: bool,
}

/// One queued `--keys` key or `--click`.
#[derive(Debug, PartialEq)]
pub enum HarnessInput {
    Key(KeyCode),
    /// Wheel notches, up (zoom in) positive.
    Scroll(f32),
    Click(i32, i32),
    /// A left click at a capture point, for the HUD.
    Press(Vec2),
    /// Click the vendor's townsperson.
    Shop(Vendor),
    /// Save, then load the save back (`reload_game`).
    Reload,
    /// `--do`: a debug console command in its turn among the inputs.
    Console(String),
}

fn key_input(c: char) -> Option<HarnessInput> {
    let key = match c {
        '+' => return Some(HarnessInput::Scroll(1.0)),
        '-' => return Some(HarnessInput::Scroll(-1.0)),
        '!' => KeyCode::Escape,
        ' ' => KeyCode::Space,
        '1' => KeyCode::Digit1,
        '2' => KeyCode::Digit2,
        'b' => KeyCode::KeyB,
        'e' => KeyCode::KeyE,
        'f' => KeyCode::KeyF,
        'h' => KeyCode::KeyH,
        'o' => KeyCode::KeyO,
        'r' => KeyCode::KeyR,
        'j' => KeyCode::KeyJ,
        'g' => KeyCode::KeyG,
        '.' => KeyCode::Period,
        'i' => KeyCode::KeyI,
        't' => KeyCode::KeyT,
        'n' => KeyCode::Tab,
        'w' => KeyCode::KeyW,
        'a' => KeyCode::KeyA,
        's' => KeyCode::KeyS,
        'd' => KeyCode::KeyD,
        'c' => KeyCode::KeyC,
        _ => return None,
    };
    Some(HarnessInput::Key(key))
}

/// `Ok(None)` when `--screenshot` is absent, so the game starts normally.
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<ScreenshotArgs>, String> {
    let mut path = None;
    let mut seed = 42;
    let mut depth = levels::TOWN_DEPTH;
    let mut reveal_map = false;
    let mut menu = false;
    let mut visualise = None;
    let mut frames = 60;
    let mut every = None;
    let mut console = Vec::new();
    let mut ledger = false;
    let mut cursor = None;
    let mut target = None;
    let mut inputs = Vec::new();
    let mut load = false;

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
        if flag == "--reload" {
            inputs.push(HarnessInput::Reload);
            continue;
        }
        if flag == "--load" {
            load = true;
            continue;
        }
        if flag == "--menu" {
            menu = true;
            continue;
        }
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        let bad = || format!("bad value for {flag}: {value}");
        match flag.as_str() {
            "--screenshot" => path = Some(PathBuf::from(&value)),
            "--seed" => seed = value.parse().map_err(|_| bad())?,
            "--depth" => depth = value.parse().map_err(|_| bad())?,
            "--frames" => frames = value.parse().map_err(|_| bad())?,
            "--every" => every = Some(value.parse().ok().filter(|&n| n > 0).ok_or_else(bad)?),
            "--console" => console.push(value),
            "--visualise" => visualise = Some(value.parse().map_err(|_| bad())?),
            "--cursor" => {
                let (x, y) = value.split_once(',').ok_or_else(bad)?;
                cursor = Some(Vec2::new(x.parse().map_err(|_| bad())?, y.parse().map_err(|_| bad())?));
            }
            "--target" => target = Some(value),
            "--keys" => {
                for c in value.chars() {
                    inputs.push(key_input(c).ok_or(format!("--keys: no key for {c:?}"))?);
                }
            }
            "--click" => {
                let (x, y) = value.split_once(',').ok_or_else(bad)?;
                inputs.push(HarnessInput::Click(x.parse().map_err(|_| bad())?, y.parse().map_err(|_| bad())?));
            }
            "--press" => {
                let (x, y) = value.split_once(',').ok_or_else(bad)?;
                inputs.push(HarnessInput::Press(Vec2::new(x.parse().map_err(|_| bad())?, y.parse().map_err(|_| bad())?)));
            }
            "--shop" => inputs.push(HarnessInput::Shop(Vendor::from_name(&value).ok_or_else(bad)?)),
            "--do" => inputs.push(HarnessInput::Console(value)),
            _ => return Err(format!("unknown argument: {flag}")),
        }
    }

    if depth < levels::TOWN_DEPTH {
        return Err("--depth must be 0 (the town) or more".into());
    }
    if frames < 1 {
        return Err("--frames must be 1 or more".into());
    }
    Ok(path.map(|path| ScreenshotArgs {
        path,
        seed,
        depth,
        reveal_map,
        menu,
        visualise,
        frames,
        every,
        console,
        ledger,
        cursor,
        target,
        inputs,
        load,
    }))
}

/// A cursor resting over the window would pop up tooltips and change the picture.
/// The picture the harness renders into and saves.
#[derive(Resource)]
struct CaptureTarget(Handle<Image>);

/// The capture size in pixels, whatever the window manager does to the window.
const CAPTURE_SIZE: UVec2 = UVec2::new(1600, 1000);

/// Points both cameras at an off-screen image. A window's picture depends on
/// its size (window managers resize it) and is black while the screen is
/// locked or asleep; an image is neither. `HARNESS_SCALE=2` draws it twice as
/// wide and high, the pixels a Retina window draws, for frame timing: the world
/// shows the same, the HUD at half size (an image target with a scale factor
/// of 2 drew nothing).
fn render_to_image(
    mut commands: Commands,
    cameras: Query<(Entity, Has<Camera3d>), With<Camera>>,
    mut images: ResMut<Assets<Image>>,
) {
    let scale = std::env::var("HARNESS_SCALE").ok().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1).max(1);
    let handle = images.add(Image::new_target_texture(
        CAPTURE_SIZE.x * scale,
        CAPTURE_SIZE.y * scale,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    for (camera, is_3d) in &cameras {
        commands.entity(camera).insert(RenderTarget::Image(ImageRenderTarget {
            handle: handle.clone(),
            scale_factor: 1.0,
        }));
        if is_3d {
            // Shadow detail is picked from a window camera unless one is named.
            commands.entity(camera).insert(ShadowLodOrigin);
        }
    }
    commands.insert_resource(CaptureTarget(handle));
}

fn ignore_mouse(mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor.hit_test = false;
}

/// Set once the harness has started the game (or tried to load it).
#[derive(Resource)]
struct Started;

/// Skip the main menu: build the requested level as a new game, or with
/// `--load` continue the saved one.
#[allow(clippy::too_many_arguments)]
fn start_game(
    mut commands: Commands,
    args: Res<ScreenshotArgs>,
    mut map: ResMut<Map>,
    mut rng: ResMut<GameRng>,
    font: Res<UiFont>,
    mut debug_state: ResMut<DebugState>,
    mut town: ResMut<TownState>,
    mut next_state: ResMut<NextState<RunState>>,
    (mut history, mut builder_name): (ResMut<MapGenHistory>, ResMut<MapGenBuilderName>),
) {
    commands.insert_resource(Started);
    if args.menu {
        return;
    }
    if let Some(index) = args.visualise {
        // As the builder menu does on a pick.
        start_preview(&mut map, &mut rng, &mut history, &mut builder_name, Some(index));
        next_state.set_if_different(RunState::MapGeneration);
        return;
    }
    if args.reveal_map {
        debug_state.reveal_map = true;
        debug_state.no_fog = true;
    }
    if args.load {
        commands.queue(crate::saveload::continue_game);
        return;
    }
    // Town commands run before the level is built, so the town shows them.
    for cmd in args.console.iter().filter(|c| is_town_command(c)) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        info!("console> {cmd}: {}", super::commands::cmd_town(&parts, &mut town));
    }
    levels::spawn_new_game(&mut commands, &mut map, &mut rng, &font, &mut town, args.depth);
    next_state.set_if_different(RunState::PreRun);
}

/// True from the third frame after every model has loaded: by then their meshes
/// are on the GPU too, so the level's own meshes are uploaded the same way each run.
fn models_loaded(
    models: Res<AvailableModels>,
    asset_server: Res<AssetServer>,
    mut frames_loaded: Local<u32>,
) -> bool {
    if models.loaded(&asset_server) {
        *frames_loaded += 1;
    }
    *frames_loaded > 2
}

/// `--reveal-map` holds on every level the party reaches, not just the first.
fn reveal_each_level(
    args: Res<ScreenshotArgs>,
    map: Res<Map>,
    mut seen: Local<Option<i32>>,
    mut debug_state: ResMut<DebugState>,
) {
    if args.reveal_map && seen.replace(map.depth) != Some(map.depth) {
        debug_state.reveal_map = true;
    }
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
    active: Res<ActiveUnit>,
    mut party: Query<(Entity, &mut Position, &mut CombatStats), With<PartyMember>>,
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
            &mut party,
            active.0,
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

/// How long a `--keys` pan key (`w` `a` `s` `d`) is held, in frames. Ten frames
/// pan about 2.3 tiles at the default zoom (ease in and out included), so a
/// pan lands between tile centres.
const PAN_HOLD_FRAMES: u32 = 10;

/// While the game waits for input and no walk is under way, feeds the next
/// queued input. Runs after picking and before the player's input handling,
/// so a click lands on its cell this frame.
fn feed_next_input(
    mut commands: Commands,
    args: Res<ScreenshotArgs>,
    mut next: Local<usize>,
    mut held: Local<Option<(KeyCode, u32)>>,
    mut settled: Local<bool>,
    walk: Res<WalkPath>,
    window: Single<Entity, With<PrimaryWindow>>,
    mut keyboard: MessageWriter<KeyboardInput>,
    mut wheel: MessageWriter<MouseWheel>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut hovered: ResMut<HoveredTile>,
    vendors: Query<(&Position, &Name), With<Bystander>>,
    debug_state: Res<DebugState>,
    presentation: Res<Presentation>,
) {
    let mut key = |key_code, state| {
        keyboard.write(KeyboardInput {
            key_code,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: *window,
        });
    };
    if let Some((key_code, frames_left)) = held.as_mut() {
        *frames_left -= 1;
        if *frames_left == 0 {
            key(*key_code, ButtonState::Released);
            *held = None;
        }
        return;
    }
    // A pending reveal would leave a click's target unexplored. Inputs wait
    // for the presentation too, so a script acts on what it has seen.
    if walk.is_active() || debug_state.reveal_map || presentation.busy() {
        return;
    }
    let Some(input) = args.inputs.get(*next) else {
        return;
    };
    // A press waits a frame, so the HUD's layout catches up with the last input.
    if matches!(input, HarnessInput::Press(_)) && !std::mem::replace(&mut *settled, true) {
        return;
    }
    *settled = false;
    *next += 1;
    match *input {
        HarnessInput::Key(key_code) => {
            key(key_code, ButtonState::Pressed);
            if matches!(key_code, KeyCode::KeyW | KeyCode::KeyA | KeyCode::KeyS | KeyCode::KeyD) {
                *held = Some((key_code, PAN_HOLD_FRAMES));
            }
        }
        HarnessInput::Scroll(y) => {
            wheel.write(MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y,
                window: *window,
                phase: TouchPhase::Moved,
            });
        }
        HarnessInput::Click(x, y) => {
            hovered.tile = Some((x, y));
            mouse.release(MouseButton::Left);
            mouse.press(MouseButton::Left);
        }
        HarnessInput::Press(point) => {
            *hovered = HoveredTile { cursor: Some(point), tile: None };
            mouse.release(MouseButton::Left);
            mouse.press(MouseButton::Left);
        }
        HarnessInput::Shop(vendor) => {
            let Some((pos, _)) = vendors.iter().find(|(_, name)| Vendor::from_name(&name.name) == Some(vendor)) else {
                warn!("--shop: no {} in this level", vendor.name());
                return;
            };
            hovered.tile = Some((pos.x, pos.y));
            mouse.release(MouseButton::Left);
            mouse.press(MouseButton::Left);
        }
        HarnessInput::Reload => commands.queue(reload_game),
        HarnessInput::Console(ref cmd) => commands.run_system_cached_with(run_console, cmd.clone()),
    }
}

/// `--do`: runs one console command now, as `prepare_capture` runs `--console`.
fn run_console(
    In(cmd): In<String>,
    mut commands: Commands,
    mut debug_state: ResMut<DebugState>,
    mut god_mode: ResMut<GodMode>,
    active: Res<ActiveUnit>,
    mut party: Query<(Entity, &mut Position, &mut CombatStats), With<PartyMember>>,
    monster_query: Query<Entity, With<Monster>>,
    mut town: ResMut<TownState>,
) {
    let out = super::commands::execute_command(
        &cmd,
        &mut commands,
        &mut debug_state,
        &mut god_mode,
        &mut party,
        active.0,
        &monster_query,
        &mut town,
    );
    info!("console> {cmd}: {out}");
}

/// `--reload`: save, load the file back, and log whether the loaded game is
/// the saved one.
fn reload_game(world: &mut World) {
    use crate::saveload::{first_difference, load_game, save_game, snapshot};
    let saved = snapshot(world);
    save_game(world);
    let line = if load_game(world) {
        let loaded = snapshot(world);
        for (a, b) in saved.iter().zip(&loaded) {
            if let Some(diff) = first_difference(std::slice::from_ref(a), std::slice::from_ref(b)) {
                info!("reload differs: {diff}");
            }
        }
        let mode = loaded.iter().find(|l| l.starts_with("mode ")).cloned().unwrap_or_default();
        match first_difference(&saved, &loaded) {
            None => format!("Reload: {mode}; the loaded game matches the saved one ({} lines).", loaded.len()),
            Some(diff) => format!("Reload: {mode}; the loaded game differs: {diff}"),
        }
    } else {
        "Reload: the save did not load.".to_string()
    };
    world.resource_mut::<GameLog>().entries.push(line);
    world.resource_mut::<NextState<RunState>>().set(RunState::PreRun);
}

/// How many frames the GPU may lag behind the CPU. A window's swap chain holds
/// a game to about this; the harness draws into an image, which holds it to
/// nothing, so without this a run queued frames until Metal's 4096 command
/// buffer cap, and two runs at once starved each other of the GPU.
const FRAMES_IN_FLIGHT: usize = 2;

/// Nanoseconds the render thread has spent waiting for the GPU (`pace_gpu`),
/// shared by the render world and the main world.
#[derive(Resource, Clone, Default)]
struct GpuWait(std::sync::Arc<std::sync::atomic::AtomicU64>);

/// Waits until the GPU has finished the frame `FRAMES_IN_FLIGHT` back.
fn pace_gpu(
    device: Res<bevy::render::renderer::RenderDevice>,
    queue: Res<bevy::render::renderer::RenderQueue>,
    wait: Res<GpuWait>,
    mut submitted: Local<std::collections::VecDeque<PollType>>,
) {
    // An empty submit names everything submitted so far, this frame included.
    submitted.push_back(PollType::Wait { submission_index: Some(queue.submit([])), timeout: None });
    if submitted.len() <= FRAMES_IN_FLIGHT {
        return;
    }
    let start = std::time::Instant::now();
    if let Some(oldest) = submitted.pop_front() {
        let _ = device.poll(oldest);
    }
    wait.0.fetch_add(start.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
}

/// Frames skipped before `FRAME_TIME_LOG=1` starts timing: level spawn, shader
/// compiles and first uploads.
const TIMING_WARMUP: u32 = 120;

/// Wall-clock frame times past the warm-up, and the GPU wait at its start.
#[derive(Default)]
struct FrameTimes {
    last: Option<std::time::Instant>,
    millis: Vec<f64>,
    gpu_wait_start: u64,
}

/// "frame time: median 9.1 ms, p95 10.4 ms, mean 9.3 ms (110 fps) over 480 frames; GPU wait 6.2 ms a frame".
fn frame_time_line(times: &FrameTimes, gpu_wait_nanos: u64) -> String {
    let mut sorted = times.millis.clone();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len().max(1);
    let median = sorted.get(n / 2).copied().unwrap_or_default();
    let p95 = sorted.get((n * 95).div_ceil(100).saturating_sub(1)).copied().unwrap_or_default();
    let mean = sorted.iter().sum::<f64>() / n as f64;
    let wait = (gpu_wait_nanos - times.gpu_wait_start) as f64 / 1e6 / n as f64;
    format!(
        "frame time: median {median:.2} ms, p95 {p95:.2} ms, mean {mean:.2} ms ({:.0} fps) over {} frames; GPU wait {wait:.2} ms a frame",
        1000.0 / median.max(f64::EPSILON),
        sorted.len()
    )
}

/// Counts frames after the game starts, then captures the picture once.
/// Saves the PNG and logs the game log, so a run can be checked by its state too.
/// With `FRAME_TIME_LOG=1` it also logs the frame times past the warm-up.
fn capture_after_frames(
    mut commands: Commands,
    args: Res<ScreenshotArgs>,
    target: Res<CaptureTarget>,
    log: Res<GameLog>,
    gpu_wait: Res<GpuWait>,
    mut frame: Local<u32>,
    mut times: Local<FrameTimes>,
    meshes: Query<(), With<Mesh3d>>,
    lights: Query<&PointLight>,
) {
    *frame += 1;
    if std::env::var_os("FRAME_TIME_LOG").is_some() && *frame > TIMING_WARMUP {
        let now = std::time::Instant::now();
        match times.last {
            Some(last) => times.millis.push((now - last).as_secs_f64() * 1000.0),
            None => times.gpu_wait_start = gpu_wait.0.load(std::sync::atomic::Ordering::Relaxed),
        }
        times.last = Some(now);
        if *frame == args.frames {
            info!("{}", frame_time_line(&times, gpu_wait.0.load(std::sync::atomic::Ordering::Relaxed)));
            let shadowed = lights.iter().filter(|l| l.shadow_maps_enabled).count();
            info!("scene: {} meshes, {} point lights ({shadowed} with shadows)", meshes.iter().count(), lights.iter().count());
        }
    }
    if args.every.is_some_and(|n| frame.is_multiple_of(n) && *frame < args.frames) {
        let stem = args.path.file_stem().unwrap_or_default().to_string_lossy();
        let path = args.path.with_file_name(format!("{stem}-{:04}.png", *frame));
        commands.spawn(Screenshot::image(target.0.clone())).observe(save_to_disk(path));
    }
    if *frame != args.frames {
        return;
    }
    for line in &log.entries {
        info!("log> {line}");
    }
    let mut save = save_to_disk(args.path.clone());
    commands.spawn(Screenshot::image(target.0.clone())).observe(move |captured: On<ScreenshotCaptured>| {
        save(captured);
        // Leave without tearing the app down: dropping the render world waits on the
        // Metal queue (`waitUntilCompleted`), which can block for minutes or for good
        // when other games share the GPU. The PNG and the log are written by now.
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        std::process::exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Result<Option<ScreenshotArgs>, String> {
        parse_args(line.split_whitespace().map(String::from))
    }

    #[test]
    fn frame_time_line_takes_median_and_95th_percentile() {
        let times = FrameTimes {
            last: None,
            millis: (1..=20).rev().map(f64::from).collect(),
            gpu_wait_start: 1_000_000,
        };
        assert_eq!(
            frame_time_line(&times, 41_000_000),
            "frame time: median 11.00 ms, p95 19.00 ms, mean 10.50 ms (91 fps) over 20 frames; GPU wait 2.00 ms a frame"
        );
    }

    #[test]
    fn no_flags_means_normal_game() {
        assert_eq!(parse(""), Ok(None));
    }

    #[test]
    fn all_flags() {
        assert_eq!(
            parse("--screenshot a.png --seed 7 --depth 3 --reveal-map --menu --visualise 3 --frames 10 --every 2 --console heal --console godmode --ledger --cursor 640,400.5 --target Dagger --keys g.in --click 3,-4 --press 800,950.5 --keys wasdc+-e --shop Priest --keys bj! --reload --load"),
            Ok(Some(ScreenshotArgs {
                path: PathBuf::from("a.png"),
                seed: 7,
                depth: 3,
                reveal_map: true,
                menu: true,
                visualise: Some(3),
                frames: 10,
                every: Some(2),
                console: vec!["heal".into(), "godmode".into()],
                ledger: true,
                cursor: Some(Vec2::new(640.0, 400.5)),
                target: Some("Dagger".into()),
                inputs: vec![
                    HarnessInput::Key(KeyCode::KeyG),
                    HarnessInput::Key(KeyCode::Period),
                    HarnessInput::Key(KeyCode::KeyI),
                    HarnessInput::Key(KeyCode::Tab),
                    HarnessInput::Click(3, -4),
                    HarnessInput::Press(Vec2::new(800.0, 950.5)),
                    HarnessInput::Key(KeyCode::KeyW),
                    HarnessInput::Key(KeyCode::KeyA),
                    HarnessInput::Key(KeyCode::KeyS),
                    HarnessInput::Key(KeyCode::KeyD),
                    HarnessInput::Key(KeyCode::KeyC),
                    HarnessInput::Scroll(1.0),
                    HarnessInput::Scroll(-1.0),
                    HarnessInput::Key(KeyCode::KeyE),
                    HarnessInput::Shop(Vendor::Priest),
                    HarnessInput::Key(KeyCode::KeyB),
                    HarnessInput::Key(KeyCode::KeyJ),
                    HarnessInput::Key(KeyCode::Escape),
                    HarnessInput::Reload,
                ],
                load: true,
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
                depth: 0,
                reveal_map: false,
                menu: false,
                visualise: None,
                frames: 60,
                every: None,
                console: vec![],
                ledger: false,
                cursor: None,
                target: None,
                inputs: vec![],
                load: false,
            }))
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("--screenshot a.png --depth -1").is_err());
        assert!(parse("--screenshot a.png --frames 0").is_err());
        assert!(parse("--screenshot a.png --every 0").is_err());
        assert!(parse("--screenshot a.png --seed x").is_err());
        assert!(parse("--screenshot").is_err());
        assert!(parse("--bogus 1").is_err());
        assert!(parse("--screenshot a.png --cursor 640").is_err());
        assert!(parse("--screenshot a.png --cursor 6x,4").is_err());
        assert!(parse("--screenshot a.png --keys gk").is_err());
        assert!(parse("--screenshot a.png --click 3").is_err());
        assert!(parse("--screenshot a.png --click 3,4.5").is_err());
        assert!(parse("--screenshot a.png --press 800").is_err());
        assert!(parse("--screenshot a.png --shop baker").is_err());
    }
}
