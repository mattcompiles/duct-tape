use std::path::Path;

pub fn create_module_id(path: &Path, project_root: &Path) -> String {
    String::from(
        path.strip_prefix(project_root)
            .expect("Failed to strip CWD")
            .to_str()
            .expect("Failed to strip CWD"),
    )
}

pub fn strip_invalid_chars(value: &str) -> String {
    value
        .chars()
        .map(|x| match x {
            '/' => '_',
            '.' => '_',
            '-' => '_',
            _ => x,
        })
        .collect()
}
