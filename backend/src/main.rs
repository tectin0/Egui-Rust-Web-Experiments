use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
};

use anyhow::{Context, Result};

use http::{
    header::{self, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN},
    HeaderValue,
};
use log::{debug, info, trace};
use shared::{
    config::{Config, CONFIG},
    ClientID, Flag, Peer, SendLines,
};
use simple_logger::SimpleLogger;

struct Client {
    id: ClientID,
}

struct State {
    lines: SendLines,
    clients: HashMap<String, Client>,
    clear_sync: Option<HashMap<ClientID, bool>>,
}

impl State {
    fn new() -> Self {
        Self {
            lines: SendLines {
                lines: HashMap::new(),
                flag: Flag::None,
            },
            clients: HashMap::new(),
            clear_sync: None,
        }
    }
}

const VALID_POST_PATHS: [&str; 2] = ["/send_lines", "/hello"];

fn main() {
    let config = CONFIG.read().unwrap();

    SimpleLogger::new()
        .init()
        .context("Failed to initialize logger")
        .unwrap();

    log::set_max_level(log::LevelFilter::Debug);

    let mut state = State::new();

    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.host.port)).unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        match handle_connection(stream, &mut state, &config).context("Failed to handle connection")
        {
            Ok(_) => (),
            Err(e) => println!("Error: {:?}", e),
        };
    }
}

fn handle_connection(
    mut stream: std::net::TcpStream,
    state: &mut State,
    config: &Config,
) -> Result<()> {
    let buf_reader = BufReader::new(&mut stream);

    let mut empty_line_counter = 0;

    let mut is_post = false;

    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| {
            if line.starts_with("POST") && VALID_POST_PATHS.iter().any(|path| line.contains(path)) {
                is_post = true;
            }

            if line.is_empty() {
                empty_line_counter += 1;
            }

            match is_post {
                true => empty_line_counter < 2,
                false => empty_line_counter < 1,
            }
        })
        .collect();

    if http_request.is_empty() {
        return Ok(());
    }

    let mut http_request_iter = http_request.iter();

    let request_line = http_request_iter.next().unwrap();
    let _ = http_request_iter.next().unwrap();

    let mut content: Option<&String> = None;

    if is_post {
        content = match http_request_iter.clone().last() {
            Some(content) => Some(content),
            None => {
                return Err(anyhow::anyhow!(
                    "Failed to get content from request: {:?}",
                    http_request
                ))?
            }
        };
    }

    let peer = Peer(stream.peer_addr().unwrap().to_string());

    let client_id: Option<ClientID> = state.clients.get(peer.ip()?).map(|client| client.id);

    trace!(
        "Request by {} (ID: {}): {:?}",
        peer.ip()?,
        match client_id {
            Some(client_id) => client_id.to_string(),
            None => "Unknown".to_string(),
        },
        request_line
    );

    let mut status_line = None;
    let mut filename = None;
    let mut content_type = None;

    let mut replace_content: Vec<[String; 2]> = Vec::new();

    match request_line.as_str() {
        "GET / HTTP/1.1" => {
            let peer_ip = peer.ip()?;

            let host = match peer_ip {
                "127.0.0.1" => format!("127.0.0.1:{}", config.host.port),
                _ => format!("{}:{}", config.host.ip, config.host.port),
            };

            replace_content.push(["#host".to_string(), host]);

            status_line = Some("HTTP/1.1 200 OK");
            filename = Some("public/index.html");
            content_type = Some("text/html");
        }
        "GET /wasm/frontend.js HTTP/1.1" => {
            status_line = Some("HTTP/1.1 200 OK");
            filename = Some("public/wasm/frontend.js");
            content_type = Some("text/javascript");
        }
        "GET /wasm/frontend_bg.wasm HTTP/1.1" => {
            status_line = Some("HTTP/1.1 200 OK");
            filename = Some("public/wasm/frontend_bg.wasm");
            content_type = Some("application/wasm");
        }
        "POST /hello HTTP/1.1" => {
            let content = match content {
                Some(content) => content,
                None => {
                    return Err(anyhow::anyhow!(
                        "Did not get any content from request: {:?}",
                        http_request
                    ))?
                }
            };

            let client_id = serde_json::from_str::<ClientID>(content)
                .context(format!("Failed to parse client id - content: {}", content))?;

            info!("Client {} connected from {}", client_id.0, peer.ip()?);

            state
                .clients
                .insert(peer.ip()?.to_string(), Client { id: client_id });
        }
        "POST /send_lines HTTP/1.1" => {
            let content = match content {
                Some(content) => content,
                None => {
                    return Err(anyhow::anyhow!(
                        "Did not get any content from request: {:?}",
                        http_request
                    ))?
                }
            };

            let lines = serde_json::from_str::<SendLines>(content).unwrap();

            debug!("Received lines: {:?}", lines.lines.keys());
            debug!("Current lines: {:?}", state.lines.lines.keys());

            state.lines.merge(lines);
        }
        "GET /get_lines HTTP/1.1" => {
            if state.clear_sync.is_some() {
                let clear_sync = state.clear_sync.as_mut().unwrap();

                clear_sync.remove(&client_id.unwrap());

                if clear_sync.is_empty() {
                    state.lines.flag = Flag::None;

                    state.clear_sync = None;
                }
            }

            let lines = state.lines.clone();

            let response = serde_json::to_string(&lines).unwrap() + "\r\n\r\n";

            stream
                .write_all(response.as_bytes())
                .context("Failed to write response")?;
        }
        "POST /clear_lines HTTP/1.1" => {
            state.lines = SendLines {
                lines: HashMap::new(),
                flag: Flag::Clear,
            };

            state.clear_sync = Some(
                state
                    .clients.values().map(|client| (client.id, false))
                    .collect(),
            );
        }
        _ => {
            status_line = Some("HTTP/1.1 404 NOT FOUND");
            filename = Some("public/404.html");
            content_type = Some("text/html");
        }
    };

    if status_line.is_none() || filename.is_none() || content_type.is_none() {
        return Ok(());
    }

    let status_line = status_line.unwrap();
    let filename = filename.unwrap();
    let content_type = content_type.unwrap();

    let contents = match content_type {
        "text/html" => {
            let mut string = fs::read_to_string(filename)?;
            for replace in replace_content {
                string = string.replace(&replace[0], &replace[1]);
            }
            string.into_bytes()
        }
        "text/javascript" => fs::read_to_string(filename)?.into_bytes(),
        "application/wasm" => fs::read(filename)?,
        _ => Vec::<u8>::new(),
    };

    let length = contents.len();

    let mut headermap = http::HeaderMap::new();

    headermap.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    headermap.insert(
        ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Origin, X-Requested-With, Content-Type, Accept"),
    );
    headermap.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headermap.insert(header::CONTENT_LENGTH, HeaderValue::from(length));

    let response = format!("{}\r\n{}\r\n", status_line, {
        headermap
            .iter()
            .fold(String::new(), |mut acc, (key, value)| {
                acc.push_str(&format!("{}: {}\r\n", key, value.to_str().unwrap()));
                acc
            })
    });

    let mut response = response.into_bytes();
    response.extend(contents);

    stream.write_all(response.as_slice()).unwrap();

    Ok(())
}
