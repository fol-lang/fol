(source_file) @local.scope
(block) @local.scope

(fun_decl declaration: (plain_fun_decl name: (identifier) @local.definition.function))
(fun_decl declaration: (method_decl name: (identifier) @local.definition.method))
(pro_decl declaration: (plain_pro_decl name: (identifier) @local.definition.function))
(pro_decl declaration: (method_decl name: (identifier) @local.definition.method))
(log_decl declaration: (plain_log_decl name: (identifier) @local.definition.function))
(log_decl declaration: (method_decl name: (identifier) @local.definition.method))
(typ_decl name: (identifier) @local.definition.type)
(ali_decl name: (identifier) @local.definition.type)
(param name: (identifier) @local.definition)
(var_decl (typed_binding name: (identifier) @local.definition))
(con_decl (typed_binding name: (identifier) @local.definition))
(lab_decl (typed_binding name: (identifier) @local.definition))

(identifier) @local.reference
