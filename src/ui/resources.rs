use bevy::prelude::*;

/// Tracks current page for paginated menus
#[derive(Resource, Default)]
pub struct MenuPage(pub usize);

/// Number of items shown per page in menus
pub const ITEMS_PER_PAGE: usize = 10;
