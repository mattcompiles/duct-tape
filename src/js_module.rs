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
    pub filename: PathBuf,
    pub code: String,
    pub dependencies: HashSet<PathBuf>,
    pub imports: Vec<ImportMeta>,
}

impl JsModule {
    pub fn new(path: &Path) -> Result<Self, String> {
        println!("Loading JS module: {}", path.to_str().unwrap());
        let root_relative_path = path
            .clone()
            .strip_prefix(env::current_dir().expect("Couldn't access CWD"))
            .expect("Failed to strip CWD")
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
            dependencies.insert(import.get_resolved_path(path));
        }

        let final_ast = module.fold_with(&mut RuntimeImportMapper {
            // TODO: Don't use clone here
            imports: imports.clone(),
            filename: path,
        });

        let buf = match emit(&final_ast, source_map, comments) {
            Err(_) => {
                return Err(format!(
                    "Failed to emit buffer: {}",
                    &root_relative_path.to_str().unwrap()
                ))
            }
            Ok(value) => value,
        };

        let code = match String::from_utf8(buf) {
            Err(_) => {
                return Err(format!(
                    "Failed to convert UTF-8 buffer to string buffer: {}",
                    &root_relative_path.to_str().unwrap(),
                ))
            }
            Ok(value) => value,
        };

        Ok(JsModule {
            filename: path.to_owned(),
            dependencies,
            code,
            imports,
        })
    }
}

fn emit(
    ast: &Module,
    source_map: Lrc<SourceMap>,
    comments: SingleThreadedComments,
) -> Result<Vec<u8>, std::io::Error> {
    let mut buf = vec![];
    {
        let writer = Box::new(JsWriter::new(source_map.clone(), "\n", &mut buf, None));
        let config = swc_ecmascript::codegen::Config { minify: false };
        let mut emitter = swc_ecmascript::codegen::Emitter {
            cfg: config,
            comments: Some(&comments),
            cm: source_map.clone(),
            wr: writer,
        };
        emitter.emit_module(ast)?;
    }
    return Ok(buf);
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

impl ImportMeta {
    fn get_resolved_path(&self, from: &Path) -> PathBuf {
        resolve_from(&self.path, PathBuf::from(&from.parent().unwrap())).expect(&format!(
            "Failed to resolve {} from {:?}",
            &self.path, &from
        ))
    }
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

struct RuntimeImportMapper<'a> {
    filename: &'a Path,
    imports: Vec<ImportMeta>,
}

impl<'a> Fold for RuntimeImportMapper<'a> {
    fn fold_module(&mut self, mut node: Module) -> Module {
        // Remove all import statements
        node.body.retain(|module_item| match module_item {
            ModuleItem::ModuleDecl(decl) => match decl {
                ModuleDecl::Import(_) => false,
                _ => true,
            },
            _ => true,
        });

        for i in 0..node.body.len() {
            match &node.body[i] {
                ModuleItem::ModuleDecl(decl) => match decl {
                    ModuleDecl::ExportNamed(_named_export) => {
                        panic!("ModuleDecl::ExportNamed: Not implemented");
                    }
                    ModuleDecl::ExportDecl(named_export) => match &named_export.decl {
                        Decl::Var(var_decl) => {
                            node.body[i] = create_runtime_export(
                                match &var_decl.decls[0].name {
                                    Pat::Ident(ident) => &ident.id.sym,
                                    _ => panic!("Not implemented"),
                                },
                                var_decl.decls[0]
                                    .init
                                    .as_ref()
                                    .expect("export const with no initialiser"),
                            );
                        }
                        _ => {}
                    },
                    ModuleDecl::ExportDefaultExpr(default_export) => {
                        let export_name: JsWord = "default".into();
                        node.body[i] = create_runtime_export(&export_name, &default_export.expr);
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        let mut runtime_imports: Vec<ModuleItem> = self
            .imports
            .iter()
            .map(|import| self.create_runtime_require(import))
            .collect();

        // Insert runtime imports at start of file
        runtime_imports.append(&mut node.body);
        node.body = runtime_imports;

        node
    }
}

impl<'a> RuntimeImportMapper<'a> {
    fn create_runtime_require(&self, import: &ImportMeta) -> ModuleItem {
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
                            value: import
                                .get_resolved_path(&self.filename)
                                .to_str()
                                .unwrap()
                                .into(),
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
}

fn create_runtime_export(name: &JsWord, value: &Box<Expr>) -> ModuleItem {
    ModuleItem::Stmt(Stmt::Expr(ExprStmt {
        span: DUMMY_SP,
        expr: Box::new(Expr::Assign(AssignExpr {
            span: DUMMY_SP,

            op: AssignOp::Assign,

            left: PatOrExpr::Expr(Box::new(Expr::Member(MemberExpr {
                span: DUMMY_SP,
                obj: ExprOrSuper::Expr(Box::new(Expr::Ident(Ident {
                    span: DUMMY_SP,
                    sym: "exports".into(),
                    optional: false,
                }))),
                prop: Box::new(Expr::Ident(Ident {
                    span: DUMMY_SP,
                    sym: name.into(),
                    optional: false,
                })),
                computed: false,
            }))),

            right: value.clone(),
        })),
    }))
}
