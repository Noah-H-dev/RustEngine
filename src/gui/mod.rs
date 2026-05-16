use std::sync::Arc;

pub struct UiButton {
    pub id: u64,
    pub label: String,
    pub position: egui::Pos2,
    pub pressed: bool,
    pub marked_for_deletion: bool,
}

impl UiButton {
    pub fn new(id: u64, label: impl Into<String>, position: egui::Pos2) -> Self {
        Self {
            id,
            label: label.into(),
            position,
            pressed: false,
            marked_for_deletion: false,
        }
    }
    pub fn draw(&mut self, ctx: &egui::Context) {
        let fill = if self.pressed {
            egui::Color32::from_rgb(80, 160, 80)
        } else {
            egui::Color32::from_rgb(60, 60, 60)
        };

        egui::Window::new("##")
            .id(egui::Id::new(self.id))
            .title_bar(false)
            .resizable(false)
            .fixed_pos(self.position)
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let btn = egui::Button::new(&self.label).fill(fill);
                if ui.add(btn).clicked() {
                    self.pressed = !self.pressed;
                }
            });
    }
    pub fn move_to(&mut self, pos: egui::Pos2) {
        self.position = pos;
    }
    pub fn delete(&mut self) {
        self.marked_for_deletion = true;
    }
}


pub struct EguiRenderer {
    pub ctx: egui::Context,
    painter: egui_glow::Painter,
    pub buttons: Vec<UiButton>,
}

impl EguiRenderer {
    pub fn new(gl: Arc<egui_glow::glow::Context>) -> Self {
        let ctx = egui::Context::default();
        let painter = egui_glow::Painter::new(gl, "", None, false)
            .expect("Failed to create egui painter");
        Self { ctx, painter, buttons: Vec::new() }
    }

    pub fn add_button(&mut self, button: UiButton) {
        self.buttons.push(button);
    }

    pub fn render(
        &mut self,
        input: egui::RawInput,
        screen_width: u32,
        screen_height: u32,
        mut ui_fn: impl FnMut(&egui::Context),
    ) {
        let buttons = &mut self.buttons;
        let full_output = self.ctx.run(input, |ctx| {
            for btn in buttons.iter_mut() {
                btn.draw(ctx);
            }
            ui_fn(ctx);
        });
        let primitives = self.ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        self.painter.paint_and_update_textures(
            [screen_width, screen_height],
            full_output.pixels_per_point,
            &primitives,
            &full_output.textures_delta,
        );
    }
}
