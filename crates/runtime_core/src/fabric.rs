use crossbeam_channel::{unbounded, Receiver, Sender};
use nexus_protocol::interface::FabricMessage;

pub struct MessageFabric {
    pub tx: Sender<FabricMessage>,
    pub rx: Receiver<FabricMessage>,
}

impl MessageFabric {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self { tx, rx }
    }
}
