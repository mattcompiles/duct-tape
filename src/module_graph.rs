use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

use crate::js_module::JsModule;

type ModuleId = String;

pub struct ModuleGraph {
    pub modules: HashMap<ModuleId, JsModule>,
    pub entrypoints: Vec<ModuleId>,
    pub dependency_map: HashMap<ModuleId, Vec<ModuleId>>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            dependency_map: HashMap::new(),
            entrypoints: Vec::new(),
        }
    }

    pub fn add_module(&mut self, module: JsModule) {
        self.modules.insert(module.id.to_string(), module);
    }

    pub fn has_module(&self, id: &str) -> bool {
        self.modules.contains_key(id)
    }

    pub fn add_dependency(&mut self, id: &str, dep_id: &str) {
        match self.dependency_map.get_mut(id) {
            Some(deps) => {
                deps.push(dep_id.to_string());
            }
            None => {
                let deps = vec![dep_id.to_string()];
                self.dependency_map.insert(id.to_string(), deps);
            }
        };
    }

    pub fn add_entrypoint(&mut self, id: String) {
        self.entrypoints.push(id.to_string());
    }

    pub fn get_module_deps(&self, module_id: &str) -> HashSet<&String> {
        let mut module_deps = HashSet::new();

        match self.dependency_map.get(module_id) {
            Some(deps) => {
                for dep in deps {
                    module_deps.extend(&self.get_module_deps(dep));
                    module_deps.insert(dep);
                }
            }
            None => {}
        }

        module_deps
    }
}

impl<'a> fmt::Debug for ModuleGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut modules: String = String::new();
        for (module_id, module) in &self.modules {
            modules.push_str(&format!("{}\n", module_id));
            modules.push_str(&format!("{}\n", module.code));
        }
        write!(f, "{}", modules)
    }
}
