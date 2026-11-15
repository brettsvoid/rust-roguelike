use bevy::prelude::*;
use rand::RngExt;

use crate::{
    combat::SufferDamage,
    components::{EntryTrigger, Hidden, InflictsDamage, Name, SingleActivation},
    gamelog::GameLog,
    map::{Map, Position},
    effects::{EffectKind, Effects},
    party::PartyMember,
    rng::GameRng,
    viewshed::Viewshed,
};

pub fn trap_trigger_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    mut effects: ResMut<Effects>,
    map: Res<Map>,
    moved_query: Query<(Entity, &Position, &Name), Changed<Position>>,
    just_spawned: Query<Entity, Added<Position>>,
    trap_query: Query<
        (Entity, &Position, &Name, &InflictsDamage, Option<&SingleActivation>),
        With<EntryTrigger>,
    >,
) {
    for (victim_entity, victim_pos, victim_name) in &moved_query {
        // Skip entities that were just spawned (not actually moved)
        if just_spawned.contains(victim_entity) {
            continue;
        }

        let idx = map.xy_idx(victim_pos.x, victim_pos.y);

        for trap_entity in map.tile_content[idx].iter() {
            if let Ok((trap_ent, trap_pos, trap_name, damage, single)) =
                trap_query.get(*trap_entity)
            {
                // Apply damage
                SufferDamage::new_damage(&mut commands, victim_entity, damage.damage);
                gamelog.entries.push(format!(
                    "{} triggers {}, taking {} damage!",
                    victim_name.name, trap_name.name, damage.damage
                ));

                effects.play(EffectKind::HitSpark, trap_pos.x, trap_pos.y);

                // Remove Hidden component (now visible)
                commands.entity(trap_ent).remove::<Hidden>();

                // Despawn if single activation
                if single.is_some() {
                    commands.entity(trap_ent).despawn();
                }
            }
        }
    }
}

pub fn reveal_hidden_system(
    mut commands: Commands,
    mut gamelog: ResMut<GameLog>,
    party: Query<&Viewshed, With<PartyMember>>,
    hidden_query: Query<(Entity, &Position, &Name), With<Hidden>>,
    mut rng: ResMut<GameRng>,
) {
    for (entity, pos, name) in &hidden_query {
        if party.iter().any(|v| v.visible_tiles.contains(&(pos.x, pos.y))) {
            // 1 in 24 chance to spot hidden entity
            if rng.0.random_range(1..=24) == 1 {
                commands.entity(entity).remove::<Hidden>();
                gamelog
                    .entries
                    .push(format!("You spotted a {}.", name.name));
            }
        }
    }
}
