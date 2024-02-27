use glam::IVec3;

use crate::entity::Entity;



pub trait Chunks {

    fn set_block(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)>;
    fn get_block(&self, pos: IVec3) -> Option<(u8, u8)>;

}

pub trait Entities {

    fn spawn_entity(&mut self, entity: impl Into<Box<Entity>>) -> u32;
    fn contains_entity(&self, id: u32) -> bool;
    fn get_entity_count(&self) -> usize;
    fn get_entity(&self, id: u32) -> Option<&Entity>;
    fn get_entity_mut(&mut self, id: u32) -> Option<&mut Entity>;
    fn remove_entity(&mut self, id: u32, reason: &str) -> bool;
    
}

pub trait Notify {

    fn set_block_self_notify(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)>;
    fn set_block_notify(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)>;

    fn notify_blocks_around(&mut self, pos: IVec3, origin_id: u8);
    fn notify_block(&mut self, pos: IVec3, origin_id: u8);

    fn notify_block_unchecked(&mut self, pos: IVec3, id: u8, metadata: u8, origin_id: u8);
    fn notify_change_unchecked(&mut self, pos: IVec3, from_id: u8, from_metadata: u8, to_id: u8, to_metadata: u8);

}




pub struct StdNotifyData {
    /* Some data for standard notify... */
}

pub trait StdNotify: Chunks + Entities {
    fn data(&self) -> &StdNotifyData;
    fn data_mut(&mut self) -> &mut StdNotifyData;
}

impl<Std: StdNotify> Notify for Std {

    fn set_block_self_notify(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)> {
        todo!()
    }

    fn set_block_notify(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)> {
        todo!()
    }

    fn notify_blocks_around(&mut self, pos: IVec3, origin_id: u8) {
        todo!()
    }

    fn notify_block(&mut self, pos: IVec3, origin_id: u8) {
        todo!()
    }

    fn notify_block_unchecked(&mut self, pos: IVec3, id: u8, metadata: u8, origin_id: u8) {
        todo!()
    }

    fn notify_change_unchecked(&mut self, pos: IVec3, from_id: u8, from_metadata: u8, to_id: u8, to_metadata: u8) {
        todo!()
    }

}
