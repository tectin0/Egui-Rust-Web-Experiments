use eframe::CreationContext;
use egui::{
    emath, pos2, Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions,
};

use reqwest::Client as ReqwestClient;

use wasm_bindgen_futures::spawn_local;

use std::sync::{Arc, Mutex};

use shared::SPos2;
use shared::SendLines;

use anyhow::Result;

use crate::HOST;

use async_recursion::async_recursion;

pub struct App {
    is_dark: bool,
    background: TextureHandle,
    send_lines: Arc<Mutex<SendLines>>,
    get_lines_timer: Option<f64>,
    stroke: Stroke,
}

impl App {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        spawn_local(async move {
            match send_hello_request().await {
                Ok(_) => (),
                Err(e) => println!("Error: {:?}", e),
            };
        });

        Self {
            is_dark: true,
            background: cc.egui_ctx.load_texture(
                "background",
                load_image_from_memory(include_bytes!("..\\example.png")).unwrap(),
                TextureOptions::default(),
            ),
            send_lines: Default::default(),
            get_lines_timer: None,
            stroke: Stroke::new(5.0, Color32::WHITE),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.toggle_value(&mut self.is_dark, "🌓").changed().then(|| {
                if self.is_dark {
                    ui.ctx().set_visuals(egui::Visuals::dark());
                } else {
                    ui.ctx().set_visuals(egui::Visuals::light());
                }
            });

            egui::stroke_ui(ui, &mut self.stroke, "Stroke");

            let canvas_size = ui.available_size_before_wrap();
            let canvas_rect = ui.available_rect_before_wrap();

            let (mut response, painter) = ui.allocate_painter(canvas_size, Sense::drag());

            painter.image(
                self.background.id(),
                canvas_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );

            let to_screen = emath::RectTransform::from_to(
                Rect::from_min_size(Pos2::ZERO, response.rect.square_proportions()),
                response.rect,
            );
            let from_screen = to_screen.inverse();

            let mut send_lines = self
                .send_lines
                .try_lock()
                .expect(&format!("Failed to lock send_lines at line {}", line!()));

            if send_lines.lines.is_empty() {
                send_lines.lines.push(vec![]);
                send_lines.line_ids.push(rand::random::<usize>());
            }

            let current_line = send_lines.lines.last_mut().unwrap();

            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let canvas_pos = from_screen * pointer_pos;
                if current_line.last() != Some(&SPos2(canvas_pos)) {
                    current_line.push(SPos2(canvas_pos));
                    response.mark_changed();
                }

                drop(send_lines);
            } else if !current_line.is_empty() {
                drop(send_lines);

                let send_lines = {
                    let unlocked = self
                        .send_lines
                        .try_lock()
                        .expect(&format!("Failed to lock send_lines at line {}", line!()));
                    unlocked.clone()
                };

                spawn_local(async move {
                    match send_lines_request(send_lines).await {
                        Ok(_) => (),
                        Err(e) => println!("Error: {:?} at Line: {}", e, line!()),
                    };
                });

                {
                    let mut send_lines = self
                        .send_lines
                        .try_lock()
                        .expect(&format!("Failed to lock send_lines at line {}", line!()));

                    send_lines.lines.push(vec![]);
                    send_lines.line_ids.push(rand::random::<usize>());
                }

                response.mark_changed();
            } else {
                drop(send_lines);
            }

            let send_lines = {
                let unlocked = self
                    .send_lines
                    .try_lock()
                    .expect(&format!("Failed to lock send_lines at line {}", line!()));
                unlocked.clone()
            };

            let shapes = send_lines
                .lines
                .iter()
                .filter(|line| line.len() >= 2)
                .map(|line| {
                    let points: Vec<Pos2> = line.iter().map(|p| to_screen * **p).collect();
                    egui::Shape::line(points, self.stroke)
                });

            painter.extend(shapes);
        });

        let seconds_since = chrono::offset::Local::now().timestamp_millis() as f64 / 1000.0;

        if self.get_lines_timer.is_none() {
            self.get_lines_timer = Some(seconds_since);
        }

        if seconds_since - self.get_lines_timer.unwrap() > 1.0 {
            let send_lines = self.send_lines.clone();

            spawn_local(async move {
                let get_lines = match get_lines_request().await {
                    Ok(get_lines) => get_lines,
                    Err(e) => {
                        println!("Error: {:?} at Line: {}", e, line!());
                        return;
                    }
                };

                let mut send_lines = send_lines
                    .try_lock()
                    .expect(&format!("Failed to lock send_lines at line {}", line!()));

                send_lines.merge(get_lines);
            });

            self.get_lines_timer = Some(seconds_since);
        }
    }
}

fn load_image_from_memory(image_data: &[u8]) -> Result<ColorImage, image::ImageError> {
    let image = image::load_from_memory(image_data)?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()))
}

#[async_recursion(?Send)]
async fn send_hello_request() -> Result<()> {
    let client = ReqwestClient::new();

    let body = r#"{ "hello": "world" }\r\n"#;

    match client
        .post(HOST.to_string() + "/hello")
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err(anyhow::anyhow!("Failed to send hello request")),
    }
}
#[async_recursion(?Send)]
async fn send_lines_request(send_lines: SendLines) -> Result<()> {
    let client = ReqwestClient::new();

    let body = serde_json::to_string(&send_lines).unwrap() + "\r\n\r\n";

    match client
        .post(HOST.to_string() + "/send_lines")
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err(anyhow::anyhow!("Failed to send lines request")),
    }
}

#[async_recursion(?Send)]
async fn get_lines_request() -> Result<SendLines> {
    let client = ReqwestClient::new();

    let response = client
        .get(HOST.to_string() + "/get_lines")
        .send()
        .await
        .unwrap();

    let body = response.text().await.unwrap();

    Ok(serde_json::from_str::<SendLines>(&body).unwrap())
}
