use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

fn main() {
    std::env::set_current_dir(".\\frontend").unwrap();

    let listener = TcpListener::bind("127.0.0.1:8000").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        match handle_connection(stream).context("Failed to handle connection") {
            Ok(_) => (),
            Err(e) => println!("Error: {:?}", e),
        };
    }
}

fn handle_connection(mut stream: std::net::TcpStream) -> Result<()> {
    let buf_reader = BufReader::new(&mut stream);

    let mut empty_line_counter = 0;

    let mut is_post = false;

    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| {
            if line.contains("POST /send_lines") {
                is_post = true;
                println!("POST request");
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

    let mut http_request_iter = http_request.iter();
    let mut content = http_request_iter.clone().last().unwrap().clone();

    let request_line = http_request_iter.next().unwrap();
    let host = http_request_iter.next().unwrap();

    if &request_line[0..5] == "POST " {
        // content = buf_reader_lines.next().unwrap().unwrap();
    }

    println!("Request: {:?}", request_line);

    let mut status_line = None;
    let mut filename = None;
    let mut content_type = None;

    match request_line.as_str() {
        "GET / HTTP/1.1" => {
            status_line = Some("HTTP/1.1 200 OK");
            filename = Some("index.html");
            content_type = Some("text/html");
        }
        "GET /wasm/egui_rust_web_experiments_lib.js HTTP/1.1" => {
            status_line = Some("HTTP/1.1 200 OK");
            filename = Some("wasm/egui_rust_web_experiments_lib.js");
            content_type = Some("text/javascript");
        }
        "GET /wasm/egui_rust_web_experiments_lib_bg.wasm HTTP/1.1" => {
            status_line = Some("HTTP/1.1 200 OK");
            filename = Some("wasm/egui_rust_web_experiments_lib_bg.wasm");
            content_type = Some("application/wasm");
        }
        "POST /hello HTTP/1.1" => {
            println!("{host} says hello!");
        }
        "POST /send_lines HTTP/1.1" => {
            #[derive(Deserialize, Serialize, Debug)]
            struct SendLines {
                lines: Vec<f32>,
                num_lines: usize,
            }

            let content = serde_json::from_str::<SendLines>(&content).unwrap();

            dbg!(content);
        }
        _ => {
            status_line = Some("HTTP/1.1 404 NOT FOUND");
            filename = Some("404.html");
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
        "text/html" => fs::read_to_string(filename)?.into_bytes(),
        "text/javascript" => fs::read_to_string(filename)?.into_bytes(),
        "application/wasm" => fs::read(filename)?.into(),
        _ => Vec::<u8>::new(),
    };

    let length = contents.len();

    let response = format!(
        "{status_line}\r\nContent-Length: {length}\r\nContent-Type: {content_type}\r\n\r\n"
    );

    let mut response = response.into_bytes();
    response.extend(contents);

    stream.write_all(response.as_slice()).unwrap();

    Ok(())
}
