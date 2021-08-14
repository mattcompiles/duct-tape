use std::path::PathBuf;
use swc_atoms::JsWord;

pub struct JsModule {
    pub id: String,
    pub filepath: PathBuf,
    pub code: String,
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
    SideEffect(),
}

#[derive(Clone)]
pub struct Dependency {
    pub id: String,
    pub filepath: PathBuf,
    pub import_type: ImportType,
}
