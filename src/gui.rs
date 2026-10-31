use eframe::egui;
use png2iff_core::{Mode, Options, Output, Rgba, Transparency, convert_rgba, decode_png};
use std::path::PathBuf;

pub fn run() -> Result<(), String> {
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Png2Iff - PNG nach Amiga IFF")
            .with_inner_size([1060.0, 660.0])
            .with_min_inner_size([640.0, 420.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("Png2Iff", native, Box::new(|_cc| Ok(Box::new(App::default())))).map_err(|e| e.to_string())
}

struct Loaded {
    path: PathBuf,
    rgba: Rgba,
    texture: egui::TextureHandle,
}

struct Converted {
    output: Output,
    texture: egui::TextureHandle,
}

struct App {
    opts: Options,
    loaded: Option<Loaded>,
    converted: Option<Converted>,
    dirty: bool,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            opts: Options::default(),
            loaded: None,
            converted: None,
            dirty: false,
            status: "PNG-Datei öffnen oder per Drag & Drop hierher ziehen.".into(),
        }
    }
}

fn texture(ctx: &egui::Context, name: &str, w: u32, h: u32, rgba: &[u8]) -> egui::TextureHandle {
    let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba);
    ctx.load_texture(name, img, egui::TextureOptions::NEAREST)
}

impl App {
    fn load(&mut self, ctx: &egui::Context, path: PathBuf) {
        let result = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| decode_png(&b).map_err(|e| e.to_string()));
        match result {
            Ok(rgba) => {
                let tex = texture(ctx, "original", rgba.width, rgba.height, &rgba.data);
                self.status = format!("{} geladen", path.display());
                self.loaded = Some(Loaded { path, rgba, texture: tex });
                self.dirty = true;
            }
            Err(e) => self.status = format!("{}: {e}", path.display()),
        }
    }

    fn reconvert(&mut self, ctx: &egui::Context) {
        self.dirty = false;
        let Some(loaded) = &self.loaded else { return };
        match convert_rgba(&loaded.rgba, &self.opts) {
            Ok(output) => {
                let texture = texture(ctx, "preview", output.width, output.height, &output.preview_rgba);
                self.converted = Some(Converted { output, texture });
            }
            Err(e) => {
                self.converted = None;
                self.status = e.to_string();
            }
        }
    }

    fn save(&mut self) {
        let (Some(loaded), Some(conv)) = (&self.loaded, &self.converted) else { return };
        let name = loaded.path.with_extension("iff");
        let dialog = rfd::FileDialog::new()
            .add_filter("Amiga IFF", &["iff", "ilbm", "lbm"])
            .set_file_name(name.file_name().and_then(|n| n.to_str()).unwrap_or("bild.iff"));
        let dialog = match loaded.path.parent() {
            Some(dir) => dialog.set_directory(dir),
            None => dialog,
        };
        if let Some(path) = dialog.save_file() {
            self.status = match std::fs::write(&path, &conv.output.iff) {
                Ok(()) => format!("{} gespeichert ({} Bytes)", path.display(), conv.output.iff.len()),
                Err(e) => format!("{}: {e}", path.display()),
            };
        }
    }

    fn options_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        let o = &mut self.opts;
        ui.heading("Format");
        changed |= ui.radio_value(&mut o.mode, Mode::Indexed, "Palette (2-256 Farben)").changed();
        changed |= ui.radio_value(&mut o.mode, Mode::Ehb, "EHB (64 Farben)").changed();
        changed |= ui.radio_value(&mut o.mode, Mode::Ham6, "HAM6 (4096 Farben)").changed();
        changed |= ui.radio_value(&mut o.mode, Mode::Ham8, "HAM8 (262144 Farben)").changed();
        ui.separator();

        ui.add_enabled_ui(o.mode == Mode::Indexed, |ui| {
            changed |= ui.add(egui::Slider::new(&mut o.planes, 1..=8).text("Max. Bitplanes")).changed();
            changed |= ui.checkbox(&mut o.auto_planes, "Planes automatisch reduzieren").changed();
        });
        ui.add_enabled_ui(matches!(o.mode, Mode::Indexed | Mode::Ehb), |ui| {
            changed |= ui.checkbox(&mut o.dither, "Dithering (Floyd-Steinberg)").changed();
        });
        ui.add_enabled_ui(matches!(o.mode, Mode::Indexed | Mode::Ehb), |ui| {
            changed |= ui.checkbox(&mut o.ocs_palette, "OCS/ECS-Palette (4 Bit)").changed();
        });
        changed |= ui.checkbox(&mut o.compress, "ByteRun1-Kompression").changed();
        ui.separator();

        ui.heading("Transparenz");
        changed |= ui.radio_value(&mut o.transparency, Transparency::Mask, "Maskenebene").changed();
        ui.add_enabled_ui(o.mode == Mode::Indexed, |ui| {
            changed |= ui.radio_value(&mut o.transparency, Transparency::ColorKey, "Transparente Farbe (Index 0)").changed();
        });
        changed |= ui.radio_value(&mut o.transparency, Transparency::None, "Keine (Hintergrund verrechnen)").changed();
        ui.horizontal(|ui| {
            changed |= ui.color_edit_button_srgb(&mut o.background).changed();
            ui.label("Hintergrundfarbe");
        });
        changed
    }
}

fn show_image(ui: &mut egui::Ui, tex: &egui::TextureHandle) {
    let size = tex.size_vec2();
    let avail = ui.available_size();
    let mut scale = (avail.x / size.x).min(avail.y / size.y).clamp(0.01, 8.0);
    if scale >= 1.0 {
        scale = scale.floor();
    }
    ui.add(egui::Image::new(tex).fit_to_exact_size(size * scale));
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dropped = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).next());
        if let Some(path) = dropped {
            self.load(&ctx, path);
        }

        let mut open = false;
        let mut save = false;
        egui::Panel::left("options").default_size(270.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                open = ui.button("PNG öffnen…").clicked();
                save = ui.add_enabled(self.converted.is_some(), egui::Button::new("Als IFF speichern…")).clicked();
            });
            ui.separator();
            if self.options_ui(ui) {
                self.dirty = true;
            }
        });

        egui::Panel::bottom("status").show(ui, |ui| {
            match &self.converted {
                Some(c) => ui.label(format!(
                    "{}x{} Pixel, {} Bitplanes, {} Farben, IFF-Größe {} Bytes  |  {}",
                    c.output.width, c.output.height, c.output.planes, c.output.colors, c.output.iff.len(), self.status
                )),
                None => ui.label(&self.status),
            };
        });

        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(loaded) = &self.loaded {
                ui.columns(2, |cols| {
                    cols[0].label("Original");
                    show_image(&mut cols[0], &loaded.texture);
                    cols[1].label("Amiga-Vorschau");
                    if let Some(c) = &self.converted {
                        show_image(&mut cols[1], &c.texture);
                    }
                });
            } else {
                ui.centered_and_justified(|ui| ui.label("PNG-Datei hierher ziehen oder über „PNG öffnen…“ wählen"));
            }
        });

        if open {
            if let Some(path) = rfd::FileDialog::new().add_filter("PNG-Bild", &["png"]).pick_file() {
                self.load(&ctx, path);
            }
        }
        if self.dirty {
            self.reconvert(&ctx);
        }
        if save {
            self.save();
        }
    }
}
