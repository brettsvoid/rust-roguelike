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

#[derive(Resource)]
pub struct MenuBackground(pub Handle<Image>);
impl FromWorld for MenuBackground {
    fn from_world(world: &mut World) -> Self {
        let server = world.resource::<AssetServer>();
        let handle: Handle<Image> = server.load("images/menu_background.png");
        MenuBackground(handle)
    }
}

pub struct ResourcesPlugin;
impl Plugin for ResourcesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFont>()
            .init_resource::<MenuBackground>();
    }
}
