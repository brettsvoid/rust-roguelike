//! The bottom bar: a card per hero on the left, the active hero's action
//! buttons in the middle, depth, gold, mode and the game log on the right.
//! Cards and buttons answer the mouse (`click_hud`); a button asks for its
//! key through `ActionRequest`, so it does exactly what the hotkey does.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use crate::abilities::{update_active_abilities, ActiveAbilities, HeroAbility, HeroClass};
use crate::combat::{in_reach, Arms, CombatStats};
use crate::components::{Confusion, HungerClock, HungerState, Name, Poison};
use crate::economy::Purse;
use crate::game_flow::Mode;
use crate::gamelog::GameLog;
use crate::map::{Map, Position};
use crate::monsters::Monster;
use crate::overwatch::{Hunkered, Overwatch};
use crate::party::{ActiveUnit, PartyMember, HEROES, PARTY_SIZE};
use crate::picking::{update_hovered_tile, BlocksPicking, HoveredTile};
use crate::player::{self, WalkPath};
use crate::resources::UiFont;
use crate::tactics::AP_PER_ROUND;
use crate::viewshed::Viewshed;
use crate::RunState;

use super::combat_hud::{action_bar, ActionContext, ActionRequest, HudAction};
use super::components::{DepthText, GameLogText, GoldText, ModeText};
use super::style::{self, font};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        // HUD updates only during actual gameplay
        let in_gameplay = not(in_state(RunState::MainMenu))
            .and_then(not(in_state(RunState::GameOver)))
            .and_then(not(in_state(RunState::MapGeneration)))
            .and_then(not(in_state(RunState::MapBuilderSelect)));

        app.init_resource::<ActionRequest>()
            .add_systems(Startup, setup_hud)
            .add_systems(
                Update,
                (
                    update_cards,
                    update_action_bar.after(update_hovered_tile).after(update_active_abilities),
                    click_hud
                        .run_if(in_state(RunState::AwaitingInput))
                        .after(update_action_bar)
                        .before(player::handle_player_input),
                    update_depth,
                    update_gold,
                    update_mode,
                    update_game_log,
                )
                    .run_if(in_gameplay),
            );
    }
}

/// What a click on a HUD element does.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudTarget {
    /// Select the hero in this party slot.
    Card(usize),
    Action(HudAction),
}

/// Whether the cursor (logical pixels) is over a UI node (physical pixels of
/// the render target, which is an image at scale 1 in the harness).
fn under_cursor(cursor: Vec2, camera: &Camera, node: &ComputedNode, transform: &UiGlobalTransform) -> bool {
    node.contains_point(*transform, cursor * camera.target_scaling_factor().unwrap_or(1.0))
}

#[derive(Component)]
struct CardStripe(usize);
#[derive(Component)]
struct CardDisc(usize);
#[derive(Component)]
struct CardName(usize);
#[derive(Component)]
struct CardClass(usize);
#[derive(Component)]
struct CardHealth(usize);
#[derive(Component)]
struct CardHealthText(usize);
#[derive(Component)]
struct CardPip(usize, u8);
#[derive(Component)]
struct CardBadge(usize, Badge);

/// A status chip on a hero's card.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Badge {
    Overwatch,
    Hunkered,
    Poisoned,
    Confused,
    Hunger,
}

impl Badge {
    const ALL: [Badge; 5] = [Badge::Overwatch, Badge::Hunkered, Badge::Poisoned, Badge::Confused, Badge::Hunger];
}

/// Whether a button can be pressed now, as `update_action_bar` last drew it.
#[derive(Component, Default)]
pub struct Enabled(bool);
#[derive(Component)]
struct ButtonLabel(HudAction);
#[derive(Component)]
struct ButtonCaption(HudAction);
#[derive(Component)]
struct ActionHint;
#[derive(Component)]
struct NewestLogLine;

fn panel_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        bottom: Val::Px(0.0),
        left: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Px(style::BAR_HEIGHT),
        padding: UiRect::all(Val::Px(style::BAR_PADDING)),
        column_gap: Val::Px(style::GAP),
        border: UiRect::top(Val::Px(1.0)),
        ..default()
    }
}

fn setup_hud(mut commands: Commands, font_handle: Res<UiFont>) {
    let f = &font_handle.0;
    commands
        .spawn((panel_node(), BackgroundColor(style::PANEL), BorderColor::all(style::PANEL_EDGE), BlocksPicking))
        .with_children(|bar| {
            // Party cards, in slot order
            bar.spawn(Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                width: Val::Px(style::CARD_WIDTH),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|cards| {
                for slot in 0..PARTY_SIZE {
                    spawn_card(cards, f, slot);
                }
            });

            // The action bar, centred in the space between
            bar.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(6.0),
                // The buttons never squeeze; the log gives way to them.
                flex_grow: 1.0,
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|middle| {
                middle.spawn((Text::new(""), font(f, style::FONT_BODY), TextColor(style::TEXT_DIM), ActionHint));
                middle
                    .spawn(Node { column_gap: Val::Px(style::BUTTON_GAP), ..default() })
                    .with_children(|buttons| {
                        for action in HudAction::ALL {
                            spawn_button(buttons, f, action);
                        }
                    });
            });

            // Depth, gold and mode over the game log
            bar.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_basis: Val::Percent(28.0),
                min_width: Val::Px(style::LOG_MIN_WIDTH),
                max_width: Val::Px(style::LOG_MAX_WIDTH),
                flex_shrink: 1.0,
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|right| {
                right
                    .spawn(Node { column_gap: Val::Px(12.0), flex_wrap: FlexWrap::Wrap, ..default() })
                    .with_children(|info| {
                        info.spawn((Text::new(""), font(f, style::FONT_BODY), TextColor(style::TEXT), DepthText));
                        info.spawn((Text::new(""), font(f, style::FONT_BODY), TextColor(style::ACCENT), GoldText));
                        info.spawn((Text::new(""), font(f, style::FONT_BODY), TextColor(style::HEALTH), ModeText));
                    });
                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        // Clip the oldest lines, not the newest, when the log wraps.
                        justify_content: JustifyContent::FlexEnd,
                        overflow: Overflow::clip(),
                        border: UiRect::top(Val::Px(1.0)),
                        padding: UiRect::top(Val::Px(3.0)),
                        ..default()
                    })
                    .insert(BorderColor::all(style::PANEL_EDGE))
                    .with_children(|log| {
                        log.spawn((Text::new(""), font(f, style::FONT_BODY), TextColor(style::TEXT_DIM), GameLogText));
                        log.spawn((Text::new(""), font(f, style::FONT_BODY), TextColor(style::TEXT), NewestLogLine));
                    });
            });
        });
}

fn spawn_card(cards: &mut ChildSpawnerCommands, f: &Handle<Font>, slot: usize) {
    cards
        .spawn((
            Node {
                height: Val::Px(style::CARD_HEIGHT),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::right(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(style::RADIUS)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(style::CARD),
            HudTarget::Card(slot),
        ))
        .with_children(|card| {
            card.spawn((Node { width: Val::Px(3.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::NONE), CardStripe(slot)));
            // Portrait: the hero's colour until there is art.
            card.spawn((
                Node {
                    width: Val::Px(12.0),
                    height: Val::Px(12.0),
                    flex_shrink: 0.0,
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(HEROES[slot].1.into()),
                CardDisc(slot),
            ));
            card.spawn((
                Node { width: Val::Px(54.0), flex_shrink: 0.0, ..default() },
                Text::new(HEROES[slot].0),
                font(f, style::FONT_NAME),
                TextColor(style::TEXT),
                CardName(slot),
            ));
            card.spawn((
                Node { width: Val::Px(40.0), flex_shrink: 0.0, ..default() },
                Text::new(""),
                font(f, style::FONT_SMALL),
                TextColor(style::TEXT_DIM),
                CardClass(slot),
            ));
            card.spawn((
                Node {
                    width: Val::Px(52.0),
                    height: Val::Px(6.0),
                    flex_shrink: 0.0,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(style::TRACK),
            ))
            .with_child((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(style::HEALTH), CardHealth(slot)));
            card.spawn((
                Node { width: Val::Px(36.0), flex_shrink: 0.0, ..default() },
                Text::new(""),
                font(f, style::FONT_SMALL),
                TextColor(style::TEXT_DIM),
                CardHealthText(slot),
            ));
            card.spawn(Node { column_gap: Val::Px(3.0), flex_shrink: 0.0, ..default() }).with_children(|pips| {
                for pip in 0..AP_PER_ROUND {
                    pips.spawn((
                        Node {
                            width: Val::Px(7.0),
                            height: Val::Px(7.0),
                            border_radius: BorderRadius::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(style::AP),
                        CardPip(slot, pip),
                    ));
                }
            });
            card.spawn(Node { column_gap: Val::Px(3.0), ..default() }).with_children(|badges| {
                for badge in Badge::ALL {
                    badges.spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(3.0), Val::Px(1.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            display: Display::None,
                            ..default()
                        },
                        Text::new(""),
                        font(f, style::FONT_SMALL),
                        TextColor(style::TEXT),
                        BorderColor::all(style::TEXT),
                        CardBadge(slot, badge),
                    ));
                }
            });
        });
}

fn spawn_button(buttons: &mut ChildSpawnerCommands, f: &Handle<Font>, action: HudAction) {
    buttons
        .spawn((
            Node {
                width: Val::Px(style::BUTTON_WIDTH),
                height: Val::Px(style::BUTTON_HEIGHT),
                flex_shrink: 1.0,
                min_width: Val::Px(52.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(3.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(style::RADIUS)),
                ..default()
            },
            BackgroundColor(style::BUTTON),
            BorderColor::all(style::BUTTON_EDGE),
            HudTarget::Action(action),
            Enabled::default(),
        ))
        .with_children(|button| {
            button.spawn((Text::new(action.label()), font(f, style::FONT_BODY), TextColor(style::TEXT), ButtonLabel(action)));
            button.spawn((Text::new(action.hotkey()), font(f, style::FONT_SMALL), TextColor(style::TEXT_DIM), ButtonCaption(action)));
        });
}

type CardHero<'a> = (
    Entity,
    &'a PartyMember,
    &'a CombatStats,
    &'a HungerClock,
    Has<Overwatch>,
    Has<Hunkered>,
    Has<Poison>,
    Has<Confusion>,
    Option<&'a HeroClass>,
);

/// Each card shows its hero's health, AP pips (in combat), hunger and
/// statuses. The active hero's card is lit and striped; a fallen hero's is
/// greyed out.
#[allow(clippy::type_complexity)]
fn update_cards(
    active: Res<ActiveUnit>,
    mode: Res<Mode>,
    hovered: Res<HoveredTile>,
    camera: Single<&Camera, With<Camera3d>>,
    party: Query<CardHero>,
    mut cards: Query<(&HudTarget, &ComputedNode, &UiGlobalTransform, &mut BackgroundColor), Without<CardHealth>>,
    mut stripes: Query<(&CardStripe, &mut BackgroundColor), (Without<HudTarget>, Without<CardHealth>, Without<CardPip>)>,
    mut discs: Query<(&CardDisc, &mut BackgroundColor), (Without<HudTarget>, Without<CardStripe>, Without<CardHealth>, Without<CardPip>)>,
    mut names: Query<(&CardName, &mut TextColor), (Without<CardHealthText>, Without<CardBadge>)>,
    mut health: Query<(&CardHealth, &mut Node, &mut BackgroundColor), (Without<HudTarget>, Without<CardPip>)>,
    mut health_text: Query<(&CardHealthText, &mut Text), Without<CardBadge>>,
    mut classes: Query<(&CardClass, &mut Text), (Without<CardHealthText>, Without<CardBadge>)>,
    mut pips: Query<(&CardPip, &mut BackgroundColor, &mut Visibility), (Without<HudTarget>, Without<CardHealth>)>,
    mut badges: Query<(&CardBadge, &mut Node, &mut Text, &mut TextColor, &mut BorderColor), Without<CardHealth>>,
) {
    let hero = |slot: usize| party.iter().find(|h| h.1.slot == slot);
    let combat = matches!(*mode, Mode::Combat { .. });
    for (target, node, transform, mut background) in &mut cards {
        let HudTarget::Card(slot) = *target else { continue };
        let is_active = hero(slot).is_some_and(|h| active.0 == Some(h.0));
        let hover = hovered.cursor.is_some_and(|c| under_cursor(c, &camera, node, transform));
        background.0 = match (is_active, hover && hero(slot).is_some()) {
            (true, _) => style::CARD_ACTIVE,
            (false, true) => style::CARD_HOVER,
            _ => style::CARD,
        };
    }
    for (CardStripe(slot), mut colour) in &mut stripes {
        colour.0 = if hero(*slot).is_some_and(|h| active.0 == Some(h.0)) { style::ACCENT } else { Color::NONE };
    }
    for (CardDisc(slot), mut colour) in &mut discs {
        colour.0 = if hero(*slot).is_some() { HEROES[*slot].1.into() } else { style::TEXT_FAINT };
    }
    for (CardName(slot), mut colour) in &mut names {
        colour.0 = if hero(*slot).is_some() { style::TEXT } else { style::TEXT_FAINT };
    }
    for (CardHealth(slot), mut node, mut colour) in &mut health {
        let fraction = hero(*slot).map_or(0.0, |h| h.2.hp.max(0) as f32 / h.2.max_hp.max(1) as f32);
        node.width = Val::Percent(100.0 * fraction);
        colour.0 = if fraction <= 0.34 { style::HEALTH_LOW } else { style::HEALTH };
    }
    for (CardClass(slot), mut text) in &mut classes {
        let class = hero(*slot).and_then(|h| h.8).map_or("", |c| c.0.as_str());
        if **text != class {
            **text = class.to_string();
        }
    }
    for (CardHealthText(slot), mut text) in &mut health_text {
        let label = hero(*slot).map_or_else(|| "Fallen".to_string(), |h| format!("{}/{}", h.2.hp, h.2.max_hp));
        if **text != label {
            **text = label;
        }
    }
    for (CardPip(slot, pip), mut colour, mut visibility) in &mut pips {
        let left = hero(*slot).map(|h| mode.ap_left(h.0));
        *visibility = if combat && left.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        colour.0 = if left.is_some_and(|ap| *pip < ap) { style::AP } else { style::AP_SPENT };
    }
    for (CardBadge(slot, badge), mut node, mut text, mut colour, mut border) in &mut badges {
        let shown = hero(*slot).and_then(|(_, _, _, hunger, watching, hunkered, poisoned, confused, _)| match badge {
            Badge::Overwatch => watching.then_some(("Watch", style::BADGE_OVERWATCH)),
            Badge::Hunkered => hunkered.then_some(("Hunker", style::BADGE_HUNKERED)),
            Badge::Poisoned => poisoned.then_some(("Poison", style::BADGE_POISON)),
            Badge::Confused => confused.then_some(("Dazed", style::BADGE_CONFUSED)),
            Badge::Hunger => match hunger.state {
                HungerState::WellFed => Some(("Fed", style::BADGE_FED)),
                HungerState::Normal => None,
                HungerState::Hungry => Some(("Hungry", style::BADGE_HUNGRY)),
                HungerState::Starving => Some(("Starving", style::HEALTH_LOW)),
            },
        });
        node.display = if shown.is_some() { Display::Flex } else { Display::None };
        if let Some((label, tint)) = shown {
            if **text != label {
                **text = label.to_string();
            }
            colour.0 = tint;
            *border = BorderColor::all(tint.with_alpha(0.45));
        }
    }
}

/// The hostiles some hero sees, where they stand.
fn seen_hostiles(
    monsters: &Query<(Entity, &Position), With<Monster>>,
    party: &Query<&Viewshed, With<PartyMember>>,
) -> Vec<(Entity, (i32, i32))> {
    monsters
        .iter()
        .filter(|(_, pos)| party.iter().any(|view| view.visible_tiles.contains(&(pos.x, pos.y))))
        .map(|(entity, pos)| (entity, (pos.x, pos.y)))
        .collect()
}

/// Shows the buttons `action_bar` gives the active hero: hidden when not
/// offered, dimmed with the reason in place of the hotkey when refused, lit
/// under the cursor.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_action_bar(
    mode: Res<Mode>,
    active: Res<ActiveUnit>,
    map: Res<Map>,
    arms: Arms,
    hovered: Res<HoveredTile>,
    camera: Single<&Camera, With<Camera3d>>,
    (heroes, views, monsters): (
        Query<(&Position, &Name), With<PartyMember>>,
        Query<&Viewshed, With<PartyMember>>,
        Query<(Entity, &Position), With<Monster>>,
    ),
    mut buttons: Query<(
        &HudTarget,
        &ComputedNode,
        &UiGlobalTransform,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut Enabled,
    )>,
    mut labels: Query<(&ButtonLabel, &mut Text, &mut TextColor), (Without<ButtonCaption>, Without<ActionHint>)>,
    mut captions: Query<(&ButtonCaption, &mut Text, &mut TextColor), (Without<ButtonLabel>, Without<ActionHint>)>,
    mut hint: Query<&mut Text, (With<ActionHint>, Without<ButtonCaption>, Without<ButtonLabel>)>,
    (abilities, ability_names): (Res<ActiveAbilities>, Query<&Name, With<HeroAbility>>),
) {
    let hero = active.0.and_then(|unit| heroes.get(unit).ok().map(|(pos, name)| (unit, pos, name)));
    let combat = matches!(*mode, Mode::Combat { .. });
    let bar = hero.map(|(unit, pos, _)| {
        let weapon = arms.weapon(unit);
        let target_in_reach = seen_hostiles(&monsters, &views)
            .iter()
            .any(|&(_, at)| in_reach(&map, &weapon, (pos.x, pos.y), at).is_ok());
        let abilities = abilities.0.map(|a| a.map(|a| a.status));
        action_bar(ActionContext { combat, ap_left: mode.ap_left(unit), target_in_reach, abilities })
    });
    let bar = bar.unwrap_or_default();
    let refusal = |action: HudAction| bar.iter().find(|b| b.action == action).map(|b| b.refusal);

    for (target, node, transform, mut layout, mut background, mut border, mut enabled) in &mut buttons {
        let HudTarget::Action(action) = *target else { continue };
        let offered = refusal(action);
        layout.display = if offered.is_some() { Display::Flex } else { Display::None };
        enabled.0 = offered == Some(None);
        let enabled = enabled.0;
        let hover = enabled && hovered.cursor.is_some_and(|c| under_cursor(c, &camera, node, transform));
        background.0 = match (enabled, hover) {
            (false, _) => style::BUTTON_DISABLED,
            (true, true) => style::BUTTON_HOVER,
            (true, false) => style::BUTTON,
        };
        *border = BorderColor::all(if hover { style::ACCENT } else { style::BUTTON_EDGE });
    }
    for (ButtonLabel(action), mut text, mut colour) in &mut labels {
        colour.0 = if refusal(*action) == Some(None) { style::TEXT } else { style::TEXT_FAINT };
        // An ability button names the ability.
        let HudAction::Ability(slot) = *action else { continue };
        let Some(ability) = abilities.0[slot] else { continue };
        let name = ability_names.get(ability.entity).map_or("", |n| n.name.as_str());
        if **text != name {
            **text = name.to_string();
        }
    }
    for (ButtonCaption(action), mut text, mut colour) in &mut captions {
        let (caption, tint) = match refusal(*action).flatten() {
            Some(reason) => (reason.to_string(), style::HEALTH_LOW.with_alpha(0.75)),
            None => (action.hotkey().to_string(), style::TEXT_DIM),
        };
        // A resting ability says how many rounds are left.
        let caption = match *action {
            HudAction::Ability(slot) if caption == "Cooldown" => {
                abilities.0[slot].map_or(caption, |a| format!("Cooldown {}", a.status.left))
            }
            _ => caption,
        };
        if **text != caption {
            **text = caption;
        }
        colour.0 = tint;
    }
    if let Ok(mut hint) = hint.single_mut() {
        let label = match (hero, combat) {
            (Some((unit, _, name)), true) => {
                format!("{}: {} AP. Click a tile to move, a foe to attack.", name.name, mode.ap_left(unit))
            }
            (Some((_, _, name)), false) => format!("{} leads. Click to walk.", name.name),
            (None, _) => String::new(),
        };
        if **hint != label {
            **hint = label;
        }
    }
}

/// A click on a card selects that hero (the living, and in combat only those
/// with AP left, as Tab); a click on an enabled button asks for its key.
#[allow(clippy::too_many_arguments)]
pub fn click_hud(
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredTile>,
    camera: Single<&Camera, With<Camera3d>>,
    targets: Query<(&HudTarget, &ComputedNode, &UiGlobalTransform, &InheritedVisibility, &Node, Option<&Enabled>)>,
    party: Query<(Entity, &PartyMember, &CombatStats)>,
    mode: Res<Mode>,
    mut active: ResMut<ActiveUnit>,
    mut walk: ResMut<WalkPath>,
    mut request: ResMut<ActionRequest>,
) {
    let Some(cursor) = hovered.cursor.filter(|_| mouse.just_pressed(MouseButton::Left)) else {
        return;
    };
    let clicked = targets.iter().find(|(_, node, transform, visible, layout, _)| {
        visible.get() && layout.display != Display::None && under_cursor(cursor, &camera, node, transform)
    });
    match clicked {
        Some((HudTarget::Card(slot), ..)) => {
            let pick = party.iter().find(|h| h.1.slot == *slot && h.2.hp > 0 && mode.can_act(h.0));
            if let Some((hero, ..)) = pick.filter(|h| active.0 != Some(h.0)) {
                *walk = WalkPath::default();
                active.0 = Some(hero);
            }
        }
        Some((HudTarget::Action(action), .., Some(Enabled(true)))) => {
            request.0 = Some(action.key());
        }
        _ => {}
    }
}

fn update_depth(map: Res<Map>, mut depth_text_query: Query<&mut Text, With<DepthText>>) {
    if let Ok(mut text) = depth_text_query.single_mut() {
        let label = match map.depth {
            crate::levels::TOWN_DEPTH => "Town".to_string(),
            depth => format!("Depth {depth}"),
        };
        if **text != label {
            **text = label;
        }
    }
}

/// "Explore", or the combat round and phase in red.
fn update_mode(
    mode: Res<Mode>,
    state: Res<State<RunState>>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<ModeText>>,
) {
    let Ok((mut text, mut colour)) = text_query.single_mut() else {
        return;
    };
    let (label, tint) = match &*mode {
        Mode::Explore => ("Explore".to_string(), style::HEALTH),
        Mode::Combat { round, .. } => {
            let phase = if *state.get() == RunState::EnemyTurn { "enemy" } else { "party" };
            (format!("Round {round}, {phase} phase"), style::HOSTILE)
        }
    };
    if **text != label {
        **text = label;
        colour.0 = tint;
    }
}

fn update_gold(purse: Res<Purse>, mut gold_text_query: Query<&mut Text, With<GoldText>>) {
    if let Ok(mut text) = gold_text_query.single_mut() {
        let label = format!("{} gold", purse.gold);
        if **text != label {
            **text = label;
        }
    }
}

/// The last few log lines, oldest first and dimmed; the newest stands out.
fn update_game_log(
    game_log: Res<GameLog>,
    mut older: Query<&mut Text, (With<GameLogText>, Without<NewestLogLine>)>,
    mut newest: Query<&mut Text, (With<NewestLogLine>, Without<GameLogText>)>,
) {
    if !game_log.is_changed() {
        return;
    }
    let (Ok(mut older), Ok(mut newest)) = (older.single_mut(), newest.single_mut()) else {
        return;
    };
    let entries = &game_log.entries;
    let last = entries.len().saturating_sub(1);
    **older = entries[last.saturating_sub(5)..last].join("\n");
    **newest = entries.last().cloned().unwrap_or_default();
}
