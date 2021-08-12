use node_resolve::resolve_from;
use std::path::{Path, PathBuf};

pub fn create_module_id(path: &Path, project_root: &Path) -> String {
    String::from(
        path.strip_prefix(project_root)
            .expect("Failed to strip CWD")
            .to_str()
            .expect("Failed to strip CWD"),
    )
}

pub fn resolve_dependency(request: &str, from: &Path) -> PathBuf {
    resolve_from(request, PathBuf::from(&from.parent().unwrap()))
        .expect(&format!("Failed to resolve {} from {:?}", request, &from))
}
