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
    Namespace(JsWord),
    Named(Vec<NamedImport>),
    SideEffect,
    Require,
}

#[derive(Clone)]
pub struct Dependency {
    pub id: String,
    pub filepath: PathBuf,
    pub import_type: ImportType,
}
