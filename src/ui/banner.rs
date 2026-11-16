//! A brief centred banner when the mode or phase changes ("Combat", "Enemy
//! phase", "Round 2", "Victory"), eased in and out on `Time`. Which change
//! shows which banner is `combat_hud::banner`.

use bevy::prelude::*;

use crate::game_flow::Mode;
use crate::resources::UiFont;
use crate::RunState;

use super::combat_hud::{banner, Phase};
use super::style::{self, font};

pub struct BannerPlugin;

impl Plugin for BannerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Banners>()
            .add_systems(Startup, setup_banner)
            .add_systems(Update, (queue_banners, show_banner).chain());
    }
}

/// Seconds to ease in, hold and ease out.
const EASE_IN: f32 = 0.25;
const HOLD: f32 = 0.9;
const EASE_OUT: f32 = 0.35;
/// How far the band slides up while easing in, in logical pixels.
const RISE: f32 = 14.0;

/// The banner to show next, and the one showing with its age in seconds.
/// Only the latest change waits: an enemy phase over in a frame gives way to
/// the round after it rather than queueing stale news.
#[derive(Resource, Default)]
struct Banners {
    next: Option<String>,
    showing: Option<(String, f32)>,
}

#[derive(Component)]
struct BannerBand;
#[derive(Component)]
struct BannerText;

fn setup_banner(mut commands: Commands, font_handle: Res<UiFont>) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(24.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(48.0), Val::Px(10.0)),
                    border: UiRect::vertical(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                Visibility::Hidden,
                BannerBand,
            ))
            .with_child((Text::new(""), font(&font_handle.0, style::FONT_BANNER), TextColor(Color::NONE), BannerText));
        });
}

/// The phase the turn loop is in. The enemy phase is the `EnemyTurn` frames.
fn phase(mode: &Mode, state: &RunState) -> Phase {
    match mode {
        Mode::Explore => Phase::Explore,
        Mode::Combat { round, .. } if *state == RunState::EnemyTurn => Phase::Enemy(*round),
        Mode::Combat { round, .. } => Phase::Party(*round),
    }
}

fn queue_banners(mode: Res<Mode>, state: Res<State<RunState>>, mut last: Local<Option<Phase>>, mut banners: ResMut<Banners>) {
    let now = phase(&mode, state.get());
    if let Some(text) = last.replace(now).and_then(|before| banner(before, now)) {
        banners.next = Some(text);
    }
}

/// 0 to 1 and back over the banner's life, smoothed at both ends.
fn opacity(age: f32) -> f32 {
    let t = if age < EASE_IN {
        age / EASE_IN
    } else if age < EASE_IN + HOLD {
        1.0
    } else {
        1.0 - (age - EASE_IN - HOLD) / EASE_OUT
    };
    style::ease(t)
}

fn show_banner(
    time: Res<Time>,
    mut banners: ResMut<Banners>,
    mut band: Query<(&mut Node, &mut BackgroundColor, &mut BorderColor, &mut Visibility), With<BannerBand>>,
    mut text: Query<(&mut Text, &mut TextColor), With<BannerText>>,
) {
    let (Ok((mut node, mut background, mut border, mut visibility)), Ok((mut text, mut colour))) =
        (band.single_mut(), text.single_mut())
    else {
        return;
    };
    let banners = &mut *banners;
    if let Some((_, age)) = banners.showing.as_mut() {
        *age += time.delta_secs();
        if *age > EASE_IN + HOLD + EASE_OUT {
            banners.showing = None;
        }
    }
    if banners.showing.is_none() {
        banners.showing = banners.next.take().map(|t| (t, 0.0));
    }
    let Some((label, age)) = &banners.showing else {
        *visibility = Visibility::Hidden;
        return;
    };
    let alpha = opacity(*age);
    *visibility = Visibility::Inherited;
    if **text != *label {
        **text = label.clone();
    }
    colour.0 = style::ACCENT.with_alpha(alpha);
    background.0 = style::PANEL.with_alpha(0.85 * alpha);
    *border = BorderColor::all(style::ACCENT.with_alpha(0.5 * alpha));
    node.margin.top = Val::Px(RISE * (1.0 - opacity(age.min(EASE_IN))));
}
