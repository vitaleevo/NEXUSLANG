use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use crate::ast::Program;
use crate::parse_checked_source;

use super::storage_backend::{default_data_dir, Storage};

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
    pub headers: Vec<(String, String)>,
}

pub fn serve_file(file_path: &str, addr: &str) -> Result<(), String> {
    let source =
        fs::read_to_string(file_path).map_err(|e| format!("Erro ao ler '{}': {}", file_path, e))?;
    let program = parse_checked_source(&source)?;
    let data_dir = default_data_dir(file_path);
    let storage = Storage::new_json(&data_dir);
    storage.ensure_storage(&program)?;

    let listener = TcpListener::bind(addr)
        .map_err(|e| format!("Não foi possível iniciar servidor em {}: {}", addr, e))?;

    println!("NexusLang serve em http://{}", addr);
    println!("OpenAPI em http://{}/openapi.json", addr);
    println!("Storage JSON em {}", data_dir.display());

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(e) = handle_stream(&program, &storage, &mut stream) {
                    eprintln!("Erro HTTP: {}", e);
                }
            }
            Err(e) => eprintln!("Erro de conexão: {}", e),
        }
    }

    Ok(())
}

pub(crate) fn handle_stream(
    program: &Program,
    storage: &Storage,
    stream: &mut TcpStream,
) -> Result<(), String> {
    let mut buffer = [0_u8; 8192];
    let size = stream.read(&mut buffer).map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buffer[..size]);
    let Some(first_line) = request.lines().next() else {
        return Ok(());
    };
    let parts = first_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 {
        return Ok(());
    }

    let headers = request_headers(&request);
    let body = request_body(&request);
    let response = super::router::handle_request_with_headers(
        program, storage, parts[0], parts[1], &headers, body,
    );
    write_response(stream, response)
}

fn request_headers(request: &str) -> Vec<(String, String)> {
    request
        .lines()
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn request_body(request: &str) -> &str {
    request
        .split_once("\r\n\r\n")
        .or_else(|| request.split_once("\n\n"))
        .map(|(_, body)| body)
        .unwrap_or("")
}

pub(crate) fn route_error_status(message: &str) -> u16 {
    if message.starts_with("Nao autorizado") {
        401
    } else if message.starts_with("Proibido") {
        403
    } else if message.starts_with("Requisicao invalida") {
        400
    } else if message.starts_with("Conflito") {
        409
    } else if message.starts_with("Nao encontrado") {
        404
    } else {
        500
    }
}

fn write_response(stream: &mut TcpStream, response: HttpResponse) -> Result<(), String> {
    let status_text = match response.status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let extra_headers = response
        .headers
        .iter()
        .map(|(name, value)| format!("{}: {}\r\n", name, value))
        .collect::<String>();
    let raw = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        response.status,
        status_text,
        response.content_type,
        extra_headers,
        response.body.len(),
        response.body
    );
    stream.write_all(raw.as_bytes()).map_err(|e| e.to_string())
}

pub(crate) fn json_response(status: u16, body: String) -> HttpResponse {
    HttpResponse {
        status,
        content_type: "application/json",
        body,
        headers: Vec::new(),
    }
}

pub(crate) fn method_name(method: &crate::ast::HttpMethod) -> &'static str {
    match method {
        crate::ast::HttpMethod::Get => "GET",
        crate::ast::HttpMethod::Post => "POST",
        crate::ast::HttpMethod::Put => "PUT",
        crate::ast::HttpMethod::Delete => "DELETE",
    }
}
