//! tsumugi: a terminal for running many AI CLI sessions side by side.
//!
//! The sessions live in a server (`tsumugi server`, the `tsumugi-mux`
//! crate), and the window is a client of it: closing the window, or the
//! window crashing, leaves the shells running, and the next window shows them
//! again. The window starts the server when none is running.
//!
//!   tsumugi            the window
//!   tsumugi server     the server (the window starts it by itself)
//!   tsumugi ls         the sessions, one per line
//!
//! Tabs, splits and the waiting marks come after (docs/v1-scope.md).

// No console window behind the GUI on Windows, in a release build.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod fonts;
mod spawn;

use std::time::Duration;

use eframe::egui;
use tsumugi_mux::{Address, Client, RemotePane};
use tsumugi_pane::{Palette, Size, ViewOptions, ViewState};

/// The pane's text size, in points.
const FONT_SIZE: f32 = 14.0;

fn main() -> std::process::ExitCode {
    // Before anything loads a DLL: `conpty.dll` only from beside the exe.
    tsumugi_pane::restrict_dll_search();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a != "server") {
        attach_console();
    }
    match args.first().map(String::as_str) {
        None => window(),
        Some("server") => server(),
        Some("ls") => ls(),
        Some("--version" | "-V") => {
            println!("tsumugi {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("tsumugi: unknown command `{other}` (server, ls, --version)");
            std::process::ExitCode::from(2)
        }
    }
}

/// A release build on Windows is a GUI program and has no console, so `ls`
/// printed nothing. Borrow the console of whoever started it. PowerShell does
/// not wait for a GUI program, so the lines can land after its next prompt;
/// filer solved that with a console front (`filer.com`), which can come here
/// too once the CLI grows.
fn attach_console() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn server() -> std::process::ExitCode {
    match tsumugi_mux::server::start(&Address::for_user()) {
        // A server no window ever started a session on goes after a minute.
        Ok(srv) => {
            srv.wait(Duration::from_secs(60));
            // The last `Exited` is still on its way to the windows; leaving
            // at once cut it off, and the window said the server went away
            // instead of closing.
            std::thread::sleep(Duration::from_millis(300));
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tsumugi server: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn ls() -> std::process::ExitCode {
    let listed = Client::connect(&Address::for_user(), || {}).and_then(|c| c.list());
    match listed {
        Ok(list) => {
            for i in list {
                println!("{}\t{}\t{}\t{}", i.id, i.command, i.cwd.display(), i.title);
            }
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tsumugi ls: no server ({e})");
            std::process::ExitCode::FAILURE
        }
    }
}

fn window() -> std::process::ExitCode {
    let viewport = egui::ViewportBuilder::default().with_title("tsumugi").with_inner_size([960.0, 600.0]);
    let options = eframe::NativeOptions { viewport, wgpu_options: wgpu_options(), ..Default::default() };
    match eframe::run_native("tsumugi", options, Box::new(|cc| Ok(Box::new(App::new(cc))))) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tsumugi: {e}");
            std::process::ExitCode::FAILURE
        }
    }
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

/// Connect to this user's server, starting one if none answers.
fn connect(wake: impl Fn() + Send + Sync + Clone + 'static) -> Result<Client, String> {
    let at = Address::for_user();
    if let Ok(c) = Client::connect(&at, wake.clone()) {
        return Ok(c);
    }
    spawn::server().map_err(|e| format!("could not start the server: {e}"))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        match Client::connect(&at, wake.clone()) {
            Ok(c) => return Ok(c),
            Err(e) if std::time::Instant::now() > deadline => return Err(format!("the server did not answer: {e}")),
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// The session to show: the first one the server has, or a new shell here.
fn open_session(client: &Client) -> Result<RemotePane, String> {
    let list = client.list().map_err(|e| e.to_string())?;
    if let Some(first) = list.first() {
        return Ok(client.attach(first.id));
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let shell = tsumugi_pane::default_shell().map(|s| (s, Vec::new()));
    client.spawn(cwd, shell, Size::new(80, 24), (8, 16)).map_err(|e| format!("the shell did not start: {e}"))
}

struct App {
    client: Option<Client>,
    pane: Option<RemotePane>,
    /// Why there is no pane, shown in its place.
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

        let ctx = cc.egui_ctx.clone();
        let (client, pane, failed) = match connect(move || ctx.request_repaint()) {
            Ok(client) => match open_session(&client) {
                Ok(pane) => (Some(client), Some(pane), None),
                Err(e) => (Some(client), None, Some(e)),
            },
            Err(e) => (None, None, Some(e)),
        };
        Self { client, pane, failed, view: ViewState::default(), palette, font: egui::FontId::monospace(FONT_SIZE), title: String::new() }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(client) = &self.client {
            for text in client.take_clipboard() {
                ctx.copy_text(text);
            }
            if client.lost() && self.failed.is_none() {
                self.failed = Some("The tsumugi server went away.".into());
                self.pane = None;
            }
        }
        if let Some(pane) = &self.pane {
            if pane.exited() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            let t = pane.title();
            let title = if t.is_empty() { "tsumugi".to_owned() } else { format!("{t} — tsumugi") };
            if title != self.title {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
                self.title = title;
            }
            let events = ctx.input(|i| i.events.clone());
            tsumugi_pane::input::feed(pane, &events, |_, _| false);
        }

        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(self.palette.bg)).show(ui, |ui| {
            let rect = ui.max_rect();
            let row_h = ui.fonts_mut(|f| f.row_height(&self.font)).ceil();
            if let Some(why) = &self.failed {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, why, self.font.clone(), self.palette.fg);
                return;
            }
            let opts = ViewOptions { focused: true, wheel: true };
            let shown = tsumugi_pane::show(ui, self.pane.as_mut(), &mut self.view, rect, &self.font, row_h, &self.palette, opts);
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
