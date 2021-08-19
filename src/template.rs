use crate::js_module::ModuleType;
use crate::Compilation;

pub fn render_chunk(entry_id: &String, c: &Compilation) -> String {
  let mut modules_in_chunk = c.graph.get_module_deps(&entry_id);
  modules_in_chunk.insert(entry_id);

  let mut module_map = String::from("{\n");

  for module_id in modules_in_chunk {
    let module = c.graph.modules.get(module_id).expect("Missing module id");

    match module.module_type {
      ModuleType::CommonJS => {
        module_map.push_str(&format!("\"{}\": [function(module, require) {{", module.id));
        module_map.push_str(&module.code);
        module_map.push_str("},'CJS'],")
      }
      ModuleType::ESM => {
        module_map.push_str(&format!(
          "\"{}\": [function(exports, require) {{",
          module.id
        ));
        module_map.push_str(&module.code);
        module_map.push_str("},'ESM'],")
      }
    }
  }

  module_map.push_str("\n}");

  format!(
    "
    var modules = {};
    var entry = \"{}\";
    function ductTape({{ modules, entry }}) {{
      var moduleCache = {{}};
      var interopRequireDefault = (exports, isDefaultImport, isCjs) => isDefaultImport && isCjs ? {{ default: exports }} : exports;
      var require = (moduleName, isDefaultImport) => {{
        if (!moduleCache[moduleName]) {{
          var exports = {{}};
          modules[moduleName][0](exports, require);
          moduleCache[moduleName] = modules[moduleName][1] === 'CJS' ? exports.exports : exports;
        }}

        return interopRequireDefault(moduleCache[moduleName], isDefaultImport, modules[moduleName][1] === 'CJS');
      }};
    
      // start the program
      require(entry);
    }}

    ductTape({{ modules, entry }});
    ",
    module_map, entry_id
  )
}
