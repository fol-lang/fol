(source_file) @symbol.scope
(block) @symbol.scope

(fun_decl declaration: (plain_fun_decl name: (identifier) @symbol.function))
(fun_decl declaration: (method_decl name: (identifier) @symbol.method))
(pro_decl declaration: (plain_pro_decl name: (identifier) @symbol.function))
(pro_decl declaration: (method_decl name: (identifier) @symbol.method))
(log_decl declaration: (plain_log_decl name: (identifier) @symbol.function))
(log_decl declaration: (method_decl name: (identifier) @symbol.method))
(typ_decl name: (identifier) @symbol.type)
(ali_decl name: (identifier) @symbol.type)
(var_decl (typed_binding name: (identifier) @symbol.variable))
(con_decl (typed_binding name: (identifier) @symbol.variable))
(lab_decl (typed_binding name: (identifier) @symbol.variable))
(select_arm binding: (identifier) @symbol.variable)
(seg_decl name: (identifier) @symbol.namespace)
(std_decl name: (identifier) @symbol.type)
(use_decl name: (identifier) @symbol.namespace)
