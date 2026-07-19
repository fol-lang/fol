use super::{EditorLspServer, JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::{EditorConfig, EditorError, EditorErrorKind, EditorResult};
use serde::Serialize;
use std::io::{BufRead, Write};

pub fn run_lsp_stdio(config: EditorConfig) -> EditorResult<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    run_lsp_stdio_with_io(stdin.lock(), stdout.lock(), config)
}

pub(crate) fn run_lsp_stdio_with_io<R: BufRead, W: Write>(
    mut reader: R,
    mut writer: W,
    config: EditorConfig,
) -> EditorResult<()> {
    let mut server = EditorLspServer::new(config);

    while let Some(payload) = read_jsonrpc_payload(&mut reader)? {
        let value: serde_json::Value = serde_json::from_slice(&payload).map_err(|error| {
            EditorError::new(
                EditorErrorKind::InvalidInput,
                format!("failed to decode JSON-RPC payload: {error}"),
            )
        })?;

        if value.get("id").is_some() {
            let request: JsonRpcRequest = serde_json::from_value(value).map_err(|error| {
                EditorError::new(
                    EditorErrorKind::InvalidInput,
                    format!("failed to decode JSON-RPC request: {error}"),
                )
            })?;
            let id = request.id.clone();
            match server.handle_request(request) {
                Ok(Some(response)) => {
                    write_jsonrpc_message(&mut writer, &response)?;
                }
                Ok(None) => {}
                Err(error) => {
                    let response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32603,
                            message: error.message,
                        }),
                    };
                    write_jsonrpc_message(&mut writer, &response)?;
                }
            }
        } else {
            let notification: JsonRpcNotification =
                serde_json::from_value(value).map_err(|error| {
                    EditorError::new(
                        EditorErrorKind::InvalidInput,
                        format!("failed to decode JSON-RPC notification: {error}"),
                    )
                })?;
            let should_exit = notification.method == "exit";
            let diagnostics = server.handle_notification(notification)?;
            for diagnostics in diagnostics {
                write_jsonrpc_message(
                    &mut writer,
                    &JsonRpcNotification {
                        jsonrpc: "2.0".to_string(),
                        method: "textDocument/publishDiagnostics".to_string(),
                        params: Some(
                            serde_json::to_value(diagnostics)
                                .expect("publish diagnostics should serialize"),
                        ),
                    },
                )?;
            }
            if should_exit || server.session.shutdown_requested {
                break;
            }
        }
    }
    Ok(())
}

pub(crate) fn read_jsonrpc_payload(reader: &mut impl BufRead) -> EditorResult<Option<Vec<u8>>> {
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line).map_err(|error| {
            EditorError::new(
                EditorErrorKind::Internal,
                format!("failed to read LSP header line: {error}"),
            )
        })?;
        if read == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            let length = value.trim().parse::<usize>().map_err(|error| {
                EditorError::new(
                    EditorErrorKind::InvalidInput,
                    format!("invalid Content-Length header: {error}"),
                )
            })?;
            content_length = Some(length);
        }
    }

    let content_length = content_length.ok_or_else(|| {
        EditorError::new(
            EditorErrorKind::InvalidInput,
            "missing Content-Length header in LSP message",
        )
    })?;

    let mut payload = vec![0; content_length];
    reader.read_exact(&mut payload).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!("failed to read LSP payload: {error}"),
        )
    })?;
    Ok(Some(payload))
}

pub(crate) fn write_jsonrpc_message(
    writer: &mut impl Write,
    value: &impl Serialize,
) -> EditorResult<()> {
    let body = serde_json::to_vec(value).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!("failed to encode JSON-RPC payload: {error}"),
        )
    })?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len()).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!("failed to write LSP header: {error}"),
        )
    })?;
    writer.write_all(&body).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!("failed to write LSP payload: {error}"),
        )
    })?;
    writer.flush().map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!("failed to flush LSP payload: {error}"),
        )
    })?;
    Ok(())
}

pub(crate) fn from_params<T: serde::de::DeserializeOwned>(
    params: Option<serde_json::Value>,
) -> EditorResult<T> {
    serde_json::from_value(params.unwrap_or(serde_json::Value::Null)).map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidInput,
            format!("invalid LSP params: {error}"),
        )
    })
}
