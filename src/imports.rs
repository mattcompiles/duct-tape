use std::collections::HashSet;

use ast::*;
use swc_atoms::JsWord;
use swc_common::DUMMY_SP;
use swc_ecmascript::ast;
use swc_ecmascript::visit::{Node, Visit, VisitWith};

pub fn collect_imports(module: &ast::Module) -> HashSet<JsWord> {
    let mut c = ImportCollector {
        imports: HashSet::new(),
    };
    module.visit_with(&ast::Invalid { span: DUMMY_SP } as _, &mut c);
    return c.imports;
}

struct ImportCollector {
    imports: HashSet<JsWord>,
}

impl Visit for ImportCollector {
    fn visit_import_decl(&mut self, node: &ImportDecl, _parent: &dyn Node) {
        self.imports.insert(node.src.value.clone());
    }
}
