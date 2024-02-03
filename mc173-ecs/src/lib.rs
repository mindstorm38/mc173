use bevy::prelude::*;

pub mod geom;
pub mod entity;


pub struct MinecraftPlugin {
    
}

impl Plugin for MinecraftPlugin {

    fn build(&self, app: &mut App) {
        
        app.run()
        
    }

}
