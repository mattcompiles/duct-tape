use crate::module_graph::ModuleGraph;

pub fn render_chunk(graph: &ModuleGraph, entry_id: &String) -> String {
  let mut modules_in_chunk = graph.get_module_deps(&entry_id);
  modules_in_chunk.insert(entry_id);

  let mut module_map = String::from("{\n");

  for module_id in modules_in_chunk {
    let module = graph.modules.get(module_id).expect("Missing module id");

    module_map.push_str(&format!(
      "\"{}\": function(exports, __runtime_require__) {{",
      module.id
    ));

    module_map.push_str(&module.code);

    module_map.push_str("},")
  }

  module_map.push_str("}");

  format!(
    "
    const modules = {};
    const entry = \"{}\";
    function ductTape({{ modules, entry }}) {{
      const moduleCache = {{}};
      const require = moduleName => {{
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
    module_map, entry_id
  )
}
