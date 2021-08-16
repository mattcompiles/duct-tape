use ast::*;
use std::path::Path;
use swc_atoms::JsWord;
use swc_common::DUMMY_SP;
use swc_ecmascript::ast;
use swc_ecmascript::visit::{Fold, FoldWith};

use crate::js_module::Dependency;
use crate::js_module::{ImportType, ModuleType, NamedImport};
use crate::utils::create_module_id;
use crate::utils::resolve_dependency;

pub fn runtime_imports(
    module: ast::Module,
    filepath: &Path,
    project_root: &Path,
) -> (Module, Vec<Dependency>, ModuleType) {
    let mut import_mapper = RuntimeImportMapper {
        dependencies: vec![],
        filepath,
        project_root,
        // Default to CJS until import/export is detected
        module_type: ModuleType::CommonJS,
    };

    let transformed_module = module.fold_with(&mut import_mapper);

    (
        transformed_module,
        import_mapper.dependencies,
        import_mapper.module_type,
    )
}

struct RuntimeImportMapper<'a> {
    filepath: &'a Path,
    project_root: &'a Path,
    dependencies: Vec<Dependency>,
    module_type: ModuleType,
}

impl<'a> Fold for RuntimeImportMapper<'a> {
    fn fold_module(&mut self, node: Module) -> Module {
        let mut node = node.fold_children_with(self);

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
                ModuleItem::ModuleDecl(decl) => {
                    // Detecting a ModuleDecl means the current file is ESM
                    self.module_type = ModuleType::ESM;
                    match decl {
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
                            node.body[i] =
                                create_runtime_export(&export_name, &default_export.expr);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        let mut runtime_imports: Vec<ModuleItem> = self
            .dependencies
            .iter()
            .filter(|import| match import.import_type {
                ImportType::Require => false,
                _ => true,
            })
            .map(|import| self.create_runtime_require(import))
            .collect();

        // Insert runtime imports at start of file
        runtime_imports.append(&mut node.body);
        node.body = runtime_imports;

        node
    }

    fn fold_import_decl(&mut self, node: ImportDecl) -> ImportDecl {
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
            (None, 0) => ImportType::SideEffect,
            (None, _) => ImportType::Named(named),
        };

        let dep_filepath = resolve_dependency(&node.src.value.clone(), self.filepath);

        self.dependencies.push(Dependency {
            import_type,
            id: create_module_id(&dep_filepath, &self.project_root),
            filepath: dep_filepath,
        });

        node
    }

    fn fold_call_expr(&mut self, node: CallExpr) -> CallExpr {
        // CommonJS Support
        let require_ident: JsWord = "require".into();

        let new_callee = match node.callee.clone() {
            ExprOrSuper::Expr(callee_expr) => match &*callee_expr {
                Expr::Ident(ident) => {
                    if ident.sym == require_ident {
                        Some(ExprOrSuper::Expr(Box::new(Expr::Ident(Ident {
                            sym: "__runtime_require__".into(),
                            span: DUMMY_SP,
                            optional: false,
                        }))))
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
        };

        if let Some(callee) = new_callee {
            let dep_filepath = match &*node.args[0].expr {
                Expr::Lit(lit) => match lit {
                    Lit::Str(request) => resolve_dependency(&request.value, self.filepath),
                    _ => {
                        panic!("Invalid syntax");
                    }
                },
                _ => {
                    panic!("Complex require statements not implemented");
                }
            };

            let dep_id = create_module_id(&dep_filepath, &self.project_root);

            self.dependencies.push(Dependency {
                import_type: ImportType::Require,
                id: dep_id.clone(),
                filepath: dep_filepath,
            });

            CallExpr {
                span: DUMMY_SP,
                callee,
                args: vec![ExprOrSpread {
                    expr: Box::from(Expr::Lit(Lit::Str(Str {
                        value: dep_id.into(),
                        span: DUMMY_SP,
                        has_escape: true,
                        kind: StrKind::Synthesized,
                    }))),
                    spread: None,
                }],
                ..node
            }
        } else {
            node
        }
    }
}

impl<'a> RuntimeImportMapper<'a> {
    fn create_runtime_require(&self, dependency: &Dependency) -> ModuleItem {
        let decl_name = match &dependency.import_type {
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
            ImportType::SideEffect => {
                panic!("NOT IMPLEMENTED: Side effect imports");
            }
            ImportType::Require => {
                panic!("Shouldn't happen");
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
                            value: dependency.id.clone().into(),
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
