use crate::module_graph::ModuleGraph;

pub fn render_chunk(graph: &ModuleGraph, entrypoint: &str) -> String {
    format!(
        "
    const modules = {};
    const entry = \"{}\";
    function ductTape({{ modules, entry }}) {{
      const moduleCache = {{}};
      const require = moduleName => {{
        // if in cache, return the cached version
        if (moduleCache[moduleName]) {{
          return moduleCache[moduleName];
        }}
        const exports = {{}};

        moduleCache[moduleName] = exports;

        modules[moduleName](exports, require);
        return moduleCache[moduleName];
      }};
    
      // start the program
      require(entry);
    }}

    ductTape({{ modules, entry }});
    ",
        render_module_map(graph),
        entrypoint
    )
}

fn render_module_map(graph: &ModuleGraph) -> String {
    let mut module_map = String::from("{\n");

    for (path, module) in &graph.modules {
        module_map.push_str(&format!(
            "\"{}\": function(exports, __runtime_require__) {{",
            path.to_str().unwrap()
        ));

        module_map.push_str(&module.code);

        module_map.push_str("},")
    }

    module_map.push_str("}");

    module_map
}
