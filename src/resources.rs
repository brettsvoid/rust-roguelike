use bevy::prelude::*;

#[derive(Resource)]
pub struct UiFont(pub Handle<Font>);
impl FromWorld for UiFont {
    fn from_world(world: &mut World) -> Self {
        let server = world.resource::<AssetServer>();
        let handle: Handle<Font> = server.load("fonts/FiraCodeNerdFontMono-Bold.ttf");

        UiFont(handle)
    }
}

pub struct ResourcesPlugin;
impl Plugin for ResourcesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFont>();
    }
}
