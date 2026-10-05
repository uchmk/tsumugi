//! tsumugi: a terminal for running many AI CLI sessions side by side.
//!
//! v0.1.0 is one window holding one pane: the shell on a PTY from
//! `tsumugi-pane`, drawn in filer's colours. Tabs, the server that keeps
//! sessions alive, and the waiting marks come after (docs/v1-scope.md).

// No console window behind the GUI on Windows, in a release build.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod fonts;

use eframe::egui;
use tsumugi_pane::{Palette, Size, Terminal, ViewOptions, ViewState};

/// The pane's text size, in points.
const FONT_SIZE: f32 = 14.0;

fn main() -> eframe::Result<()> {
    // Before anything loads a DLL: `conpty.dll` only from beside the exe.
    tsumugi_pane::restrict_dll_search();

    let viewport = egui::ViewportBuilder::default().with_title("tsumugi").with_inner_size([960.0, 600.0]);
    let options = eframe::NativeOptions { viewport, wgpu_options: wgpu_options(), ..Default::default() };
    eframe::run_native("tsumugi", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

/// GL first on Windows, where Vulkan and DX12 spin a core on AMD for an idle
/// window (filer #232); `WGPU_BACKEND` overrides, as it does for wgpu itself.
fn wgpu_options() -> eframe::WgpuConfiguration {
    use eframe::egui_wgpu::WgpuSetup;
    let mut options = eframe::WgpuConfiguration::default();
    if std::env::var_os("WGPU_BACKEND").is_some_and(|v| !v.is_empty()) {
        return options;
    }
    let picked = tsumugi_pane::gpu::pick_backends(None, cfg!(windows), tsumugi_pane::gpu::has_adapter);
    if let (Ok(Some(backends)), WgpuSetup::CreateNew(setup)) = (picked, &mut options.wgpu_setup) {
        setup.instance_descriptor.backends = backends;
    }
    options
}

struct App {
    term: Option<Terminal>,
    /// Why the shell did not start, shown in its place.
    failed: Option<String>,
    view: ViewState,
    palette: Palette,
    font: egui::FontId,
    /// What the window title was last set to, so it is set only on a change.
    title: String,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        fonts::install(&cc.egui_ctx);
        let palette = Palette::default();
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = palette.bg;
        cc.egui_ctx.set_visuals(visuals);
        // egui zooms on Ctrl +/-/0 by itself; in a terminal those are keys.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let shell = tsumugi_pane::default_shell().map(|s| (s, Vec::new()));
        let log = std::env::var_os("TSUMUGI_PTY_LOG").map(std::path::PathBuf::from);
        let ctx = cc.egui_ctx.clone();
        let (term, failed) =
            match Terminal::spawn(&cwd, Size::new(80, 24), (8, 16), shell, log.as_deref(), move || ctx.request_repaint()) {
                Ok(t) => (Some(t), None),
                Err(e) => (None, Some(format!("The shell did not start: {e}"))),
            };
        Self { term, failed, view: ViewState::default(), palette, font: egui::FontId::monospace(FONT_SIZE), title: String::new() }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(term) = &mut self.term {
            for text in term.drain() {
                ctx.copy_text(text);
            }
            term.set_colors(rgb(self.palette.fg), rgb(self.palette.bg));
            if term.exited {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            let title = if term.title.is_empty() { "tsumugi".to_owned() } else { format!("{} — tsumugi", term.title) };
            if title != self.title {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
                self.title = title;
            }
            let events = ctx.input(|i| i.events.clone());
            tsumugi_pane::input::feed(term, &events, |_, _| false);
        }

        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(self.palette.bg)).show(ui, |ui| {
            let rect = ui.max_rect();
            let row_h = ui.fonts_mut(|f| f.row_height(&self.font)).ceil();
            if let Some(why) = &self.failed {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, why, self.font.clone(), self.palette.fg);
                return;
            }
            let opts = ViewOptions { focused: true, wheel: true };
            let shown = tsumugi_pane::show(ui, self.term.as_mut(), &mut self.view, rect, &self.font, row_h, &self.palette, opts);
            if let Some(text) = shown.copy {
                ctx.copy_text(text);
            }
            if shown.paste {
                // A right-click asks for the clipboard; it arrives as a paste
                // event next frame, which `feed` hands to the shell.
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
        });
    }
}

fn rgb(c: egui::Color32) -> [u8; 3] {
    [c.r(), c.g(), c.b()]
}
