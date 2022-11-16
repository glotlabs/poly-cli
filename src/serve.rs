use http::{request, HeaderMap, HeaderValue, Request, Response};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

const HTTP1_1: &[u8] = b"HTTP/1.1 200 OK";
const CRNL: &[u8] = b"\r\n";

pub struct Config {
    pub static_base_path: PathBuf,
}

pub fn start(config: &Config) {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        match handle_connection(config, stream) {
            Ok(_) => {}
            Err(err) => eprintln!("Error: {}", err),
        };
    }
}

fn handle_connection(config: &Config, mut stream: TcpStream) -> Result<(), String> {
    let req = read_request(&mut stream)?;
    log_request(&req);
    let res = prepare_response(config, &req, &HeaderMap::new())?;
    write_response(stream, res)?;
    Ok(())
}

fn log_request(req: &Request<()>) {
    println!("[{}] {}", req.method(), req.uri().path());
}

fn write_response(mut stream: TcpStream, res: Response<Vec<u8>>) -> Result<(), String> {
    let body = res.body();
    let length = body.len();

    write(&mut stream, HTTP1_1)?;
    write(&mut stream, CRNL)?;

    write(
        &mut stream,
        format!("Content-Length: {}", length).as_bytes(),
    )?;
    write(&mut stream, CRNL)?;

    for (name, value) in res.headers() {
        write(&mut stream, format!("{}: ", name).as_bytes())?;
        write(&mut stream, value.as_bytes())?;
        write(&mut stream, CRNL)?;
    }

    write(&mut stream, CRNL)?;

    stream
        .write_all(body)
        .map_err(|err| format!("Failed to write body: {}", err))?;

    Ok(())
}

fn write(stream: &mut TcpStream, data: &[u8]) -> Result<(), String> {
    stream
        .write_all(data)
        .map_err(|err| format!("Failed to write response: {}", err))
}

fn prepare_response(
    config: &Config,
    req: &Request<()>,
    headers: &HeaderMap<HeaderValue>,
) -> Result<Response<Vec<u8>>, String> {
    let file_path = file_path_from_req(config, req)?;
    let body = fs::read(&file_path).map_err(|err| format!("Failed to read file: {}", err))?;
    let content_type = mime_guess::from_path(&file_path)
        .first()
        .unwrap_or_else(|| mime_guess::mime::APPLICATION_OCTET_STREAM);

    let res_builder = Response::builder()
        .status(200)
        .header("Content-Type", content_type.to_string());

    let res_builder2 = headers.iter().fold(res_builder, |builder, (name, value)| {
        builder.header(name, value)
    });

    let response = res_builder2.body(body).unwrap();

    Ok(response)
}

fn read_request(stream: &mut TcpStream) -> Result<Request<()>, String> {
    let mut req_reader = BufReader::new(stream);
    let mut buffer = Vec::new();

    // Read until start of body
    loop {
        req_reader
            .read_until(b'\n', &mut buffer)
            .map_err(|err| format!("Failed to read request: {:?}", err))?;
        if buffer.ends_with(&vec![b'\r', b'\n', b'\r', b'\n']) {
            break;
        }
    }

    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);
    req.parse(&mut buffer).unwrap();

    let req = request::Builder::new()
        .method(req.method.unwrap_or_else(|| "GET"))
        .uri(req.path.unwrap_or_else(|| "/"))
        .body(())
        .unwrap();

    Ok(req)
}

fn file_path_from_req(config: &Config, req: &Request<()>) -> Result<PathBuf, String> {
    let req_path = req.uri().path().trim_start_matches("/");
    let abs_path = config.static_base_path.join(&req_path);

    let file_path = if Path::new(&abs_path).is_dir() {
        Path::new(&abs_path).join("index.html")
    } else {
        abs_path
    };

    if file_path.exists() {
        Ok(file_path)
    } else {
        return Err(format!("Path not found: {}", file_path.to_string_lossy()));
    }
}
