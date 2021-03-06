use crate::utils::strip_invalid_chars;
use ast::*;
use swc_atoms::JsWord;
use swc_common::DUMMY_SP;
use swc_ecmascript::ast;
use swc_ecmascript::visit::{Fold, FoldWith};

use crate::js_module::Dependency;
use crate::js_module::{ImportType, ModuleType, NamedImport};

pub fn runtime_imports(module: ast::Module) -> (Module, Vec<Dependency>, ModuleType) {
    let mut import_mapper = RuntimeImportMapper {
        dependencies: vec![],
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

struct RuntimeImportMapper {
    dependencies: Vec<Dependency>,
    module_type: ModuleType,
}

impl Fold for RuntimeImportMapper {
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
            let mut exports = Vec::new();

            match &node.body[i] {
                ModuleItem::ModuleDecl(decl) => {
                    // Detecting a ModuleDecl means the current file is ESM
                    self.module_type = ModuleType::ESM;
                    match decl {
                        ModuleDecl::ExportNamed(export) => {
                            for specifier in &export.specifiers {
                                match specifier {
                                    ExportSpecifier::Namespace(namespace) => {
                                        if let Some(src) = &export.src {
                                            self.dependencies.push(Dependency {
                                                request: src.value.clone(),
                                                import_type: ImportType::Namespace(
                                                    namespace.name.sym.clone(),
                                                ),
                                            })
                                        } else {
                                            panic!("Invalid syntax: Namespace export must have src")
                                        }

                                        exports.push(create_runtime_export(
                                            &namespace.name.sym,
                                            &Box::new(Expr::Ident(namespace.name.clone())),
                                        ));
                                    }
                                    ExportSpecifier::Default(default_export) => {
                                        if let Some(src) = &export.src {
                                            self.dependencies.push(Dependency {
                                                request: src.value.clone(),
                                                import_type: ImportType::Default(
                                                    default_export.exported.sym.clone(),
                                                ),
                                            })
                                        }

                                        exports.push(create_runtime_export(
                                            &default_export.exported.sym,
                                            &Box::new(Expr::Ident(default_export.exported.clone())),
                                        ));
                                    }
                                    ExportSpecifier::Named(named_export) => {
                                        if let Some(src) = &export.src {
                                            self.dependencies.push(Dependency {
                                                request: src.value.clone(),
                                                // TODO: Group all named imports/exports from the same module into the same Dependency
                                                import_type: ImportType::Named(vec![NamedImport {
                                                    local: named_export.orig.sym.clone(),
                                                    import_name: named_export.orig.sym.clone(),
                                                }]),
                                            })
                                        }

                                        let export_ident;

                                        if let Some(exported) = &named_export.exported {
                                            export_ident = exported;
                                        } else {
                                            export_ident = &named_export.orig;
                                        }

                                        exports.push(create_runtime_export(
                                            &export_ident.sym,
                                            &Box::new(Expr::Ident(named_export.orig.clone())),
                                        ));
                                    }
                                }
                            }
                        }
                        ModuleDecl::ExportDecl(named_export) => match &named_export.decl {
                            Decl::Var(var_decl) => {
                                exports.push(create_runtime_export(
                                    match &var_decl.decls[0].name {
                                        Pat::Ident(ident) => &ident.id.sym,
                                        _ => panic!("Not implemented"),
                                    },
                                    var_decl.decls[0]
                                        .init
                                        .as_ref()
                                        .expect("export const with no initialiser"),
                                ));
                            }
                            Decl::Fn(fn_decl) => {
                                exports.push(create_runtime_export(
                                    &fn_decl.ident.sym,
                                    &Box::new(Expr::Fn(FnExpr {
                                        ident: Some(fn_decl.ident.clone()),
                                        function: fn_decl.function.clone(),
                                    })),
                                ));
                            }
                            _ => {}
                        },
                        ModuleDecl::ExportDefaultExpr(default_export) => {
                            let export_name: JsWord = "default".into();
                            exports.push(create_runtime_export(&export_name, &default_export.expr));
                        }
                        ModuleDecl::ExportAll(export_all) => {
                            let export_local =
                                format!("namespace_{}", strip_invalid_chars(&export_all.src.value));

                            self.dependencies.push(Dependency {
                                request: export_all.src.value.clone(),
                                import_type: ImportType::Namespace(export_local.clone().into()),
                            });

                            exports.push(Box::new(Expr::Call(CallExpr {
                                span: DUMMY_SP,
                                type_args: None,
                                callee: ExprOrSuper::Expr(Box::new(Expr::Ident(Ident {
                                    span: DUMMY_SP,
                                    optional: false,
                                    sym: "__exportAll__".into(),
                                }))),
                                args: vec![ExprOrSpread {
                                    spread: None,
                                    expr: Box::new(Expr::Ident(Ident {
                                        span: DUMMY_SP,
                                        optional: false,
                                        sym: export_local.into(),
                                    })),
                                }],
                            })));
                        }
                        _ => {}
                    }
                }
                _ => {}
            };

            if exports.len() > 0 {
                node.body[i] = ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                    span: DUMMY_SP,
                    expr: Box::new(Expr::Seq(SeqExpr {
                        span: DUMMY_SP,
                        exprs: exports,
                    })),
                }))
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
        if node.specifiers.len() == 0 {
            // No specifiers means a side effect import
            self.dependencies.push(Dependency {
                request: node.src.value.clone(),
                import_type: ImportType::SideEffect,
            });

            return node;
        }

        let mut namespace = None;
        let mut named: Vec<NamedImport> = vec![];
        let mut default = None;

        for specifier in &node.specifiers {
            match specifier {
                ImportSpecifier::Default(default_import) => {
                    default = Some(default_import.local.sym.clone());
                }
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

        if let Some(namespace_local) = namespace {
            self.dependencies.push(Dependency {
                request: node.src.value.clone(),
                import_type: ImportType::Namespace(namespace_local),
            });
        }

        if let Some(default_local) = default {
            self.dependencies.push(Dependency {
                request: node.src.value.clone(),
                import_type: ImportType::Default(default_local),
            });
        }

        if named.len() > 0 {
            self.dependencies.push(Dependency {
                request: node.src.value.clone(),
                import_type: ImportType::Named(named),
            });
        }

        node
    }

    // CommonJS Support
    fn fold_call_expr(&mut self, node: CallExpr) -> CallExpr {
        let require_ident: JsWord = "require".into();

        let is_require_call = match node.callee.clone() {
            ExprOrSuper::Expr(callee_expr) => match &*callee_expr {
                Expr::Ident(ident) => {
                    if ident.sym == require_ident {
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
            _ => false,
        };

        if is_require_call {
            let request = match &*node.args[0].expr {
                Expr::Lit(lit) => match lit {
                    Lit::Str(request) => &request.value,
                    _ => {
                        panic!("Invalid syntax");
                    }
                },
                _ => {
                    panic!("Complex require statements not implemented");
                }
            };

            self.dependencies.push(Dependency {
                request: request.clone(),
                import_type: ImportType::Require,
            });
        }

        node
    }
}

impl RuntimeImportMapper {
    fn create_runtime_require(&self, dependency: &Dependency) -> ModuleItem {
        let mut is_default_import = false;
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
            ImportType::Default(local) => {
                is_default_import = true;

                Pat::Object(ObjectPat {
                    span: DUMMY_SP,
                    optional: false,
                    type_ann: None,
                    props: vec![ObjectPatProp::KeyValue(KeyValuePatProp {
                        key: PropName::Ident(Ident {
                            span: DUMMY_SP,
                            optional: false,
                            sym: "default".into(),
                        }),
                        value: Box::from(Pat::Ident(BindingIdent::from(Ident {
                            sym: local.clone(),
                            span: DUMMY_SP,
                            optional: false,
                        }))),
                    })],
                })
            }
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
                        sym: "require".into(),
                        span: DUMMY_SP,
                        optional: false,
                    }))),
                    args: vec![
                        ExprOrSpread {
                            expr: Box::from(Expr::Lit(Lit::Str(Str {
                                value: dependency.request.clone().into(),
                                span: DUMMY_SP,
                                has_escape: true,
                                kind: StrKind::Synthesized,
                            }))),
                            spread: None,
                        },
                        ExprOrSpread {
                            expr: Box::from(Expr::Lit(Lit::Bool(Bool {
                                span: DUMMY_SP,
                                value: is_default_import,
                            }))),
                            spread: None,
                        },
                    ],
                    span: DUMMY_SP,
                    type_args: None,
                }))),
                span: DUMMY_SP,
                definite: false,
            }],
        })))
    }
}

fn create_runtime_export(name: &JsWord, value: &Box<Expr>) -> Box<Expr> {
    Box::new(Expr::Assign(AssignExpr {
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
    }))
}
