pub trait WasmComponent {
    fn id(&self) -> String;
    fn version(&self) -> String;
    fn capabilities(&self) -> Vec<String>;
}

pub struct ComponentManager {
    pub registry: std::collections::HashMap<String, Box<dyn WasmComponent>>,
}

impl ComponentManager {
    pub fn new() -> Self {
        Self { registry: std::collections::HashMap::new() }
    }
    pub fn register(&mut self, component: Box<dyn WasmComponent>) {
        self.registry.insert(component.id(), component);
    }
}