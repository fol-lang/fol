use super::*;

fn first_parse_error_message(path: &str) -> String {
    let mut file_stream = FileStream::from_file(path).expect("Should read parser fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail for malformed type-reference fixture");

    errors
        .first()
        .expect("First parser error should be ParseError")
        .message
        .clone()
}

#[test]
fn test_any_none_never_missing_close_report_type_reference_close() {
    for path in [
        "test/parser/simple_any_type_missing_close.fol",
        "test/parser/simple_none_type_missing_close.fol",
        "test/parser/simple_never_type_missing_close.fol",
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains("Expected closing ']' in type reference"),
            "Expected normalized missing-close diagnostic for fixture {path}, got: {message}",
        );
    }
}

#[test]
fn test_scalar_type_missing_close_report_type_reference_close() {
    for path in [
        "test/parser/simple_int_type_missing_close.fol",
        "test/parser/simple_float_type_missing_close.fol",
        "test/parser/simple_char_type_missing_close.fol",
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains("Expected closing ']' in type reference"),
            "Expected normalized missing-close diagnostic for fixture {path}, got: {message}",
        );
    }
}

#[test]
fn test_optional_multiple_union_missing_close_report_type_reference_close() {
    for path in [
        "test/parser/simple_opt_type_missing_close.fol",
        "test/parser/simple_mul_type_missing_close.fol",
        "test/parser/simple_uni_type_missing_close.fol",
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains("Expected closing ']' in type reference"),
            "Expected normalized missing-close diagnostic for fixture {path}, got: {message}",
        );
    }
}

#[test]
fn test_pointer_and_error_missing_close_report_type_reference_close() {
    for path in [
        "test/parser/simple_ptr_type_missing_close.fol",
        "test/parser/simple_err_type_missing_close.fol",
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains("Expected closing ']' in type reference"),
            "Expected normalized missing-close diagnostic for fixture {path}, got: {message}",
        );
    }
}

#[test]
fn test_vector_sequence_set_and_map_missing_close_report_type_reference_close() {
    for path in [
        "test/parser/simple_vec_type_missing_close.fol",
        "test/parser/simple_seq_type_missing_close.fol",
        "test/parser/simple_set_type_missing_close.fol",
        "test/parser/simple_map_type_missing_close.fol",
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains("Expected closing ']' in type reference"),
            "Expected normalized missing-close diagnostic for fixture {path}, got: {message}",
        );
    }
}

#[test]
fn test_module_block_test_and_source_kind_missing_close_report_type_reference_close() {
    for path in [
        "test/parser/simple_mod_type_missing_close.fol",
        "test/parser/simple_blk_type_missing_close.fol",
        "test/parser/simple_tst_type_missing_close.fol",
        "test/parser/simple_loc_type_missing_close.fol",
        "test/parser/simple_std_type_missing_close.fol",
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains("Expected closing ']' in type reference"),
            "Expected normalized missing-close diagnostic for fixture {path}, got: {message}",
        );
    }
}

#[test]
fn test_array_and_matrix_missing_close_report_type_reference_close() {
    for path in [
        "test/parser/simple_arr_type_missing_close.fol",
        "test/parser/simple_mat_type_missing_close.fol",
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains("Expected closing ']' in type reference"),
            "Expected normalized missing-close diagnostic for fixture {path}, got: {message}",
        );
    }
}

#[test]
fn test_special_type_bad_separator_diagnostics_stay_shape_specific() {
    for (path, expected) in [
        (
            "test/parser/simple_opt_type_bad_separator.fol",
            "Expected ',', ';', or closing ']' in type reference",
        ),
        (
            "test/parser/simple_tst_type_bad_separator.fol",
            "Expected ',', ';', or ']' in tst[...] arguments",
        ),
    ] {
        let message = first_parse_error_message(path);
        assert!(
            message.contains(expected),
            "Expected shape-specific separator diagnostic for fixture {path}, got: {message}",
        );
    }
}
