//! The victory screen: the party reached town carrying the Heart of the Abyss.

use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::abilities::HeroClass;
use crate::components::Name;
use crate::economy::Purse;
use crate::experience::Experience;
use crate::levels::Delve;
use crate::party::PartyMember;
use crate::RunState;

use crate::ui::panel::{close_panels, show, spawn_panel, BodyLine, Hint, Panel, PanelSpec, Placement};

pub struct VictoryPlugin;

impl Plugin for VictoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(RunState::Victory), |mut commands: Commands| {
            spawn_panel(&mut commands, Placement::Modal);
        })
        .add_systems(OnExit(RunState::Victory), close_panels)
        .add_systems(Update, (handle_victory_input, write_victory).run_if(in_state(RunState::Victory)));
    }
}

/// One surviving hero: name, class and level.
type Survivor = (String, String, i32);

fn victory_spec(delve: &Delve, gold: i32, survivors: &[Survivor]) -> PanelSpec {
    let mut body = vec![
        BodyLine::Text("The Heart of the Abyss is home. The run is won.".into()),
        BodyLine::Text(format!(
            "Deepest level {}, {} levels entered, {gold} gold.",
            delve.deepest, delve.levels_entered
        )),
        BodyLine::Heading("Survivors".into()),
    ];
    body.extend(
        survivors
            .iter()
            .map(|(name, class, level)| BodyLine::Text(format!("{name}, {class}, level {level}"))),
    );
    body.push(BodyLine::Text("The town keeps its trade for the next party.".into()));
    PanelSpec {
        title: "The Heart is home".into(),
        width: 420.0,
        body,
        hints: vec![Hint::new("Any key", "Main menu", KeyCode::Escape)],
        ..default()
    }
}

fn write_victory(
    mut panels: Query<(&Panel, &mut PanelSpec)>,
    delve: Res<Delve>,
    purse: Res<Purse>,
    party: Query<(&PartyMember, &Name, Option<&HeroClass>, &Experience)>,
) {
    let mut heroes: Vec<_> = party.iter().collect();
    heroes.sort_by_key(|(member, ..)| member.slot);
    let survivors: Vec<Survivor> = heroes
        .into_iter()
        .map(|(_, name, class, xp)| (name.name.clone(), class.map_or("Adventurer".into(), |c| c.0.clone()), xp.level))
        .collect();
    show(&mut panels, victory_spec(&delve, purse.gold, &survivors));
}

fn handle_victory_input(mut evr_kbd: MessageReader<KeyboardInput>, mut next_state: ResMut<NextState<RunState>>) {
    for ev in evr_kbd.read() {
        if ev.state == ButtonState::Pressed {
            next_state.set_if_different(RunState::MainMenu);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_names_the_survivors() {
        let delve = Delve { deepest: 12, levels_visited: 0, levels_entered: 23 };
        let spec = victory_spec(&delve, 340, &[("Aldric".into(), "Fighter".into(), 6)]);
        assert_eq!(spec.body[1], BodyLine::Text("Deepest level 12, 23 levels entered, 340 gold.".into()));
        assert_eq!(spec.body[3], BodyLine::Text("Aldric, Fighter, level 6".into()));
    }
}
