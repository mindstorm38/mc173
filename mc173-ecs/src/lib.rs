use bevy::prelude::*;

pub mod geom;
pub mod rand;

pub mod item;
pub mod block;
pub mod entity;
pub mod chunk;


/// Base plugin defining the 
pub struct MinecraftPlugin {
    
}

impl Plugin for MinecraftPlugin {

    fn build(&self, app: &mut App) {
        app.add_plugins(entity::EntityPlugin);
    }

}
