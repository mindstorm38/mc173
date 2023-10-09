//! The network server managing connected players and dispatching incoming packets.

use crate::proto::PacketServer;
use crate::world::World;


pub struct Server {
    pub packet_server: PacketServer,
    pub overworld: World,
}

impl Server {

}
