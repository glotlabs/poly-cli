use http::{request, HeaderMap, HeaderValue, Request, Response};
use mime_guess::Mime;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

const HTTP1_1: &[u8] = b"HTTP/1.1 200 OK";
const CRNL: &[u8] = b"\r\n";

pub struct Config {
    pub static_base_path: PathBuf,
}

#[derive(Debug)]
pub enum Error {
    Bind(std::io::Error),
}

pub fn start(config: &Config) -> Result<(), Error> {
    let port = listen_port_from_str(&config.static_base_path.to_string_lossy());
    let addr = format!("127.0.0.1:{}", port);

    println!("Listening on {}", addr);
    let listener = TcpListener::bind(&addr).map_err(Error::Bind)?;

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        match handle_connection(config, stream) {
            Ok(_) => {}
            Err(err) => eprintln!("Error: {}", err),
        };
    }

    Ok(())
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
    let body = prepare_response_body(config, req)?;

    let res_builder = Response::builder()
        .status(200)
        .header("Content-Type", body.content_type.to_string());

    let res_builder2 = headers.iter().fold(res_builder, |builder, (name, value)| {
        builder.header(name, value)
    });

    let response = res_builder2.body(body.content).unwrap();

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

pub struct Body {
    content: Vec<u8>,
    content_type: Mime,
}

fn prepare_response_body(config: &Config, req: &Request<()>) -> Result<Body, String> {
    let file_path = file_path_from_req(config, req)?;

    if file_path.exists() {
        let content =
            fs::read(&file_path).map_err(|err| format!("Failed to read file: {}", err))?;
        let content_type = mime_guess::from_path(&file_path)
            .first()
            .unwrap_or_else(|| mime_guess::mime::APPLICATION_OCTET_STREAM);
        Ok(Body {
            content,
            content_type,
        })
    } else if file_path.ends_with("favicon.ico") {
        let content_type = mime_guess::from_ext("ico")
            .first()
            .unwrap_or_else(|| mime_guess::mime::APPLICATION_OCTET_STREAM);

        Ok(Body {
            content: favicon(),
            content_type,
        })
    } else {
        Err(format!("Path not found: {}", file_path.to_string_lossy()))
    }
}

fn file_path_from_req(config: &Config, req: &Request<()>) -> Result<PathBuf, String> {
    let req_path = req.uri().path().trim_start_matches("/");
    let abs_path = config.static_base_path.join(&req_path);

    if Path::new(&abs_path).is_dir() {
        Ok(Path::new(&abs_path).join("index.html"))
    } else {
        Ok(abs_path)
    }
}

fn listen_port_from_str(s: &str) -> u32 {
    let n = s
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .fold(0, |sum, c| {
            // fmt
            sum + c.to_digit(36).unwrap_or_default()
        });

    8000 + (n % 1000)
}

fn favicon() -> Vec<u8> {
    let encoded = "AAABAAEAEBAQAAEABAAoAQAAFgAAACgAAAAQAAAAIAAAAAEABAAAAAAAgAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD//wAA//8AAP//AAD//wAA//8AAP//AAD//wAA//8AAP//AAD//wAA//8AAP//AAD//wAA//8AAP//AAD//wAA";
    base64::decode(&encoded).unwrap()
}
