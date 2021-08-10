use node_resolve::resolve_from;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecmascript::codegen::text_writer::JsWriter;

use crate::parser::parse;

use ast::*;
use swc_atoms::JsWord;
use swc_common::DUMMY_SP;
use swc_ecmascript::ast;
use swc_ecmascript::visit::{Fold, FoldWith, Node, Visit, VisitWith};

#[derive(Clone)]
pub struct JsModule {
    pub filename: String,
    pub ast: Module,
    pub source_map: Lrc<SourceMap>,
    pub dependencies: HashSet<PathBuf>,
    pub imports: Vec<ImportMeta>,
    pub comments: SingleThreadedComments,
}

impl JsModule {
    pub fn new(path: &Path) -> Result<JsModule, String> {
        println!("Loading JS module: {}", path.to_str().unwrap());
        let root_relative_path = path
            .clone()
            .strip_prefix(env::current_dir().expect("Couldn't access CWD"))
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

        let source_map = Lrc::new(SourceMap::default());
        let src_code = fs::read_to_string(&path).unwrap();

        let (module, comments) = match parse(&src_code, path.to_str().unwrap(), &source_map) {
            Err(_) => return Err(String::from("Error parsing module")),
            Ok(module) => module,
        };

        let imports = collect_imports(&module);

        let mut dependencies = HashSet::new();

        for import in &imports {
            dependencies.insert(
                resolve_from(&import.path, PathBuf::from(&path.parent().unwrap())).expect(
                    &format!("Failed to resolve {} from {:?}", &import.path, &path),
                ),
            );
        }

        Ok(JsModule {
            filename: root_relative_path,
            dependencies,
            ast: module,
            imports,
            source_map,
            comments,
        })
    }

    pub fn render(&self) -> Result<String, String> {
        // TODO: Don't use clone here
        let final_ast = self.ast.clone().fold_with(&mut RuntimeImportMapper {
            imports: self.imports.clone(),
        });

        let buf = match self.emit(&final_ast) {
            Err(_) => return Err(format!("Failed to emit buffer: {}", self.filename)),
            Ok(value) => value,
        };

        match String::from_utf8(buf) {
            Err(_) => Err(format!(
                "Failed to convert UTF-8 buffer to string buffer: {}",
                self.filename
            )),
            Ok(value) => Ok(value),
        }
    }

    fn emit(&self, ast: &Module) -> Result<Vec<u8>, std::io::Error> {
        let mut buf = vec![];
        {
            let writer = Box::new(JsWriter::new(self.source_map.clone(), "\n", &mut buf, None));
            let config = swc_ecmascript::codegen::Config { minify: false };
            let mut emitter = swc_ecmascript::codegen::Emitter {
                cfg: config,
                comments: Some(&self.comments),
                cm: self.source_map.clone(),
                wr: writer,
            };
            emitter.emit_module(ast)?;
        }
        return Ok(buf);
    }
}

fn collect_imports(module: &ast::Module) -> Vec<ImportMeta> {
    let mut c = ImportExportCollector { imports: vec![] };
    module.visit_with(&ast::Invalid { span: DUMMY_SP } as _, &mut c);
    return c.imports;
}

#[derive(Clone)]
struct NamedImport {
    local: JsWord,
    import_name: JsWord,
}

#[derive(Clone)]
enum ImportType {
    Namespace(JsWord),
    Named(Vec<NamedImport>),
    SideEffect(),
}

#[derive(Clone)]
pub struct ImportMeta {
    import_type: ImportType,
    path: JsWord,
}

struct ImportExportCollector {
    imports: Vec<ImportMeta>,
}

impl Visit for ImportExportCollector {
    fn visit_import_decl(&mut self, node: &ImportDecl, _parent: &dyn Node) {
        let mut namespace = None;
        let mut named: Vec<NamedImport> = vec![];

        for specifier in &node.specifiers {
            match specifier {
                ImportSpecifier::Default(default_import) => named.push(NamedImport {
                    local: default_import.local.sym.clone(),
                    import_name: "default".into(),
                }),
                ImportSpecifier::Named(named_import) => {
                    let imported = match &named_import.imported {
                        Some(i) => i.sym.clone(),
                        None => named_import.local.sym.clone(),
                    };
                    named.push(NamedImport {
                        local: named_import.local.sym.clone(),
                        import_name: imported,
                    })
                }
                ImportSpecifier::Namespace(ns_import) => {
                    namespace = Some(ns_import.local.sym.clone())
                }
            }
        }

        let import_type = match (namespace, node.specifiers.len()) {
            (Some(local), _) => ImportType::Namespace(local),
            (None, 0) => ImportType::SideEffect(),
            (None, _) => ImportType::Named(named),
        };
        self.imports.push(ImportMeta {
            import_type,
            path: node.src.value.clone(),
        });
    }
}

struct RuntimeImportMapper {
    imports: Vec<ImportMeta>,
}

impl Fold for RuntimeImportMapper {
    fn fold_module(&mut self, mut node: Module) -> Module {
        // Remove all import statements
        node.body.retain(|module_item| match module_item {
            ModuleItem::ModuleDecl(decl) => match decl {
                ModuleDecl::Import(_) => false,
                _ => true,
            },
            _ => true,
        });

        let mut runtime_imports: Vec<ModuleItem> = self
            .imports
            .iter()
            .map(|import| create_runtime_require(import))
            .collect();

        // Insert runtime imports at start of file
        runtime_imports.append(&mut node.body);
        node.body = runtime_imports;

        node
    }
}

fn create_runtime_require(import: &ImportMeta) -> ModuleItem {
    let decl_name = match &import.import_type {
        ImportType::Namespace(local) => Pat::Ident(BindingIdent::from(Ident {
            sym: local.into(),
            span: DUMMY_SP,
            optional: false,
        })),
        ImportType::Named(locals) => Pat::Object(ObjectPat {
            span: DUMMY_SP,
            optional: false,
            type_ann: None,
            props: locals
                .iter()
                .map(|named_import| {
                    if named_import.import_name == named_import.local {
                        ObjectPatProp::Assign(AssignPatProp {
                            span: DUMMY_SP,
                            key: Ident {
                                sym: named_import.local.clone(),
                                span: DUMMY_SP,
                                optional: false,
                            },
                            value: None,
                        })
                    } else {
                        ObjectPatProp::KeyValue(KeyValuePatProp {
                            key: PropName::Ident(Ident {
                                span: DUMMY_SP,
                                optional: false,
                                sym: named_import.import_name.clone(),
                            }),
                            value: Box::from(Pat::Ident(BindingIdent::from(Ident {
                                sym: named_import.local.clone(),
                                span: DUMMY_SP,
                                optional: false,
                            }))),
                        })
                    }
                })
                .collect(),
        }),
        ImportType::SideEffect() => {
            panic!("NOT IMPLEMENTED: Side effect imports");
        }
    };

    ModuleItem::Stmt(Stmt::Decl(Decl::Var(VarDecl {
        kind: VarDeclKind::Var,
        declare: false,
        span: DUMMY_SP,
        decls: vec![VarDeclarator {
            name: decl_name,
            init: Some(Box::new(Expr::Call(CallExpr {
                callee: ExprOrSuper::Expr(Box::from(Expr::Ident(Ident {
                    sym: "__runtime_require__".into(),
                    span: DUMMY_SP,
                    optional: false,
                }))),
                args: vec![ExprOrSpread {
                    expr: Box::from(Expr::Lit(Lit::Str(Str {
                        value: import.path.clone().into(),
                        span: DUMMY_SP,
                        has_escape: true,
                        kind: StrKind::Synthesized,
                    }))),
                    spread: None,
                }],
                span: DUMMY_SP,
                type_args: None,
            }))),
            span: DUMMY_SP,
            definite: false,
        }],
    })))
}
