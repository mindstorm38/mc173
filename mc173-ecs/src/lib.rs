use bevy::prelude::*;

pub mod geom;
pub mod rand;

pub mod item;
pub mod block;
pub mod biome;
pub mod chunk;

pub mod world;
pub mod entity;


pub struct MinecraftPlugin {
    
}

impl Plugin for MinecraftPlugin {

    fn build(&self, app: &mut App) {
        
        app.run()
        
    }

}
