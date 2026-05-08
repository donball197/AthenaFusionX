#[derive(Default, Debug)]
pub struct VmoState {
    pub last_parent: Option<u32>,
    pub last_child: Option<u32>,
}
