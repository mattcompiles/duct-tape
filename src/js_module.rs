use std::path::PathBuf;
use swc_atoms::JsWord;

pub enum ModuleType {
    ESM,
    CommonJS,
}

pub struct JsModule {
    pub id: String,
    pub filepath: PathBuf,
    pub code: String,
    pub module_type: ModuleType,
}

#[derive(Clone)]
pub struct NamedImport {
    pub local: JsWord,
    pub import_name: JsWord,
}

#[derive(Clone)]
pub enum ImportType {
    Default(JsWord),
    Namespace(JsWord),
    Named(Vec<NamedImport>),
    SideEffect,
    Require,
}

#[derive(Clone)]
pub struct Dependency {
    pub request: JsWord,
    pub import_type: ImportType,
}

impl JsModule {
    pub fn update_dep_src(&mut self, request: &str, dep_id: &str) {
        // TODO: Hardcoded to max 5 replaces, should be equal to the amount of required replaces
        self.code = self.code.replacen(&request, &dep_id, 5);
    }
}
