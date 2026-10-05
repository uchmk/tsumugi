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
//! The sidebar lists the server's workspaces (tabs); the one picked is drawn
//! beside it, its panes split as the server keeps them (`tsumugi-layout`).

// No console window behind the GUI on Windows, in a release build.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod fonts;
mod keys;
mod spawn;

use std::time::Duration;

use eframe::egui;
use std::collections::HashMap;

use tsumugi_layout::{Node, Rect};
use tsumugi_mux::{Address, Client, Dir, Info, Place, RemotePane, SessionId, State, Workspace, WorkspaceId};
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

/// Make sure the server has a tab to show: a new shell here when it has none.
fn first_session(client: &Client) -> Result<(), String> {
    let list = client.list().map_err(|e| e.to_string())?;
    if list.is_empty() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        new_session(client, cwd, Place::NewWorkspace).map(drop)?;
    }
    Ok(())
}

fn new_session(client: &Client, cwd: std::path::PathBuf, place: Place) -> Result<RemotePane, String> {
    let shell = tsumugi_pane::default_shell().map(|s| (s, Vec::new()));
    client.spawn_at(cwd, shell, Size::new(80, 24), (8, 16), place).map_err(|e| format!("the shell did not start: {e}"))
}

/// The sidebar's width when the window first opens, and how far a drag may
/// take it (docs/v1-scope.md 1f).
const SIDEBAR: f32 = 240.0;
const SIDEBAR_RANGE: std::ops::RangeInclusive<f32> = 200.0..=480.0;
/// The gap between split panes, and how wide a strip around it a drag can
/// start in (1f: 12px).
const GAP: f32 = 6.0;
const GRAB: f32 = 12.0;
/// How much a pane without the keys is dimmed (1e).
const DIM: f32 = 0.35;

/// The most urgent state among a tab's panes, which its row shows.
fn urgency(state: State) -> u8 {
    match state {
        State::Waiting => 4,
        State::Error => 3,
        State::MaybeWaiting => 2,
        State::Running => 1,
        State::Done => 0,
    }
}

struct App {
    client: Option<Client>,
    /// Why there is nothing to show, shown in its place.
    failed: Option<String>,
    /// The tab shown.
    active: Option<WorkspaceId>,
    /// A session just started, to show once the server has placed it.
    pending: Option<SessionId>,
    /// The panes on screen, attached; the rest are not sent here.
    panes: HashMap<SessionId, RemotePane>,
    views: HashMap<SessionId, ViewState>,
    /// The pane with the keys shown alone (`Ctrl+Shift+Z`).
    zoom: bool,
    /// A divider being dragged: the tab's shape as the drag has it so far.
    dragging: Option<(WorkspaceId, Node<SessionId>)>,
    palette: Palette,
    font: egui::FontId,
    /// What the window title was last set to, so it is set only on a change.
    title: String,
    /// The server has had a tab since this window opened: when the last one
    /// goes, so does the window.
    had_tabs: bool,
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
        let (client, failed) = match connect(move || ctx.request_repaint()) {
            Ok(client) => {
                let failed = first_session(&client).err();
                (Some(client), failed)
            }
            Err(e) => (None, Some(e)),
        };
        Self {
            client,
            failed,
            active: None,
            pending: None,
            panes: HashMap::new(),
            views: HashMap::new(),
            zoom: false,
            dragging: None,
            palette,
            font: egui::FontId::monospace(FONT_SIZE),
            title: String::new(),
            had_tabs: false,
        }
    }

    /// The tab shown, following a new session to the tab it landed in.
    fn current(&mut self, workspaces: &[Workspace]) -> Option<Workspace> {
        if let Some(new) = self.pending {
            if let Some(w) = workspaces.iter().find(|w| w.layout.contains(&new)) {
                self.active = Some(w.id);
                self.pending = None;
            }
        }
        let found = self.active.and_then(|id| workspaces.iter().find(|w| w.id == id));
        let w = found.or_else(|| workspaces.first())?.clone();
        self.active = Some(w.id);
        Some(w)
    }

    fn set_focus(&self, w: &Workspace, focus: SessionId) {
        if let Some(client) = &self.client {
            client.set_layout(w.id, w.layout.clone(), focus);
        }
    }

    fn act(&mut self, action: keys::Action, w: &Workspace, workspaces: &[Workspace], area: Rect) {
        let Some(client) = self.client.clone() else { return };
        let sessions = client.sessions();
        let cwd_of = |id: SessionId| sessions.iter().find(|i| i.id == id).map(|i| i.cwd.clone());
        // A new shell starts where the pane with the keys is.
        let here = cwd_of(w.focus).or_else(|| std::env::current_dir().ok()).unwrap_or_else(|| ".".into());
        let at = workspaces.iter().position(|x| x.id == w.id).unwrap_or(0);
        let mut start = |place| match new_session(&client, here.clone(), place) {
            Ok(pane) => Some(pane.id()),
            Err(e) => {
                self.failed = Some(e);
                None
            }
        };
        match action {
            keys::Action::NewTab => self.pending = start(Place::NewWorkspace),
            keys::Action::SplitRight => self.pending = start(Place::Split { beside: w.focus, dir: Dir::Right }),
            keys::Action::SplitDown => self.pending = start(Place::Split { beside: w.focus, dir: Dir::Down }),
            keys::Action::CloseTab => {
                if let Some(pane) = self.panes.get(&w.focus) {
                    pane.kill();
                }
            }
            keys::Action::NextTab | keys::Action::PrevTab => {
                let n = workspaces.len();
                let next = if action == keys::Action::NextTab { (at + 1) % n } else { (at + n - 1) % n };
                self.active = Some(workspaces[next].id);
            }
            keys::Action::Tab(i) => {
                if let Some(x) = workspaces.get(i) {
                    self.active = Some(x.id);
                }
            }
            keys::Action::NextWaiting => {
                // The longest-waiting first; from the one shown, on to the next.
                let mut waiting: Vec<&Info> =
                    sessions.iter().filter(|i| matches!(i.state, State::Waiting | State::MaybeWaiting)).collect();
                waiting.sort_by_key(|i| i.since_ms);
                let here = waiting.iter().position(|i| i.id == w.focus);
                let next = here.map_or(0, |h| (h + 1) % waiting.len().max(1));
                if let Some(info) = waiting.get(next) {
                    if let Some(x) = workspaces.iter().find(|x| x.layout.contains(&info.id)) {
                        self.active = Some(x.id);
                        self.set_focus(x, info.id);
                    }
                }
            }
            keys::Action::Move(toward) => {
                if let Some(next) = w.layout.neighbor(&w.focus, toward, area) {
                    self.set_focus(w, next);
                }
            }
            keys::Action::Zoom => self.zoom = !self.zoom,
        }
    }

    /// One row per tab: the state of its most urgent pane, the title of the
    /// pane with the keys, and how many panes it has.
    fn sidebar(&mut self, ui: &mut egui::Ui, workspaces: &[Workspace], sessions: &[Info]) -> Option<WorkspaceId> {
        let pal = self.palette;
        let mut picked = None;
        ui.add_space(6.0);
        for w in workspaces {
            let infos: Vec<&Info> = w.layout.leaves().iter().filter_map(|id| sessions.iter().find(|i| i.id == *id)).collect();
            let Some(focus) = infos.iter().find(|i| i.id == w.focus).or(infos.first()) else { continue };
            let urgent = infos.iter().max_by_key(|i| (urgency(i.state), std::cmp::Reverse(i.since_ms))).unwrap_or(focus);
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), egui::Sense::click());
            let painter = ui.painter_at(rect);
            if Some(w.id) == self.active {
                painter.rect_filled(rect.shrink2(egui::vec2(6.0, 2.0)), 6.0, pal.selection);
            } else if resp.hovered() {
                painter.rect_filled(rect.shrink2(egui::vec2(6.0, 2.0)), 6.0, pal.selection.gamma_multiply(0.4));
            }
            let dot = egui::pos2(rect.left() + 18.0, rect.top() + 15.0);
            let color = state_color(urgent.state);
            match urgent.state {
                // The guess is a ring, the sure mark a filled dot (Q2).
                State::MaybeWaiting => {
                    painter.circle_stroke(dot, 4.0, egui::Stroke::new(1.5, color));
                }
                _ => {
                    painter.circle_filled(dot, 4.0, color);
                }
            }
            let name = if focus.title.is_empty() { program_name(&focus.command) } else { focus.title.clone() };
            let name = if infos.len() > 1 { format!("{name}  ·{}", infos.len()) } else { name };
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
            let second = match urgent.state {
                State::Running => home_short(&focus.cwd),
                _ if !urgent.note.is_empty() => urgent.note.clone(),
                _ => home_short(&focus.cwd),
            };
            line(second, 25.0, 11.0, pal.fg_dim);
            if resp.clicked() {
                picked = Some(w.id);
            }
        }
        picked
    }

    /// The tab's panes in `area`, each with its own view, and the dividers
    /// between them to drag.
    fn panes(&mut self, ui: &mut egui::Ui, w: &Workspace, sessions: &[Info]) {
        let area = to_rect(ui.max_rect());
        let layout = match &self.dragging {
            Some((id, l)) if *id == w.id => l.clone(),
            _ => w.layout.clone(),
        };
        let rects = if self.zoom { vec![(w.focus, area)] } else { layout.layout(area, GAP) };
        // Attach what is on screen, and let the rest go.
        if let Some(client) = &self.client {
            for (id, _) in &rects {
                self.panes.entry(*id).or_insert_with(|| client.attach(*id));
            }
        }
        self.panes.retain(|id, _| rects.iter().any(|(r, _)| r == id));
        self.views.retain(|id, _| rects.iter().any(|(r, _)| r == id));

        let row_h = ui.fonts_mut(|f| f.row_height(&self.font)).ceil();
        let ctx = ui.ctx().clone();
        let mut focus_to = None;
        for (id, r) in &rects {
            let rect = from_rect(*r);
            let focused = *id == w.focus;
            let (Some(pane), view) = (self.panes.get_mut(id), self.views.entry(*id).or_default()) else { continue };
            let opts = ViewOptions { focused, wheel: true };
            let shown = ui.push_id(id, |ui| tsumugi_pane::show(ui, Some(pane), view, rect, &self.font, row_h, &self.palette, opts)).inner;
            if !focused {
                // The panes without the keys sit back; their marks do not (1e).
                ui.painter().rect_filled(rect, 0.0, self.palette.on_cursor.gamma_multiply(DIM));
            }
            if shown.focus && !focused {
                focus_to = Some(*id);
            }
            if let Some(text) = shown.copy {
                ctx.copy_text(text);
            }
            if shown.paste {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
        }
        if let Some(id) = focus_to {
            self.set_focus(w, id);
        }

        if self.zoom {
            // Said in gold when a pane hidden by the zoom wants a person (1f).
            let hidden = layout.leaves().into_iter().filter(|id| *id != w.focus);
            let waiting = hidden.filter(|id| sessions.iter().any(|i| i.id == *id && i.state == State::Waiting)).count();
            if waiting > 0 {
                let text = format!(" {waiting} waiting behind the zoom ");
                let at = egui::pos2(area.right() - 8.0, area.y + 6.0);
                let galley = ui.fonts_mut(|f| f.layout_no_wrap(text, egui::FontId::proportional(12.0), self.palette.on_cursor));
                let r = egui::Rect::from_min_size(egui::pos2(at.x - galley.size().x, at.y), galley.size());
                ui.painter().rect_filled(r.expand(2.0), 4.0, egui::Color32::from_rgb(0xe8, 0xc8, 0x7a));
                ui.painter().galley(r.min, galley, self.palette.on_cursor);
            }
            return;
        }

        // The dividers: only a gap until the pointer comes near, then a line
        // to grab; dragged, the split follows; double-clicked, it halves.
        for d in layout.dividers(area, GAP) {
            let gap = from_rect(d.gap);
            let grab = match d.dir {
                Dir::Right => gap.expand2(egui::vec2((GRAB - GAP) / 2.0, 0.0)),
                Dir::Down => gap.expand2(egui::vec2(0.0, (GRAB - GAP) / 2.0)),
            };
            let resp = ui.interact(grab, ui.id().with(("divider", &d.path)), egui::Sense::click_and_drag());
            let cursor = match d.dir {
                Dir::Right => egui::CursorIcon::ResizeHorizontal,
                Dir::Down => egui::CursorIcon::ResizeVertical,
            };
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(cursor);
                ui.painter().rect_filled(gap.shrink2(match d.dir {
                    Dir::Right => egui::vec2(GAP / 2.0 - 1.0, 0.0),
                    Dir::Down => egui::vec2(0.0, GAP / 2.0 - 1.0),
                }), 1.0, self.palette.cursor);
            }
            if resp.double_clicked() {
                let mut l = layout.clone();
                l.set_ratio(&d.path, 0.5);
                if let Some(client) = &self.client {
                    client.set_layout(w.id, l, w.focus);
                }
            } else if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
                let ratio = match d.dir {
                    Dir::Right => (p.x - d.area.x) / d.area.w.max(1.0),
                    Dir::Down => (p.y - d.area.y) / d.area.h.max(1.0),
                };
                let mut l = layout.clone();
                l.set_ratio(&d.path, ratio);
                self.dragging = Some((w.id, l));
            }
            if resp.drag_stopped() {
                if let (Some((_, l)), Some(client)) = (self.dragging.take(), &self.client) {
                    client.set_layout(w.id, l, w.focus);
                }
            }
        }
    }
}

fn to_rect(r: egui::Rect) -> Rect {
    Rect::new(r.min.x, r.min.y, r.width(), r.height())
}

fn from_rect(r: Rect) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(r.x, r.y), egui::vec2(r.w, r.h))
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
        let Some(client) = self.client.clone() else {
            self.message(ui);
            return;
        };
        for text in client.take_clipboard() {
            ctx.copy_text(text);
        }
        if client.lost() && self.failed.is_none() {
            self.failed = Some("The tsumugi server went away.".into());
            self.panes.clear();
        }
        let workspaces = client.workspaces();
        let sessions = client.sessions();
        if !workspaces.is_empty() {
            self.had_tabs = true;
        } else if self.had_tabs && self.pending.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let current = self.current(&workspaces);

        if let Some(w) = &current {
            let t = sessions.iter().find(|i| i.id == w.focus).map(|i| i.title.clone()).unwrap_or_default();
            let title = if t.is_empty() { "tsumugi".to_owned() } else { format!("{t} — tsumugi") };
            if title != self.title {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
                self.title = title;
            }
            let mut actions = Vec::new();
            if let Some(pane) = self.panes.get(&w.focus) {
                let events = ctx.input(|i| i.events.clone());
                tsumugi_pane::input::feed(pane, &events, |key, m| match keys::action(key, m) {
                    Some(a) => {
                        actions.push(a);
                        true
                    }
                    None => false,
                });
            }
            let area = to_rect(ctx.content_rect());
            for a in actions {
                self.act(a, w, &workspaces, area);
            }
        }

        let side = egui::Frame::NONE.fill(self.palette.on_cursor);
        let picked = egui::Panel::left("sessions")
            .resizable(true)
            .default_size(SIDEBAR)
            .size_range(SIDEBAR_RANGE)
            .frame(side)
            .show(ui, |ui| self.sidebar(ui, &workspaces, &sessions))
            .inner;
        if let Some(id) = picked {
            self.active = Some(id);
        }

        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(self.palette.on_cursor)).show(ui, |ui| {
            if self.failed.is_some() {
                self.message(ui);
                return;
            }
            if let Some(w) = &current {
                self.panes(ui, w, &sessions);
            }
        });
    }
}

impl App {
    fn message(&self, ui: &mut egui::Ui) {
        if let Some(why) = &self.failed {
            let rect = ui.max_rect();
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, why, self.font.clone(), self.palette.fg);
        }
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
