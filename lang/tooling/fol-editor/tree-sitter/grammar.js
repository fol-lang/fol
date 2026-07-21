module.exports = grammar({
  name: 'fol',

  word: $ => $.identifier,

  reserved: {
    global: $ => [$._removed_keyword],
  },

  extras: $ => [
    /\s/,
    $.comment,
    $.doc_comment,
  ],

  conflicts: $ => [
    [$.record_literal, $.block],
    [$.record_literal, $.container_literal],
    [$.block, $.stmt],
    [$.block, $.record_literal, $.container_literal],
    [$.block, $.stmt, $.container_literal],
    [$.block, $.container_literal],
    [$.expr, $.expr_atom],
    [$.stmt, $.expr],
    [$.generic_params, $.type_contract_claims],
    [$.generic_param, $.param],
    [$.generic_param, $.type_expr],
    [$.if_stmt, $.if_expr],
    [$.expr, $.range_expr],
    [$.else_clause, $.if_expr],
    [$.type_expr, $.generic_type_expr],
  ],

  rules: {
    source_file: $ => seq(
      optional($._reserved_word_anchor),
      repeat(choice($._top_level_item, ';')),
    ),

    // Keep deleted keywords reachable for Tree-sitter's reserved-word table
    // without admitting them into ordinary FOL source.
    _reserved_word_anchor: $ => seq($._reserved_word_anchor_token, repeat1($._removed_keyword)),
    _reserved_word_anchor_token: $ => alias('\u0001', $.comment),
    _removed_keyword: _ => token(choice('defer', 'go')),

    _top_level_item: $ => choice(
      $.use_decl,
      $.var_decl,
      $.con_decl,
      $.lab_decl,
      $.fun_decl,
      $.pro_decl,
      $.log_decl,
      $.typ_decl,
      $.ali_decl,
      $.def_decl,
      $.seg_decl,
      $.std_decl,
      $.comment,
      $.doc_comment,
    ),

    use_decl: $ => seq('use', field('name', $.identifier), ':', field('source_kind', $.source_kind), '=', '{', field('target', $.string_literal), '}'),
    var_decl: $ => prec.right(seq(choice('var', '+var', '~var', '-var', '!var', '?var', '@var'), optional(field('modifiers', $.decl_modifiers)), $.typed_binding, optional(seq('=', field('value', $.expr))))),
    con_decl: $ => seq(choice('con', '-con'), optional(field('modifiers', $.decl_modifiers)), $.typed_binding, '=', field('value', $.expr)),
    lab_decl: $ => seq('lab', optional(field('modifiers', $.decl_modifiers)), $.typed_binding, '=', field('value', $.expr)),
    fun_decl: $ => seq('fun', optional(field('modifiers', $.decl_modifiers)), field('declaration', choice($.plain_fun_decl, $.method_decl))),
    pro_decl: $ => seq('pro', optional(field('modifiers', $.decl_modifiers)), field('declaration', choice($.plain_pro_decl, $.method_decl))),
    log_decl: $ => seq('log', optional(field('modifiers', $.decl_modifiers)), field('declaration', choice($.plain_log_decl, $.method_decl))),
    typ_decl: $ => seq(
      'typ',
      optional(field('modifiers', $.decl_modifiers)),
      field('name', $.identifier),
      optional(field('generics', $.generic_params)),
      optional(field('contracts', $.type_contract_claims)),
      ':',
      choice($.record_type, $.entry_type),
      '=',
      $.type_block,
    ),
    ali_decl: $ => seq('ali', optional(field('modifiers', $.decl_modifiers)), field('name', $.identifier), ':', field('target', $.type_expr)),
    def_decl: $ => seq('def', optional(field('modifiers', $.decl_modifiers)), field('name', $.identifier), optional($.params), ':', field('def_type', $.type_expr), optional(seq('=', $.block))),
    seg_decl: $ => seq('seg', optional(field('modifiers', $.decl_modifiers)), field('name', $.identifier), ':', field('seg_type', $.type_expr), '=', $.block),
    std_decl: $ => seq(
      'std',
      optional(field('modifiers', $.decl_modifiers)),
      field('name', $.identifier),
      optional(field('generics', $.generic_params)),
      ':',
      field('kind', choice('pro', 'blu', 'ext')),
      '=',
      $.standard_block,
    ),

    source_kind: _ => choice('loc', 'pkg'),
    decl_modifiers: $ => seq('[', optional($.modifier_list), ']'),
    modifier_list: $ => seq($.identifier, repeat(seq(choice(',', ';'), $.identifier)), optional(choice(',', ';'))),
    typed_binding: $ => seq(field('name', $.identifier), optional(seq(':', field('type', $.type_expr)))),
    plain_fun_decl: $ => seq(field('name', $.identifier), optional(field('generics', $.generic_params)), $.params, optional($.return_type), optional($.error_type), '=', $.block),
    plain_pro_decl: $ => seq(field('name', $.identifier), optional(field('generics', $.generic_params)), $.params, optional($.return_type), optional($.error_type), '=', $.block),
    plain_log_decl: $ => seq(field('name', $.identifier), optional(field('generics', $.generic_params)), $.params, optional($.return_type), optional($.error_type), '=', $.block),
    method_decl: $ => seq($.receiver, field('name', $.identifier), optional(field('generics', $.generic_params)), $.params, optional($.return_type), optional($.error_type), '=', $.block),
    receiver: $ => seq('(', $.type_expr, ')'),
    generic_params: $ => seq('(', optional(commaSep($.generic_param)), ')'),
    generic_param: $ => seq(field('name', $.identifier), optional(seq(':', field('constraint', $.type_expr)))),
    type_contract_claims: $ => seq('(', optional(commaSep($.type_expr)), ')'),
    params: $ => seq('(', optional(commaSep($.param)), ')'),
    param: $ => seq(
      field('name', $.identifier),
      optional(field('options', $.parameter_options)),
      ':',
      optional('...'),
      field('type', $.type_expr),
      optional(seq('=', field('default', $.expr))),
    ),
    parameter_options: $ => seq('[', commaSep1(seq(choice('bor', 'mut'), optional(seq('=', $.identifier)))), ']'),
    return_type: $ => seq(':', $.type_expr),
    error_type: $ => seq('/', $.type_expr),
    record_type: _ => 'rec',
    entry_type: _ => 'ent',

    type_expr: $ => choice(
      $.pointer_type,
      $.channel_type,
      $.eventual_type,
      $.mutex_type,
      $.function_type,
      $.generic_type_expr,
      $.qualified_path,
      $.identifier,
      $.container_type,
      $.shell_type,
      $.owned_type,
    ),

    // Braced routine type `{fun (n: int): int}` with an optional environment
    // lifetime `[bor=L]` marking an escaping-closure type (V3 section 5.3).
    function_type: $ => prec.right(seq(
      '{',
      'fun',
      optional(field('name', $.identifier)),
      $.params,
      optional($.return_type),
      optional($.error_type),
      '}',
      optional(seq('[', 'bor', '=', field('env_lifetime', $.identifier), ']')),
    )),

    generic_type_expr: $ => seq(
      field('base', choice($.qualified_path, $.identifier)),
      '[',
      // A bracket item may carry a lifetime assignment, e.g. `Job[bor=L]`.
      commaSep1(choice(seq($.type_expr, optional(seq('=', $.identifier))), $.integer_literal)),
      ']',
    ),
    container_type: $ => seq(choice('arr', 'vec', 'seq', 'set', 'map'), '[', commaSep1(choice($.type_expr, $.integer_literal)), ']'),
    channel_type: $ => seq(
      'chn',
      '[',
      optional(seq(field('endpoint', choice('tx', 'rx')), ',')),
      field('element', $.type_expr),
      ']',
    ),
    // `evt[T]`, `evt[L, T]`, `evt[T / E]`, `evt[L, T / E]`.
    eventual_type: $ => seq(
      'evt',
      '[',
      field('first', $.type_expr),
      optional(seq(',', field('second', $.type_expr))),
      optional(seq('/', field('error', $.type_expr))),
      ']',
    ),
    mutex_type: $ => seq('mux', '[', field('target', $.type_expr), ']'),
    pointer_type: $ => prec(2, seq(
      'ptr',
      '[',
      optional(seq(field('qualifier', choice('shared', 'weak', 'raw')), ',')),
      optional(seq(field('sync', 'sync'), ',')),
      field('target', $.type_expr),
      ']',
    )),
    shell_type: $ => choice(
      seq(choice('opt', 'err'), '[', $.type_expr, ']'),
      seq('opt', $.owned_type),
      seq('opt', $.pointer_type),
    ),
    owned_type: $ => seq('@', $.type_expr),

    block: $ => seq('{', repeat(choice($.stmt, ';')), optional($.expr), '}'),
    type_block: $ => seq('{', repeat(choice($.var_decl, $.field_decl, $.record_field, ';', ',', $.comment, $.doc_comment)), '}'),
    field_decl: $ => prec(2, seq('var', optional(field('modifiers', $.decl_modifiers)), $.typed_binding)),
    record_field: $ => prec.right(seq($.typed_binding, optional(seq('=', field('default', $.expr))))),
    standard_block: $ => seq(
      '{',
      repeat(choice(
        $.standard_requirement,
        $.standard_field_requirement,
        ';',
        ',',
        $.comment,
        $.doc_comment,
      )),
      '}',
    ),
    standard_requirement: $ => seq(
      choice('fun', 'pro', 'log'),
      optional(field('modifiers', $.decl_modifiers)),
      field('name', $.identifier),
      $.params,
      optional($.return_type),
      optional($.error_type),
      // Optional default body for protocol / extended standards.
      optional(seq('=', $.block)),
      ';',
    ),
    standard_field_requirement: $ => seq(
      'var',
      optional(field('modifiers', $.decl_modifiers)),
      field('name', $.identifier),
      ':',
      field('type', $.type_expr),
      ';',
    ),
    stmt: $ => choice(
      $.var_decl,
      $.block,
      $.assignment_stmt,
      $.return_stmt,
      $.yield_stmt,
      $.dfr_stmt,
      $.edf_stmt,
      $.report_stmt,
      $.panic_stmt,
      $.assert_stmt,
      $.unreachable_stmt,
      $.break_stmt,
      $.if_stmt,
      $.select_stmt,
      $.while_stmt,
      $.for_stmt,
      $.each_stmt,
      $.when_expr,
      $.loop_expr,
      $.expr,
    ),

    assignment_stmt: $ => prec(1, seq(
      field('target', choice($.identifier, $.qualified_path, $.field_access, $.index_access, $.deref_target)),
      '=',
      field('value', $.expr),
    )),
    deref_target: $ => prec.right(4, seq('[', 'drf', ']', field('pointer', $.expr_atom))),
    return_stmt: $ => prec.right(seq('return', optional($.expr))),
    yield_stmt: $ => prec.right(seq('yield', optional($.expr))),
    dfr_stmt: $ => seq('dfr', optional($.routine_capture_list), $.block),
    edf_stmt: $ => seq('edf', optional($.routine_capture_list), $.block),
    report_stmt: $ => prec.right(seq('report', $.expr)),
    panic_stmt: $ => prec.right(seq('panic', $.expr)),
    assert_stmt: $ => prec.right(seq('assert', optional($.expr))),
    unreachable_stmt: _ => 'unreachable',
    break_stmt: $ => prec.right(seq('break', optional($.expr))),
    if_stmt: $ => prec.right(seq('if', '(', $.expr, ')', $.flow_body, optional($.else_clause))),
    else_clause: $ => seq('else', $.flow_body),
    select_stmt: $ => seq('select', $.select_block),
    select_block: $ => seq(
      '{',
      repeat(choice($.select_arm, $.select_default_arm, $.comment, $.doc_comment)),
      '}',
    ),
    select_arm: $ => seq(
      'when',
      field('channel', choice($.channel_access, $.identifier, $.qualified_path)),
      'as',
      field('binding', $.identifier),
      $.block,
    ),
    select_default_arm: $ => seq('*', $.block),
    while_stmt: $ => seq('while', '(', $.expr, ')', $.flow_body),
    for_stmt: $ => seq('for', '(', field('header', $.loop_header), ')', $.flow_body),
    each_stmt: $ => seq('each', '(', field('header', $.loop_header), ')', $.flow_body),
    loop_header: $ => choice(
      $.iteration_header,
      field('condition', $.expr),
    ),
    iteration_header: $ => prec(3, seq(
      optional(field('declaration', $.iteration_binder_declaration)),
      field('binding', choice($.identifier, $.string_literal, $.char_literal)),
      'in',
      field('iterable', $.expr),
      optional(seq('when', field('condition', $.expr))),
    )),
    iteration_binder_declaration: $ => seq(
      'var',
      field('name', choice($.identifier, $.string_literal, $.char_literal)),
      ':',
      field('type', $.type_expr),
      ';',
    ),
    when_expr: $ => seq('when', '(', $.expr, ')', $.when_block),
    loop_expr: $ => seq('loop', optional(seq('(', $.expr, ')')), $.block),
    when_block: $ => seq('{', repeat(choice(
      $.case_clause,
      $.is_clause,
      $.in_clause,
      $.has_clause,
      $.of_clause,
      $.on_clause,
      $.default_clause,
      $.comment,
      $.doc_comment,
    )), '}'),
    case_clause: $ => seq('case', '(', $.expr, ')', $.block),
    is_clause: $ => seq('is', '(', $.expr, ')', $.block),
    in_clause: $ => seq('in', '(', $.expr, ')', $.block),
    has_clause: $ => seq('has', '(', $.expr, ')', $.block),
    of_clause: $ => seq('of', '(', $.expr, ')', $.block),
    // `on (binding) { ... }` binds the present/error shell payload.
    on_clause: $ => seq('on', '(', $.expr, ')', $.block),
    default_clause: $ => seq('*', $.block),
    flow_body: $ => choice(prec(1, $.block), prec.right(1, seq('=>', choice(prec(1, $.block), $.stmt)))),
    expr: $ => choice(
      $.pipe_or_expr,
      $.pipe_expr,
      $.binary_expr,
      $.unary_expr,
      $.ownership_op,
      $.if_expr,
      $.when_expr,
      $.loop_expr,
      $.range_expr,
      $.spawn_expr,
      $.anonymous_fun_expr,
      $.anonymous_pro_expr,
      $.anonymous_log_expr,
      $.expr_atom,
    ),

    pipe_or_expr: $ => prec.left(1, seq(field('left', choice($.expr_atom, $.unary_expr, $.ownership_op)), '||', field('right', $.expr))),
    pipe_expr: $ => prec.left(1, seq(field('left', choice($.expr_atom, $.unary_expr, $.ownership_op)), '|', field('right', $.expr))),
    binary_expr: $ => prec.left(2, seq(field('left', choice($.expr_atom, $.unary_expr, $.ownership_op)), field('operator', choice(
      '==', '!=', '<=', '>=', '<', '>', '&&', '+', '-', '*', '/', '%', '^',
      'or', 'xor', 'nor', 'and', 'nand', 'as', 'cast', 'is', 'has', 'in', 'on', 'of', 'at'
    )), field('right', $.expr))),
    unary_expr: $ => prec.right(3, seq(field('operator', choice('not', '-')), field('operand', $.expr_atom))),

    // V3 prefix bracket operations: canonical ownership options ([mov]/[cpy]/
    // [cln]/[bor]/[mut, bor]/[new, ...]/[weak]/[upg]/[fin] plus their readable
    // aliases) and the standalone bracket unary operators ([uwp]/[drf]/[ref]/
    // [end]). The operand is a primary expression; `[op]receiver.method()`
    // rebases over the method receiver in the compiler, which this flat shape
    // approximates.
    ownership_op: $ => prec.right(3, seq(
      '[',
      commaSep1(field('option', $.ownership_option)),
      ']',
      field('operand', choice($.expr_atom, $.ownership_op)),
    )),
    ownership_option: _ => choice(
      'mov', 'move', 'cpy', 'copy', 'cln', 'clone', 'bor', 'borrow',
      'mut', 'new', 'weak', 'upg', 'fin', 'uwp', 'drf', 'ref', 'end',
    ),
    expr_atom: $ => choice(
      $.check_expr,
      $.call_expr,
      $.dot_intrinsic,
      $.field_access,
      $.channel_access,
      $.index_access,
      $.qualified_path,
      $.identifier,
      $.this_expr,
      $.self_expr,
      $.where_expr,
      $.get_expr,
      $.async_expr,
      $.await_expr,
      $.do_expr,
      $.if_expr,
      $.range_expr,
      $.anonymous_fun_expr,
      $.anonymous_pro_expr,
      $.anonymous_log_expr,
      $.paren_expr,
      $.record_literal,
      $.container_literal,
      $.string_literal,
      $.raw_string_literal,
      $.char_literal,
      $.integer_literal,
      $.boolean_literal,
      $.nil_literal,
    ),

    if_expr: $ => prec.right(1, seq('if', '(', $.expr, ')', field('then', $.flow_body), 'else', field('else', $.flow_body))),
    range_expr: $ => choice(
      prec.right(seq(field('start', choice($.expr_atom, $.unary_expr)), field('operator', choice('..', '...')), field('end', $.expr))),
      prec.right(seq(field('operator', choice('..', '...')), field('end', $.expr))),
      prec.right(seq(field('start', choice($.expr_atom, $.unary_expr)), field('operator', choice('..', '...')))),
    ),
    // `[spn]call` scoped spawn, `[spn, det]call` detached spawn, `[>]call`
    // scoped-spawn shorthand.
    spawn_expr: $ => prec.right(4, seq(
      choice('[>]', seq('[', 'spn', optional(seq(',', 'det')), ']')),
      field('task', $.expr_atom),
    )),
    anonymous_fun_expr: $ => seq('fun', optional($.decl_modifiers), $.params, optional($.routine_capture_list), optional($.return_type), optional($.error_type), choice('=', '=>'), $.routine_body_expr),
    anonymous_pro_expr: $ => seq('pro', optional($.decl_modifiers), $.params, optional($.routine_capture_list), optional($.return_type), optional($.error_type), choice('=', '=>'), $.routine_body_expr),
    anonymous_log_expr: $ => seq('log', optional($.decl_modifiers), $.params, optional($.routine_capture_list), optional($.return_type), optional($.error_type), choice('=', '=>'), $.routine_body_expr),
    routine_capture_list: $ => seq('[', optional(commaSep($.routine_capture)), ']'),
    routine_capture: $ => seq(
      field('binding', $.identifier),
      optional(seq(
        '[',
        // The composite `[mut, bor]` capture takes a mutable loan; `mut`
        // composes only with `bor` (enforced by the compiler parser).
        optional(seq(field('mutability', 'mut'), ',')),
        field('endpoint', choice(
          'tx', 'rx', 'mov', 'move', 'cpy', 'copy', 'cln', 'clone', 'bor', 'borrow',
        )),
        ']',
      )),
    ),
    routine_body_expr: $ => choice(prec(1, $.block), prec.right(1, seq('=>', choice(prec(1, $.block), $.stmt)))),
    call_expr: $ => prec.left(3, seq(
      field('callee', choice($.qualified_path, $.identifier, $.field_access)),
      // Optional turbofish `::[T, U]` that selects explicit generic
      // type arguments before the call argument list.
      optional(field('type_args', $.turbofish_type_args)),
      optional('$'),
      '(',
      optional(commaSep($.call_arg)),
      ')',
    )),
    call_arg: $ => choice(
      $.named_call_arg,
      $.spread_arg,
      $.expr,
    ),
    named_call_arg: $ => seq(field('name', $.identifier), '=', field('value', $.expr)),
    spread_arg: $ => prec(1, seq('...', $.expr)),
    turbofish_type_args: $ => seq(token('::['), commaSep1($.type_expr), ']'),
    check_expr: $ => seq('check', '(', $.expr, ')'),
    dot_intrinsic: $ => seq('.', field('name', $.identifier), '(', optional(commaSep($.expr)), ')'),
    field_access: $ => prec.left(4, seq(
      field('receiver', choice($.identifier, $.qualified_path, $.field_access, $.self_expr, $.channel_access, $.index_access, $.call_expr, $.paren_expr)),
      '.',
      field('field', $.identifier),
    )),
    channel_access: $ => prec.left(5, seq(
      field('channel', choice($.identifier, $.qualified_path, $.field_access)),
      '[',
      field('endpoint', choice('tx', 'rx')),
      ']',
    )),
    index_access: $ => prec.left(4, seq(
      field('container', choice($.identifier, $.qualified_path, $.field_access, $.channel_access, $.index_access)),
      '[',
      // Empty access `container[]` is the V3 uniform inner-place access
      // (pointer pointee, opt payload, err payload).
      optional(field('index', $.expr)),
      ']',
    )),
    record_literal: $ => seq('{', optional(commaSep($.field_init)), '}'),
    field_init: $ => seq(field('name', $.identifier), '=', field('value', $.expr)),
    container_literal: $ => seq('{', optional(commaSep($.expr)), '}'),

    paren_expr: $ => seq('(', $.expr, ')'),
    qualified_path: $ => prec.left(seq(field('root', $.identifier), repeat1(seq('::', field('segment', $.identifier))))),
    this_expr: _ => 'this',
    self_expr: _ => 'self',
    where_expr: _ => 'where',
    get_expr: _ => 'get',
    async_expr: _ => 'async',
    await_expr: _ => 'await',
    do_expr: _ => 'do',
    identifier: _ => /[A-Za-z_][A-Za-z0-9_]*/,
    integer_literal: _ => /[0-9]+/,
    // Single quotes are the compiler's raw-quoted family: one Unicode scalar
    // lowers as a character, while empty/two-or-more scalars lower as a raw
    // string. Backslashes have no escape meaning in this family.
    char_literal: _ => /'[^']'/,
    raw_string_literal: _ => token(choice(/''/, /'[^'][^']+'/)),
    string_literal: _ => /"([^"\\]|\\.)*"/,
    boolean_literal: _ => choice('true', 'false'),
    nil_literal: _ => 'nil',
    // These lexical boundaries intentionally mirror fol-lexer's non-nested
    // comment forms. Negated classes include newlines, so backtick and slash
    // block comments may span lines. A higher lexical precedence keeps the
    // exact `[doc]` prefix distinct from an ordinary backtick comment.
    comment: _ => token(choice(
      /`[^`]*`/,
      /\/\/[^\n]*/,
      /\/\*[^*]*\*+([^/*][^*]*\*+)*\//,
    )),
    doc_comment: _ => token(prec(1, /`\[doc\][^`]*`/)),
  }
});

function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)));
}

function commaSep(rule) {
  return seq(rule, repeat(seq(',', rule)), optional(','));
}
