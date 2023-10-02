use eframe::CreationContext;
use egui::{
    emath, pos2, Color32, ColorImage, Image, ImageData, ImageSource, Pos2, Rect, Sense, Stroke,
    TextureHandle, TextureOptions,
};

pub struct App {
    is_dark: bool,
    background: TextureHandle,
    lines: Vec<Vec<Pos2>>,
    stroke: Stroke,
}

impl App {
    pub fn new(cc: &CreationContext) -> Self {
        Self {
            is_dark: true,
            background: cc.egui_ctx.load_texture(
                "background",
                load_image_from_memory(include_bytes!("..\\example.png")).unwrap(),
                TextureOptions::default(),
            ),
            lines: Default::default(),
            stroke: Stroke::new(1.0, Color32::WHITE),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
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

            if self.lines.is_empty() {
                self.lines.push(vec![]);
            }

            let current_line = self.lines.last_mut().unwrap();

            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let canvas_pos = from_screen * pointer_pos;
                if current_line.last() != Some(&canvas_pos) {
                    current_line.push(canvas_pos);
                    response.mark_changed();
                }
            } else if !current_line.is_empty() {
                self.lines.push(vec![]);
                response.mark_changed();
            }

            let shapes = self
                .lines
                .iter()
                .filter(|line| line.len() >= 2)
                .map(|line| {
                    let points: Vec<Pos2> = line.iter().map(|p| to_screen * *p).collect();
                    egui::Shape::line(points, self.stroke)
                });

            painter.extend(shapes);
        });
    }
}

fn load_image_from_memory(image_data: &[u8]) -> Result<ColorImage, image::ImageError> {
    let image = image::load_from_memory(image_data)?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()))
}
