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
//!   tsumugi notify     mark this session (waiting / done / error), from a hook
//!
//! The sidebar lists the server's sessions; the one picked is drawn beside
//! it. Splits and the waiting marks come after (docs/v1-scope.md).

// No console window behind the GUI on Windows, in a release build.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod fonts;
mod keys;
mod spawn;

use std::time::Duration;

use eframe::egui;
use tsumugi_mux::{Address, Client, Info, RemotePane, SessionId};
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
        Some("notify") => notify(&args[1..]),
        Some("--version" | "-V") => {
            println!("tsumugi {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("tsumugi: unknown command `{other}` (server, ls, notify, --version)");
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
                println!("{}\t{}\t{}\t{}\t{}\t{}", i.id, i.state.word(), i.command, i.cwd.display(), i.title, i.note);
            }
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tsumugi ls: no server ({e})");
            std::process::ExitCode::FAILURE
        }
    }
}

/// `tsumugi notify [--state waiting|done|error] [--session N] [MESSAGE...]`:
/// mark a session for the sidebar. Meant for an agent's hooks, run inside
/// the session, which is where `TSUMUGI_SESSION` and `TSUMUGI_ADDRESS` are
/// set. Claude Code's `Notification` hook is `tsumugi notify`, its `Stop`
/// hook `tsumugi notify --state done`.
fn notify(args: &[String]) -> std::process::ExitCode {
    let mut state = tsumugi_mux::State::Waiting;
    let mut session = std::env::var("TSUMUGI_SESSION").ok().and_then(|s| s.parse().ok());
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--state" => match it.next().and_then(|s| tsumugi_mux::State::from_word(s)) {
                Some(s) => state = s,
                None => {
                    eprintln!("tsumugi notify: --state takes waiting, done, error or running");
                    return std::process::ExitCode::from(2);
                }
            },
            "--session" => session = it.next().and_then(|s| s.parse().ok()),
            // Claude Code hands a hook its event as JSON on stdin; its
            // `message` ("Claude needs your permission to use Bash") is the
            // note the sidebar shows.
            "--stdin" => {
                let mut input = String::new();
                let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut input);
                if let Some(m) = json_string(&input, "message") {
                    words.push(m);
                }
            }
            _ => words.push(a.clone()),
        }
    }
    let Some(id) = session else {
        eprintln!("tsumugi notify: not inside a tsumugi session (no TSUMUGI_SESSION); give --session N");
        return std::process::ExitCode::from(2);
    };
    match Client::connect(&Address::for_user(), || {}).and_then(|c| c.notify(id, state, words.join(" "))) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tsumugi notify: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// The string value of `key` at the top of a JSON object, escapes undone --
/// enough for a hook's event without a JSON crate.
fn json_string(json: &str, key: &str) -> Option<String> {
    let at = json.find(&format!("\"{key}\""))? + key.len() + 2;
    let rest = json[at..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).unwrap_or('\u{fffd}'));
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
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

/// The session to show at first: the first one the server has, or a new
/// shell here.
fn open_session(client: &Client) -> Result<RemotePane, String> {
    let list = client.list().map_err(|e| e.to_string())?;
    if let Some(first) = list.first() {
        return Ok(client.attach(first.id));
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    new_session(client, cwd)
}

fn new_session(client: &Client, cwd: std::path::PathBuf) -> Result<RemotePane, String> {
    let shell = tsumugi_pane::default_shell().map(|s| (s, Vec::new()));
    client.spawn(cwd, shell, Size::new(80, 24), (8, 16)).map_err(|e| format!("the shell did not start: {e}"))
}

/// The sidebar's width when the window first opens, and how far a drag may
/// take it (docs/v1-scope.md 1f).
const SIDEBAR: f32 = 240.0;
const SIDEBAR_RANGE: std::ops::RangeInclusive<f32> = 200.0..=480.0;

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

    fn active(&self) -> Option<SessionId> {
        self.pane.as_ref().map(RemotePane::id)
    }

    /// Show session `id` (dropping the old pane stops its screen coming).
    fn switch(&mut self, id: SessionId) {
        if self.active() == Some(id) {
            return;
        }
        if let Some(client) = &self.client {
            self.pane = Some(client.attach(id));
            self.view = ViewState::default();
        }
    }

    fn act(&mut self, action: keys::Action, ctx: &egui::Context) {
        let Some(client) = self.client.clone() else { return };
        let list = client.sessions();
        let at = self.active().and_then(|id| list.iter().position(|i| i.id == id));
        match action {
            keys::Action::NewTab => {
                // The new shell starts where the shown one is.
                let cwd = at.map(|i| list[i].cwd.clone()).or_else(|| std::env::current_dir().ok()).unwrap_or_else(|| ".".into());
                match new_session(&client, cwd) {
                    Ok(pane) => {
                        self.pane = Some(pane);
                        self.view = ViewState::default();
                    }
                    Err(e) => self.failed = Some(e),
                }
            }
            keys::Action::CloseTab => {
                if let Some(pane) = &self.pane {
                    pane.kill();
                }
            }
            keys::Action::NextTab | keys::Action::PrevTab if !list.is_empty() => {
                let n = list.len();
                let i = at.unwrap_or(0);
                let next = if action == keys::Action::NextTab { (i + 1) % n } else { (i + n - 1) % n };
                self.switch(list[next].id);
            }
            keys::Action::NextWaiting => {
                // The longest-waiting first; from the one shown, on to the next.
                let mut waiting: Vec<&Info> =
                    list.iter().filter(|i| matches!(i.state, tsumugi_mux::State::Waiting | tsumugi_mux::State::MaybeWaiting)).collect();
                waiting.sort_by_key(|i| i.since_ms);
                let here = self.active().and_then(|id| waiting.iter().position(|i| i.id == id));
                let next = here.map_or(0, |h| (h + 1) % waiting.len().max(1));
                if let Some(info) = waiting.get(next) {
                    self.switch(info.id);
                }
            }
            keys::Action::Tab(i) => {
                if let Some(info) = list.get(i) {
                    self.switch(info.id);
                }
            }
            _ => {}
        }
        ctx.request_repaint();
    }

    /// The shown session's shell ended: show the next one, or close.
    fn after_exit(&mut self, ctx: &egui::Context) {
        let Some(gone) = self.active() else { return };
        let list = self.client.as_ref().map(Client::sessions).unwrap_or_default();
        let rest: Vec<&Info> = list.iter().filter(|i| i.id != gone).collect();
        match rest.first() {
            Some(next) => {
                let id = next.id;
                self.pane = None;
                self.switch(id);
            }
            None => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) -> Option<SessionId> {
        let list = self.client.as_ref().map(Client::sessions).unwrap_or_default();
        let active = self.active();
        let pal = self.palette;
        let mut picked = None;
        ui.add_space(6.0);
        for info in &list {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), egui::Sense::click());
            let painter = ui.painter_at(rect);
            if Some(info.id) == active {
                painter.rect_filled(rect.shrink2(egui::vec2(6.0, 2.0)), 6.0, pal.selection);
            } else if resp.hovered() {
                painter.rect_filled(rect.shrink2(egui::vec2(6.0, 2.0)), 6.0, pal.selection.gamma_multiply(0.4));
            }
            let dot = egui::pos2(rect.left() + 18.0, rect.top() + 15.0);
            let color = state_color(info.state);
            match info.state {
                // The guess is a ring, the sure mark a filled dot (Q2).
                tsumugi_mux::State::MaybeWaiting => {
                    painter.circle_stroke(dot, 4.0, egui::Stroke::new(1.5, color));
                }
                _ => {
                    painter.circle_filled(dot, 4.0, color);
                }
            }
            let name = if info.title.is_empty() { program_name(&info.command) } else { info.title.clone() };
            let left = rect.left() + 30.0;
            let width = rect.right() - left - 10.0;
            let line = |text: String, y: f32, size: f32, color: egui::Color32| {
                let galley = ui.fonts_mut(|f| {
                    let mut job = egui::text::LayoutJob::simple_singleline(text, egui::FontId::proportional(size), color);
                    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
                    f.layout_job(job)
                });
                painter.galley(egui::pos2(left, rect.top() + y), galley, color);
            };
            line(name, 7.0, 13.0, pal.fg);
            let second = match info.state {
                tsumugi_mux::State::Running => home_short(&info.cwd),
                _ if !info.note.is_empty() => info.note.clone(),
                _ => home_short(&info.cwd),
            };
            line(second, 25.0, 11.0, pal.fg_dim);
            if resp.clicked() {
                picked = Some(info.id);
            }
        }
        picked
    }
}

/// The four state colours (docs/v1-scope.md 1k): waiting yellow, running
/// cyan, error red, done green -- filer's own yellow, cyan, red and green.
fn state_color(state: tsumugi_mux::State) -> egui::Color32 {
    use tsumugi_mux::State;
    match state {
        State::Waiting | State::MaybeWaiting => egui::Color32::from_rgb(0xe8, 0xc8, 0x7a),
        State::Running => egui::Color32::from_rgb(0x6f, 0xd0, 0xd0),
        State::Error => egui::Color32::from_rgb(0xf0, 0x71, 0x78),
        State::Done => egui::Color32::from_rgb(0x8e, 0xd0, 0x8e),
    }
}

/// `pwsh` out of `C:\Program Files\PowerShell\7\pwsh.exe`.
fn program_name(command: &str) -> String {
    let name = command.rsplit(['/', '\\']).next().unwrap_or(command);
    name.strip_suffix(".exe").unwrap_or(name).to_owned()
}

/// A folder with the home folder said as `~`.
fn home_short(path: &std::path::Path) -> String {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(std::path::PathBuf::from);
    match home.as_deref().and_then(|h| path.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => path.display().to_string(),
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
        if self.pane.as_ref().is_some_and(RemotePane::exited) {
            self.after_exit(&ctx);
        }
        let mut actions = Vec::new();
        if let Some(pane) = &self.pane {
            let t = pane.title();
            let title = if t.is_empty() { "tsumugi".to_owned() } else { format!("{t} — tsumugi") };
            if title != self.title {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
                self.title = title;
            }
            let events = ctx.input(|i| i.events.clone());
            if std::env::var_os("TSUMUGI_DEBUG_KEYS").is_some() {
                for e in &events {
                    if !matches!(e, egui::Event::PointerMoved(_) | egui::Event::MouseMoved(_)) {
                        eprintln!("event: {e:?}");
                    }
                }
            }
            tsumugi_pane::input::feed(pane, &events, |key, m| match keys::action(key, m) {
                Some(a) => {
                    actions.push(a);
                    true
                }
                None => false,
            });
        }
        for a in actions {
            self.act(a, &ctx);
        }

        let side = egui::Frame::NONE.fill(self.palette.on_cursor);
        let picked = egui::Panel::left("sessions")
            .resizable(true)
            .default_size(SIDEBAR)
            .size_range(SIDEBAR_RANGE)
            .frame(side)
            .show(ui, |ui| self.sidebar(ui))
            .inner;
        if let Some(id) = picked {
            self.switch(id);
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

#[cfg(test)]
mod tests {
    use super::json_string;

    #[test]
    fn a_hooks_message_is_read() {
        let event = r#"{"session_id":"x","hook_event_name":"Notification","message":"Claude needs your \"OK\"\u3002"}"#;
        assert_eq!(json_string(event, "message").as_deref(), Some("Claude needs your \"OK\"\u{3002}"));
        assert_eq!(json_string(event, "missing"), None);
    }
}
