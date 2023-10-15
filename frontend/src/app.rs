use eframe::CreationContext;

use egui::{
    emath, pos2, Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions,
};

use reqwest::Client as ReqwestClient;

use shared::ClientID;
use shared::Flag;
use wasm_bindgen_futures::spawn_local;

use std::sync::{Arc, Mutex};

use shared::SPos2;
use shared::SendLines;

use anyhow::Result;

use async_recursion::async_recursion;

pub struct App {
    client_id: ClientID,
    host: String,
    is_dark: bool,
    background: TextureHandle,
    send_lines: Arc<Mutex<SendLines>>,
    current_line_id: Option<usize>,
    get_lines_timer: Option<f64>,
    stroke: Stroke,
}

impl App {
    pub fn new(cc: &CreationContext<'_>, host: String) -> Self {
        let client_id = ClientID(rand::random::<usize>());

        let host_clone = host.clone();

        spawn_local(async move {
            match send_hello_request(host_clone, client_id).await {
                Ok(_) => (),
                Err(e) => println!("Error: {:?}", e),
            };
        });

        Self {
            client_id,
            host: host.to_string(),
            is_dark: true,
            background: cc.egui_ctx.load_texture(
                "background",
                load_image_from_memory(include_bytes!("../../assets/example.png")).unwrap(),
                TextureOptions::default(),
            ),
            send_lines: Default::default(),
            current_line_id: None,
            get_lines_timer: None,
            stroke: Stroke::new(5.0, Color32::WHITE),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.toggle_value(&mut self.is_dark, "🌓").changed().then(|| {
                    if self.is_dark {
                        ui.ctx().set_visuals(egui::Visuals::dark());
                    } else {
                        ui.ctx().set_visuals(egui::Visuals::light());
                    }
                });

                egui::stroke_ui(ui, &mut self.stroke, "Stroke");

                let host = self.host.clone();

                if ui.button("Clear").clicked() {
                    spawn_local(async move {
                        match send_clear_lines_request(&host).await {
                            Ok(_) => (),
                            Err(e) => println!("Error sending clear lines request: {:?}", e),
                        };
                    });

                    let mut send_lines = self
                        .send_lines
                        .try_lock()
                        .expect(&format!("Failed to lock send_lines at line {}", line!()));

                    send_lines.lines.clear();
                }
            });

            let canvas_size = ui.available_size_before_wrap();
            let canvas_rect = ui.available_rect_before_wrap();

            let (mut response, painter) = ui.allocate_painter(canvas_size, Sense::drag());

            painter.image(
                self.background.id(),
                canvas_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );

            let background_size = self.background.size();
            let background_rect = Rect::from_min_size(
                Pos2::ZERO,
                emath::vec2(background_size[0] as f32, background_size[1] as f32),
            );

            let to_screen = emath::RectTransform::from_to(background_rect, response.rect);

            let from_screen = to_screen.inverse();

            let mut send_lines = self
                .send_lines
                .try_lock()
                .expect(&format!("Failed to lock send_lines at line {}", line!()));

            if send_lines.lines.is_empty() {
                let current_line_id = rand::random::<usize>();
                self.current_line_id = Some(current_line_id);
                send_lines.lines.insert(current_line_id, vec![]);
            }

            let current_line = match send_lines.lines.get_mut(match &self.current_line_id {
                Some(current_line_id) => current_line_id,
                None => {
                    log::error!("Failed to get current line id");
                    return;
                }
            }) {
                Some(current_line) => current_line,
                None => {
                    log::error!("Failed to get current line");
                    return;
                }
            };

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

                let host = self.host.clone();

                spawn_local(async move {
                    match send_lines_request(&host, send_lines).await {
                        Ok(_) => (),
                        Err(e) => println!("Error: {:?} at Line: {}", e, line!()),
                    };
                });

                let mut send_lines = self
                    .send_lines
                    .try_lock()
                    .expect(&format!("Failed to lock send_lines at line {}", line!()));

                let current_line_id = rand::random::<usize>();

                self.current_line_id = Some(current_line_id);

                send_lines.lines.insert(current_line_id, vec![]);

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
                .filter(|(_line_id, line)| line.len() >= 2)
                .map(|(_line_id, line)| {
                    let points: Vec<Pos2> = line.iter().map(|p| to_screen * **p).collect();
                    egui::Shape::line(points, self.stroke)
                });

            painter.extend(shapes);
        });

        let seconds_since = chrono::offset::Local::now().timestamp_millis() as f64 / 1000.0;

        if self.get_lines_timer.is_none() {
            self.get_lines_timer = Some(seconds_since);
        }

        if seconds_since - self.get_lines_timer.unwrap() > 0.5 {
            let send_lines = self.send_lines.clone();

            let host = self.host.clone();

            spawn_local(async move {
                let get_lines = match get_lines_request(&host).await {
                    Ok(get_lines) => get_lines,
                    Err(e) => {
                        println!("Error: {:?} at Line: {}", e, line!());
                        return;
                    }
                };

                let mut send_lines = send_lines
                    .try_lock()
                    .expect(&format!("Failed to lock send_lines at line {}", line!()));

                match get_lines.flag {
                    Flag::Clear => {
                        send_lines.lines.clear();
                    }
                    Flag::None => {
                        send_lines.merge(get_lines);
                    }
                }
            });

            self.get_lines_timer = Some(seconds_since);
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(500));
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
async fn send_hello_request(host: String, client_id: ClientID) -> Result<()> {
    let client = ReqwestClient::new();

    let body = serde_json::to_string(&client_id).unwrap() + "\r\n\r\n";

    match client
        .post("http://".to_string() + &host + "/hello")
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
async fn send_lines_request(host: &str, send_lines: SendLines) -> Result<()> {
    let client = ReqwestClient::new();

    let body = serde_json::to_string(&send_lines).unwrap() + "\r\n\r\n";

    match client
        .post("http://".to_string() + host + "/send_lines")
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
async fn get_lines_request(host: &str) -> Result<SendLines> {
    let client = ReqwestClient::new();

    let response = client
        .get("http://".to_string() + host + "/get_lines")
        .send()
        .await
        .unwrap();

    let body = response.text().await.unwrap();

    Ok(serde_json::from_str::<SendLines>(&body).unwrap())
}

#[async_recursion(?Send)]
async fn send_clear_lines_request(host: &str) -> Result<()> {
    let client = ReqwestClient::new();

    let body = r#""#;

    match client
        .post("http://".to_string() + host + "/clear_lines")
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err(anyhow::anyhow!("Failed to send clear lines request")),
    }
}
