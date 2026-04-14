use super::*;

fn first_parse_error_message(path: &str) -> String {
    let mut file_stream = FileStream::from_file(path).expect("Should read parser fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail for diagnostic consistency fixture");

    errors
        .first()
        .expect("First parser error should be ParseError")
        .message
        .clone()
}

#[test]
fn test_parser_owned_unknown_option_diagnostics_name_the_surface() {
    for (path, expected) in [
        ("test/parser/simple_use_unknown_option.fol", "Unknown use option"),
        (
            "test/parser/simple_std_unknown_options.fol",
            "Unknown standard option",
        ),
        ("test/parser/simple_typ_options_unknown.fol", "Unknown type option"),
        (
            "test/parser/simple_routine_options_unknown.fol",
            "Unknown routine option",
        ),
        (
            "test/parser/simple_binding_unknown_option.fol",
            "Unknown binding option",
        ),
        (
            "test/parser/simple_def_unknown_option.fol",
            "Unknown definition option",
        ),
        (
            "test/parser/simple_seg_unknown_option.fol",
            "Unknown segment option",
        ),
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains(expected),
            "Expected explicit parser-owned unknown-option diagnostic for fixture {path}, got: {message}",
        );
    }
}

#[test]
fn test_unsupported_combinations_report_explicit_messages() {
    for (path, expected) in [
        (
            "test/parser/simple_typ_multi_names_with_generics.fol",
            "Type generics and explicit contracts are currently supported only on single-name type declarations",
        ),
        (
            "test/parser/simple_typ_multi_names_with_contracts.fol",
            "Type generics and explicit contracts are currently supported only on single-name type declarations",
        ),
        (
            "test/parser/simple_typ_multi_names_mismatched_defs.fol",
            "Type definition count must match declared names or provide a single shared definition",
        ),
        (
            "test/parser/simple_def_alt_with_params.fol",
            "Definition parameters are currently supported only for mac definitions",
        ),
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains(expected),
            "Expected explicit unsupported-combination diagnostic for fixture {path}, got: {message}",
        );
    }
}

#[test]
fn test_duplicate_and_conflicting_diagnostics_stay_surface_specific() {
    for (path, expected) in [
        (
            "test/parser/simple_use_duplicate_options.fol",
            "Duplicate use option 'export'",
        ),
        (
            "test/parser/simple_use_conflicting_options.fol",
            "Conflicting use options 'export' and 'hidden'",
        ),
        (
            "test/parser/simple_std_conflicting_options.fol",
            "Conflicting standard visibility options",
        ),
        (
            "test/parser/simple_typ_record_duplicate_method.fol",
            "Duplicate type member 'getBrand#0'",
        ),
        (
            "test/parser/simple_std_blueprint_duplicate_field.fol",
            "Duplicate standard member 'color'",
        ),
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains(expected),
            "Expected surface-specific duplicate/conflict diagnostic for fixture {path}, got: {message}",
        );
    }
}

#[test]
fn test_representative_missing_close_diagnostics_keep_shape_specific_wording() {
    for (path, expected) in [
        (
            "test/parser/simple_opt_type_missing_close.fol",
            "Expected closing ']' in type reference",
        ),
        (
            "test/parser/simple_fun_index_assignment_missing_close.fol",
            "Expected closing ']' for index assignment target",
        ),
        (
            "test/parser/simple_pro_method_receiver_missing_close_paren.fol",
            "Expected ')' after method receiver type",
        ),
        (
            "test/parser/simple_call_top_level_unmatched_open_paren_arg.fol",
            "Expected closing ')' for parenthesized expression",
        ),
        (
            "test/parser/simple_fun_function_type_missing_close.fol",
            "Expected '}' to close function type",
        ),
        (
            "test/parser/simple_fun_nested_block_missing_close.fol",
            "Expected '}' to close block",
        ),
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains(expected),
            "Expected stable missing-close diagnostic for fixture {path}, got: {message}",
        );
    }
}

#[test]
fn test_representative_expected_x_diagnostics_name_the_missing_shape() {
    for (path, expected) in [
        (
            "test/parser/simple_fun_method_receiver_missing_name.fol",
            "Expected function name after 'fun'",
        ),
        (
            "test/parser/simple_pro_method_receiver_missing_name.fol",
            "Expected procedure name after 'pro'",
        ),
        (
            "test/parser/simple_fun_field_assignment_missing_name.fol",
            "Expected field name after '.' in assignment target",
        ),
        (
            "test/parser/simple_routine_grouped_params_missing_colon.fol",
            "Expected ':' after parameter name",
        ),
        (
            "test/parser/simple_routine_default_param_missing_value.fol",
            "Expected default value expression after '=' in parameter",
        ),
        (
            "test/parser/simple_fun_unary_plus_missing_operand.fol",
            "Expected expression after unary '+'",
        ),
        (
            "test/parser/simple_fun_when_star_default_missing_body.fol",
            "Expected '{' after when default '*'",
        ),
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains(expected),
            "Expected representative expected-X diagnostic for fixture {path}, got: {message}",
        );
    }
}
