use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Capabilities: u32 {
        const NONE     = 0;
        const READ     = 1 << 0;
        const WRITE    = 1 << 1;
        const EXEC     = 1 << 2;
        const NETWORK  = 1 << 3;
    }
}

pub struct Sandbox {
    caps: Capabilities,
}

impl Sandbox {
    pub fn new(caps: Capabilities) -> Self { Self { caps } }
    pub fn can_read(&self) -> bool { self.caps.contains(Capabilities::READ) }
    pub fn can_write(&self) -> bool { self.caps.contains(Capabilities::WRITE) }
    pub fn can_exec(&self) -> bool { self.caps.contains(Capabilities::EXEC) }
    pub fn can_network(&self) -> bool { self.caps.contains(Capabilities::NETWORK) }
}
