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
//!   tsumugi shell-hook make a shell say its folder (pwsh, bash, zsh)
//!
//! The sidebar lists the server's workspaces (tabs); the one picked is drawn
//! beside it, its panes split as the server keeps them (`tsumugi-layout`).

// No console window behind the GUI on Windows, in a release build.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod alert;
mod answer;
mod chrome;
mod cli;
mod drop;
mod files;
mod fonts;
mod hooks;
mod import;
mod json;
mod gitinfo;
mod inputbox;
mod keys;
mod material;
mod backdrop;
mod quake;
mod menu;
mod newsession;
mod palette;
mod prefs;
mod shellhook;
mod sort;
mod sound;
mod spend;
mod theme;
mod usage;
mod price;
mod permit;
mod prompts;
mod parallel;
mod worktree;
mod spawn;
mod autostart;
mod facts;
mod update;
mod export;
mod icon;
mod clip;
mod history;
mod diffview;
mod lists;
mod copymode;
mod remote;
mod layouts;
mod help;
mod find;
mod paste;

use std::time::Duration;

use eframe::egui;
use std::collections::HashMap;

use tsumugi_layout::{Node, Rect};
use tsumugi_mux::{Address, Client, Dir, Info, Place, RemotePane, SessionId, State, Workspace, WorkspaceId};
use tsumugi_pane::{Palette, Size, ViewOptions, ViewState};

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
        Some("ls") => ls(&args[1..]),
        Some("send" | "read" | "split" | "close" | "wait") => remote(&args[0], &args[1..]),
        Some("help" | "--help" | "-h") => {
            println!("tsumugi: the window, or one of\n{}\n{}\ntsumugi attach N|NAME\ntsumugi notify [--state S] [--session N] [MESSAGE]\ntsumugi tag [--session N] [--remove] TAG...\ntsumugi shell-hook bash|zsh|pwsh", cli::NEW_USAGE, cli::REMOTE_USAGE);
            std::process::ExitCode::SUCCESS
        }
        Some("notify") => notify(&args[1..]),
        Some("tag") => tag(&args[1..]),
        Some("new") => new_cli(&args[1..]),
        Some("attach") => attach(&args[1..]),
        Some("shell-hook") => match shellhook::text(args.get(1).map(String::as_str)) {
            Ok(text) => {
                print!("{text}");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("tsumugi shell-hook: {e}");
                std::process::ExitCode::from(2)
            }
        },
        Some("--version" | "-V") => {
            println!("tsumugi {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("tsumugi: unknown command `{other}` (server, ls, new, attach, send, read, split, close, wait, notify, tag, shell-hook, help, --version)");
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

/// `tsumugi new [FOLDER] [--tag TAG]... [-- COMMAND...]`: a session in a tab
/// of its own, the server started if need be; prints its number.
fn new_cli(args: &[String]) -> std::process::ExitCode {
    let n = match cli::parse_new(args) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("tsumugi new: {e}\nusage: {}", cli::NEW_USAGE);
            return std::process::ExitCode::from(2);
        }
    };
    match connect(|| {}).and_then(|c| cli::new(&c, n)) {
        Ok(id) => {
            println!("{id}");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tsumugi new: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// The session `tsumugi attach` asked the window to show.
static SHOW_SESSION: std::sync::OnceLock<SessionId> = std::sync::OnceLock::new();

/// `tsumugi attach NAME`: open the window on a session, by its number,
/// folder, program or tag. The sessions live in the server whether a window
/// is open or not, so attaching is opening a window there.
fn attach(args: &[String]) -> std::process::ExitCode {
    let [name] = args else {
        eprintln!("usage: tsumugi attach NUMBER|NAME (tsumugi ls lists them)");
        return std::process::ExitCode::from(2);
    };
    let found = Client::connect(&Address::for_user(), || {}).map_err(|e| format!("no server ({e})")).and_then(|c| c.list().map_err(|e| e.to_string())).and_then(|list| cli::find(&list, name));
    match found {
        Ok(id) => {
            let _ = SHOW_SESSION.set(id);
            window()
        }
        Err(e) => {
            eprintln!("tsumugi attach: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn ls(args: &[String]) -> std::process::ExitCode {
    let listed = Client::connect(&Address::for_user(), || {}).and_then(|c| c.list());
    match listed {
        Ok(list) if args.iter().any(|a| a == "--json") => {
            print!("{}", cli::ls_json(&list));
            std::process::ExitCode::SUCCESS
        }
        Ok(list) => {
            for i in list {
                println!("{}\t{}\t{}\t{}\t{}\t{}\t{}", i.id, i.state.word(), i.command, i.cwd.display(), i.title, i.note, i.tags.join(","));
            }
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tsumugi ls: no server ({e})");
            std::process::ExitCode::FAILURE
        }
    }
}

/// The remote control (`cli::REMOTE_USAGE`): send, read, split, close and
/// wait on a session from another program, or an AI in another pane.
fn remote(what: &str, args: &[String]) -> std::process::ExitCode {
    use std::process::ExitCode;
    let fail = |e: String| {
        eprintln!("tsumugi {what}: {e}");
        ExitCode::FAILURE
    };
    let Some(who) = args.iter().find(|a| !a.starts_with('-')).cloned() else {
        eprintln!("usage:\n{}", cli::REMOTE_USAGE);
        return ExitCode::from(2);
    };
    let client = match Client::connect(&Address::for_user(), || {}) {
        Ok(c) => c,
        Err(e) => return fail(format!("no server ({e})")),
    };
    let list = match client.list() {
        Ok(l) => l,
        Err(e) => return fail(e.to_string()),
    };
    let id = match cli::find(&list, &who) {
        Ok(id) => id,
        // Waiting on a session that has already ended: its own exit code.
        Err(_) if what == "wait" && who.parse::<SessionId>().is_ok() => {
            eprintln!("tsumugi wait: session {who} ended");
            return ExitCode::from(3);
        }
        Err(e) => return fail(e),
    };
    match what {
        "send" => {
            let rest: Vec<String> = args.iter().skip_while(|a| **a != who).skip(1).cloned().collect();
            if rest.is_empty() {
                return fail("nothing to send".into());
            }
            client.send_prompt(id, rest.join(" "));
            // Sent before this process ends: a question after it is answered
            // once the server has it.
            match client.list() {
                Ok(_) => ExitCode::SUCCESS,
                Err(e) => fail(e.to_string()),
            }
        }
        "read" => {
            let r = match cli::parse_read(args) {
                Ok(r) => r,
                Err(e) => return fail(e),
            };
            client.all_text(id);
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                if let Some((_, text)) = client.take_texts().into_iter().find(|(i, _)| *i == id) {
                    print!("{}", if r.all { text } else { cli::last_lines(&text, r.lines) });
                    return ExitCode::SUCCESS;
                }
                if std::time::Instant::now() > deadline {
                    return fail("the server did not answer".into());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        "split" => match cli::parse_split(args).and_then(|s| cli::split(&client, &list, id, &s)) {
            Ok(new) => {
                println!("{new}");
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        "close" => {
            client.attach(id).kill();
            match client.list() {
                Ok(_) => ExitCode::SUCCESS,
                Err(e) => fail(e.to_string()),
            }
        }
        _ => {
            let w = match cli::parse_wait(args) {
                Ok(w) => w,
                Err(e) => return fail(e),
            };
            let start = std::time::Instant::now();
            loop {
                let Ok(list) = client.list() else { return fail("the server went away".into()) };
                let Some(info) = list.iter().find(|i| i.id == id) else {
                    eprintln!("tsumugi wait: session {id} ended");
                    return ExitCode::from(3);
                };
                if w.states.contains(&info.state) {
                    println!("{}", info.state.word());
                    return ExitCode::SUCCESS;
                }
                if w.timeout.is_some_and(|t| start.elapsed() >= t) {
                    eprintln!("tsumugi wait: still {} after {:?}", info.state.word(), w.timeout.unwrap_or_default());
                    return ExitCode::FAILURE;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
}

/// `tsumugi notify [--state waiting|done|error] [--session N] [MESSAGE...]`:
/// mark a session for the sidebar. Meant for an agent's hooks, run inside
/// the session, which is where `TSUMUGI_SESSION` and `TSUMUGI_ADDRESS` are
/// set. Claude Code's `Notification` hook is `tsumugi notify --stdin`, its
/// `Stop` hook `tsumugi notify --state done`. Outside a session a hook's
/// (stdin not a terminal) does nothing and says nothing: Claude Code run in
/// another terminal shows a hook's error on every reply, and takes a `Stop`
/// hook's exit code 2 as "do not stop yet".
fn notify(args: &[String]) -> std::process::ExitCode {
    let mut state = tsumugi_mux::State::Waiting;
    let mut session = std::env::var("TSUMUGI_SESSION").ok().and_then(|s| s.parse().ok());
    let mut words = Vec::new();
    let mut claude = None;
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
                // Its conversation, for `claude --resume` after a restart.
                claude = json_string(&input, "session_id");
            }
            // Codex's `notify` program gets its event as one JSON argument:
            // a turn finished, and what it said last.
            a if a.trim_start().starts_with('{') => {
                if let Some((done, said)) = codex_event(a) {
                    if done {
                        state = tsumugi_mux::State::Done;
                    }
                    words.push(said);
                }
            }
            _ => words.push(a.clone()),
        }
    }
    let Some(id) = session else {
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            return std::process::ExitCode::SUCCESS;
        }
        eprintln!("tsumugi notify: not inside a tsumugi session (no TSUMUGI_SESSION); give --session N");
        return std::process::ExitCode::from(2);
    };
    match Client::connect(&Address::for_user(), || {}).and_then(|c| c.notify(id, state, words.join(" "), claude)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tsumugi notify: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Codex's notice (`{"type":"agent-turn-complete","last-assistant-message":…}`):
/// whether a turn finished, and the first line of what it said last (cut to
/// 200 characters). `None` for JSON that is not one.
fn codex_event(text: &str) -> Option<(bool, String)> {
    let v = json::Json::parse(text).ok()?;
    let kind = v.get("type")?.string()?;
    let said = v.get("last-assistant-message").and_then(json::Json::string).unwrap_or_default();
    let first: String = said.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().trim().chars().take(200).collect();
    Some((kind == "agent-turn-complete", first))
}

/// `tsumugi tag [--session N] [--remove] [TAG...]`: put tags on a session
/// (by default the one it runs in), take them off, or with no tag print the
/// session's tags, one a line.
fn tag(args: &[String]) -> std::process::ExitCode {
    let mut session = std::env::var("TSUMUGI_SESSION").ok().and_then(|s| s.parse().ok());
    let mut on = true;
    let mut tags = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--session" => session = it.next().and_then(|s| s.parse().ok()),
            "--remove" | "-r" => on = false,
            _ => tags.push(a.clone()),
        }
    }
    let Some(id) = session else {
        eprintln!("tsumugi tag: not inside a tsumugi session (no TSUMUGI_SESSION); give --session N");
        return std::process::ExitCode::from(2);
    };
    let done = Client::connect(&Address::for_user(), || {}).and_then(|c| {
        for t in tags.iter().filter_map(|t| tsumugi_mux::proto::tag_name(t)) {
            c.tag(vec![id], t, on);
        }
        // Asked after the tags, so it answers once they are on.
        c.list()
    });
    match done {
        Ok(list) => match list.into_iter().find(|i| i.id == id) {
            Some(i) => {
                if tags.is_empty() {
                    for t in i.tags {
                        println!("{t}");
                    }
                }
                std::process::ExitCode::SUCCESS
            }
            None => {
                eprintln!("tsumugi tag: no session {id}");
                std::process::ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("tsumugi tag: {e}");
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
    // Before the window shows, so the taskbar files it under this name.
    alert::name_process();
    let first = tsumugi_mux::settings::default_path().and_then(|p| tsumugi_mux::settings::load(&p).ok()).unwrap_or_default();
    // The design's logo as the window's and the taskbar's icon.
    let icon = egui::IconData { rgba: icon::pixels(256), width: 256, height: 256 };
    let mut viewport = egui::ViewportBuilder::default().with_title("tsumugi").with_inner_size([960.0, 600.0]).with_min_inner_size([420.0, 260.0]).with_icon(icon);
    if first.window.own_titlebar() {
        // macOS keeps its traffic lights, over the band; elsewhere the band
        // draws its own buttons (chrome::top_band).
        viewport = if cfg!(target_os = "macos") {
            viewport.with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false)
        } else {
            viewport.with_decorations(false)
        };
    }
    // Material, or less than all of the window over the desktop: the
    // window is made see-through when it opens.
    if ((cfg!(windows) || cfg!(target_os = "macos")) && first.window.material != "none") || first.window.opacity < 100 {
        viewport = viewport.with_transparent(true);
    }
    let options = eframe::NativeOptions { viewport, wgpu_options: wgpu_options(&first.advanced.backend), ..Default::default() };
    match eframe::run_native("tsumugi", options, Box::new(|cc| Ok(Box::new(App::new(cc))))) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tsumugi: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// GL first on Windows, where Vulkan and DX12 spin a core on AMD for an idle
/// window (filer #232); `[advanced] backend` names another, and
/// `WGPU_BACKEND` overrides both, as it does for wgpu itself.
fn wgpu_options(backend: &str) -> eframe::WgpuConfiguration {
    use eframe::egui_wgpu::WgpuSetup;
    let mut options = eframe::WgpuConfiguration::default();
    if std::env::var_os("WGPU_BACKEND").is_some_and(|v| !v.is_empty()) {
        return options;
    }
    let named = (backend != "auto").then_some(backend);
    let picked = match tsumugi_pane::gpu::pick_backends(named, cfg!(windows), tsumugi_pane::gpu::has_adapter) {
        Ok(b) => b,
        Err(auto) => {
            eprintln!("tsumugi: no `{backend}` adapter on this machine; drawing with the automatic choice");
            auto
        }
    };
    if let (Some(backends), WgpuSetup::CreateNew(setup)) = (picked, &mut options.wgpu_setup) {
        setup.instance_descriptor.backends = backends;
    }
    options
}

/// Connect to this user's server, starting one if none answers.
fn connect(wake: impl Fn() + Send + Sync + Clone + 'static) -> Result<Client, String> {
    let at = Address::for_user();
    match Client::connect(&at, wake.clone()) {
        Ok(c) => return Ok(c),
        // A server of another version answers: no new one can start beside
        // it. Said apart, so the window offers to stop it.
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => return Err(format!("{OTHER_VERSION}{e}")),
        Err(_) => {}
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

/// The offer to add Claude Code's hooks (the design's First run): ringed in
/// gold, what it adds, and the three answers. `Some(0)` add, `1` not now,
/// `2` never.
/// `full`: with the hooks' lines, as on the first-run screen; else short,
/// for the corner, over the panes.
fn hooks_card(ui: &mut egui::Ui, width: f32, full: bool) -> Option<u8> {
    let c = theme::colors();
    let mut answer = None;
    egui::Frame::NONE.fill(c.panel).stroke(egui::Stroke::new(1.0, c.wait.gamma_multiply(0.5))).corner_radius(12.0).inner_margin(egui::Margin::symmetric(20, 18)).show(ui, |ui| {
        ui.set_width(width - 40.0);
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                ui.painter().circle_filled(dot.center(), 4.0, c.wait);
                ui.label(egui::RichText::new("Let Claude Code tell tsumugi when it waits").size(15.0).strong().color(c.strong()));
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Adds two hooks to ~/.claude/settings.json. Without them tsumugi still guesses from quiet output, but a hook is certain and instant, and lets a restart resume the conversation.").size(13.0).color(c.dim));
            ui.add_space(6.0);
            if full {
                egui::Frame::NONE.fill(c.side).corner_radius(8.0).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let [notification, stop] = hooks::commands();
                    ui.label(egui::RichText::new(format!("\"Notification\": [{{ \"command\": {notification:?} }}]\n\"Stop\":         [{{ \"command\": {stop:?} }}]")).font(egui::FontId::monospace(12.0)).color(c.fg));
                });
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let add = egui::Button::new(egui::RichText::new("Add the hooks").color(c.on_accent()).strong()).fill(c.wait).min_size(egui::vec2(0.0, 34.0));
                if ui.add(add).clicked() {
                    answer = Some(0);
                }
                if ui.add(egui::Button::new("Not now").min_size(egui::vec2(0.0, 34.0))).clicked() {
                    answer = Some(1);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("Don't ask again").size(12.5).color(c.dim)).frame(false)).clicked() {
                        answer = Some(2);
                    }
                });
            });
        });
    });
    answer
}

/// What a settings job says when it is done: words for the toast, or why
/// it failed.
type JobDone = Result<String, String>;
/// Files to attach to a session's draft, with their sizes (a thread's work).
type Attach = (SessionId, Vec<(std::path::PathBuf, u64)>);

/// How a failure to connect to a server of another version starts.
const OTHER_VERSION: &str = "A tsumugi server of another version is running: ";

/// Stop the server of another version and start this version's, on a thread
/// of its own (it can take seconds). A server from before tsumugi 0.19
/// does not know how to be asked, and is said so.
fn replace_server(wake: impl Fn() + Send + Sync + Clone + 'static) -> std::sync::mpsc::Receiver<Result<Client, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = std::thread::Builder::new().name("replace-server".into()).spawn(move || {
        let at = Address::for_user();
        let _ = Client::stop(&at);
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        let stopped = loop {
            match Client::connect(&at, || {}) {
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {}
                _ => break true,
            }
            if std::time::Instant::now() > deadline {
                break false;
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        let answer = if stopped {
            connect(wake)
        } else {
            let by_hand = if cfg!(windows) { "Stop-Process -Name tsumugi" } else { "pkill -f 'tsumugi server'" };
            Err(format!("The other server did not stop: it is from before tsumugi 0.19, which cannot be asked. Stop it by hand ({by_hand}) and open tsumugi again; its tabs come back."))
        };
        let _ = tx.send(answer);
    });
    rx
}

/// `[general] restart_after_update`, read from the file: with no server to
/// talk to, the window has not read the settings yet.
fn restart_after_update() -> bool {
    tsumugi_mux::settings::default_path().and_then(|p| tsumugi_mux::settings::load(&p).ok()).is_some_and(|s| s.general.restart_after_update)
}

/// What the server has to show. After a restart (no session, but a saved
/// state) that is the "Welcome back" screen, or the saved tabs at once when
/// it was told not to ask; else nothing, and the window shows "Start your
/// first session" (the design's First run) rather than a shell nobody asked
/// for.
fn first_session(client: &Client) -> Result<Option<chrome::RestoreView>, String> {
    let list = client.list().map_err(|e| e.to_string())?;
    if !list.is_empty() {
        return Ok(None);
    }
    if let Some(saved) = client.saved().ok().flatten().filter(|s| s.workspaces.iter().any(|w| !w.panes.is_empty())) {
        if !always_restore() {
            return Ok(Some(chrome::RestoreView::new(saved)));
        }
        if client.restore().unwrap_or(0) > 0 {
            return Ok(None);
        }
    }
    Ok(None)
}

fn start_here(client: &Client) -> Result<RemotePane, String> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    new_session(client, cwd, Place::NewWorkspace)
}

/// "Always restore without asking", kept as a file beside the state.
fn always_restore_file() -> Option<std::path::PathBuf> {
    tsumugi_mux::state::default_path().map(|p| p.with_file_name("always-restore"))
}

fn always_restore() -> bool {
    always_restore_file().is_some_and(|p| p.exists())
}

/// The sidebar's own choices, kept beside the state file between runs: a
/// window's preferences, not settings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct View {
    sort: sort::Sort,
    density: sort::Density,
    /// The narrow rail instead of the sidebar (the design's 1i A).
    rail: bool,
    /// Asked once, past twelve tabs, whether to go to one line each.
    asked: bool,
    /// Told not to offer Claude Code's hooks again.
    hooks_asked: bool,
}

impl View {
    fn file() -> Option<std::path::PathBuf> {
        tsumugi_mux::state::default_path().map(|p| p.with_file_name("view"))
    }

    /// Read at the start; the order alone was kept in `sort` before v0.15.
    fn load() -> Self {
        let read = |p: Option<std::path::PathBuf>| p.and_then(|p| std::fs::read_to_string(p).ok());
        match read(Self::file()) {
            Some(text) => Self::parse(&text),
            None => {
                let sort = read(tsumugi_mux::state::default_path().map(|p| p.with_file_name("sort")));
                Self { sort: sort.and_then(|w| sort::Sort::from_word(w.trim())).unwrap_or_default(), ..Self::default() }
            }
        }
    }

    fn parse(text: &str) -> Self {
        let mut v = Self::default();
        for (k, val) in text.lines().filter_map(|l| l.split_once('=')) {
            match (k.trim(), val.trim()) {
                ("sort", w) => v.sort = sort::Sort::from_word(w).unwrap_or_default(),
                ("density", w) => v.density = sort::Density::from_word(w).unwrap_or_default(),
                ("rail", w) => v.rail = w == "yes",
                ("asked", w) => v.asked = w == "yes",
                ("hooks_asked", w) => v.hooks_asked = w == "yes",
                _ => {}
            }
        }
        v
    }

    fn text(self) -> String {
        let yes = |b: bool| if b { "yes" } else { "no" };
        format!("sort={}\ndensity={}\nrail={}\nasked={}\nhooks_asked={}\n", self.sort.word(), self.density.word(), yes(self.rail), yes(self.asked), yes(self.hooks_asked))
    }

    /// Written on a thread of its own: no disk on the window's thread.
    fn save(self) {
        let _ = std::thread::Builder::new().name("view".into()).spawn(move || {
            let Some(p) = Self::file() else { return };
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, self.text());
        });
    }
}

fn set_always_restore(on: bool) {
    let Some(p) = always_restore_file() else { return };
    if on {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, b"");
    } else {
        let _ = std::fs::remove_file(p);
    }
}

fn new_session(client: &Client, cwd: std::path::PathBuf, place: Place) -> Result<RemotePane, String> {
    let shell = None;
    client.spawn_at(cwd, shell, Size::new(80, 24), (8, 16), place).map_err(|e| format!("the shell did not start: {e}"))
}

/// The sidebar's width when the window first opens, and how far a drag may
/// take it (docs/v1-scope.md 1f).
const SIDEBAR: f32 = 288.0;
/// Dragged narrower than `RAIL_AT`, the sidebar becomes the rail, and the
/// rail dragged wider than it becomes the sidebar again (the design's 1i A).
const SIDEBAR_RANGE: std::ops::RangeInclusive<f32> = 100.0..=480.0;
const RAIL: f32 = 60.0;
const RAIL_AT: f32 = 120.0;
/// The gap between split panes, and how wide a strip around it a drag can
/// start in (1f: 12px).
const GAP: f32 = 8.0;
/// The room round the panes, as round each card of the design's Main.
const MARGIN: f32 = 8.0;
const GRAB: f32 = 12.0;
/// How much a pane without the keys is dimmed (1e).
/// (now `[appearance] dim` in the settings)
/// The heading over each pane of a split.
const HEADER: f32 = 30.0;

/// The panes' letter size: the settings' with `step` points added, kept to
/// what the settings allow.
fn font_size(base: f32, step: f32) -> f32 {
    (base + step).clamp(8.0, 32.0)
}

/// Tab and Shift+Tab are the shell's while no control of the window has
/// the keys (lazygit's panels, a shell's completion): egui would hand the
/// first button the focus on them, and the pane would then get no keys at
/// all until it was clicked. `own`: a screen of the window's own is open,
/// whose Tab moves between its controls. Before anything is drawn.
fn keep_tab_for_pane(ctx: &egui::Context, own: bool) {
    if !own && ctx.memory(|m| m.focused().is_none()) {
        ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
    }
}

/// Keeps the keys in a field that should hold them while it is shown.
/// Asking again on every frame would end an IME composition each time
/// (`request_focus` interrupts it), so Japanese could never be typed: ask
/// only when the field does not have them, and not on the frame it let them
/// go, which its owner reads as leaving.
fn keep_focus(field: &egui::Response) {
    if !field.has_focus() && !field.lost_focus() {
        field.request_focus();
    }
}

/// Whether Claude Code's hooks are in, and the file when they were pointed
/// here (`hooks::repair_on_disk`).
type HooksFound = (bool, Result<Option<std::path::PathBuf>, String>);

struct App {
    client: Option<Client>,
    /// Why there is nothing to show, shown in its place.
    failed: Option<String>,
    /// The tab shown.
    active: Option<WorkspaceId>,
    /// A session just started, to show once the server has placed it.
    pending: Option<SessionId>,
    /// A pane sent to a tab of its own with the keys, the tab it left and
    /// when: its new tab is shown once the server has made it.
    following: Option<(SessionId, WorkspaceId, std::time::Instant)>,
    /// The WSL distributions and SSH hosts the new-session dialog offers,
    /// looked for again each time it opens.
    places: std::sync::Arc<std::sync::Mutex<Vec<remote::Where>>>,
    /// The panes on screen, attached; the rest are not sent here.
    panes: HashMap<SessionId, RemotePane>,
    views: HashMap<SessionId, ViewState>,
    /// The pane with the keys shown alone (`Ctrl+Shift+Z`).
    zoom: bool,
    /// A divider being dragged: the tab's shape as the drag has it so far.
    dragging: Option<(WorkspaceId, Node<SessionId>)>,
    /// Whether the card's note and name fields were drawn this frame.
    fields_drawn: (bool, bool),
    /// A pane being carried by its header to another place in its tab.
    moving: Option<(WorkspaceId, SessionId)>,
    palette: Palette,
    font: egui::FontId,
    /// Points added to the settings' size by `Ctrl+=` / `Ctrl+-`, for this
    /// window only (`Ctrl+0` takes them away).
    font_step: f32,
    /// Copy mode, on the pane it was started in.
    copy_mode: Option<(SessionId, copymode::CopyMode)>,
    /// The find bar, on the pane it finds in (`Ctrl+Shift+F`).
    find: Option<find::Bar>,
    /// A paste waiting for its answer (`paste::why`).
    paste_ask: Option<paste::Held>,
    /// The saved layouts (`layouts.toml`), and one being opened: its panes
    /// as they start, set in its shape once all are in one tab.
    layouts: Vec<layouts::Layout>,
    opening: Option<(Node<SessionId>, SessionId, std::time::Instant)>,
    /// What the window title was last set to, so it is set only on a change.
    title: String,
    /// The server has had a tab since this window opened: when the last one
    /// goes, so does the window.
    had_tabs: bool,
    /// The bell's list is open, and where.
    bell_open: Option<egui::Pos2>,
    /// Where the bell's list goes: under the bell, as last drawn.
    bell_anchor: egui::Pos2,
    /// After a restart: the "Welcome back" screen, until it is answered.
    restore: Option<chrome::RestoreView>,
    /// The sidebar's "Jump to waiting" was pressed.
    jump_waiting: bool,
    /// A Nerd Font is installed: the branch mark is its character, not drawn.
    nerd: bool,
    /// Commits not pushed and the pull request, for the status bar (1d).
    git: gitinfo::Watcher,
    /// What each tab has changed and not committed (B5).
    changes: gitinfo::Watcher,
    /// Claude Code's tokens, today's and each conversation's (1d).
    usage: usage::Watcher,
    /// The window has no system title bar: the band is it. On macOS fixed
    /// when the window opens; elsewhere it follows the settings.
    own_frame: bool,
    /// The system's frame is shown (not on macOS, where it stays as opened).
    system_frame: bool,
    /// The desktop shows through the chrome (Mica or Acrylic, Windows 11).
    material: bool,
    /// The window was opened see-through for `[window] opacity`: it follows
    /// the setting from then on.
    transparent: bool,
    /// `[window] image`, behind the panes.
    backdrop: backdrop::Backdrop,
    /// `[window] quake`, the key held from any program.
    quake: quake::Quake,
    /// Each session's folder as last seen, and the folders of those that
    /// ended (the new-session dialog's "closed" ones).
    known_cwds: HashMap<SessionId, std::path::PathBuf>,
    closed: Vec<std::path::PathBuf>,
    /// The tab and panes shown last frame, and when the tab changed or a
    /// pane appeared: they fade in (the short moves of v1-scope's look).
    shown_tab: Option<WorkspaceId>,
    shown_panes: std::collections::HashSet<SessionId>,
    tab_at: Option<std::time::Instant>,
    pane_at: HashMap<SessionId, std::time::Instant>,
    /// The session whose card the pointer is on: watched for its preview.
    peek: Option<SessionId>,
    /// Claude Code's hooks: being looked for (and pointed here when theirs
    /// is gone, `hooks::repair_on_disk`), offered, being added.
    hooks_check: Option<std::sync::mpsc::Receiver<HooksFound>>,
    hooks_offered: bool,
    hooks_adding: Option<std::sync::mpsc::Receiver<Result<std::path::PathBuf, String>>>,
    /// Prompts to send when their sessions are done, oldest first; and the
    /// sessions just sent one, held until they have run again.
    queue: Vec<Queued>,
    queue_hold: HashMap<SessionId, std::time::Instant>,
    /// Sessions waiting, watched for a question on their screens this frame.
    watching: Vec<SessionId>,
    /// The worktrees tsumugi made; one being made, with what to start in
    /// it; those a session has been seen in; the one whose last session
    /// ended, to ask about; one being removed.
    worktrees: Vec<std::path::PathBuf>,
    /// For threads to wake the window with.
    ctx: egui::Context,
    worktree_making: Option<(std::sync::mpsc::Receiver<Result<std::path::PathBuf, String>>, newsession::Create)>,
    worktree_lived: std::collections::HashSet<std::path::PathBuf>,
    worktree_ask: Option<std::path::PathBuf>,
    /// The window was asked to close while sessions were running: how many,
    /// until the question is answered.
    closing: Option<usize>,
    /// What the input box sent this frame, from inside the pane.
    input_sent: Option<inputbox::Send>,
    /// The input box's height last frame, to keep its room in the pane.
    input_h: f32,
    /// Files for the input box with their sizes, read on threads.
    attach: (std::sync::mpsc::Sender<Attach>, std::sync::mpsc::Receiver<Attach>),
    /// The tab being named by F2, in a field on its card, and the words.
    renaming_card: Option<(WorkspaceId, String)>,
    /// The tab whose note is being written on its card, and the text so far.
    noting_card: Option<(WorkspaceId, String)>,
    /// The waiting list or the recently closed, over the window.
    lists: Option<lists::View>,
    /// The help (F1), while it is open.
    help: Option<help::View>,
    /// The sessions that ended, newest first (`history.rs`).
    ended: Vec<history::Closed>,
    /// A tab's changes not committed, over the window.
    diff: Option<diffview::View>,
    /// A menu of the window's was open as the frame began.
    menu_open: bool,
    /// "Start in parallel", while open; and the worktrees being made for it,
    /// each with its prompt, as they are made.
    parallel: Option<parallel::View>,
    parallel_made: Option<std::sync::mpsc::Receiver<Result<(std::path::PathBuf, String), String>>>,
    /// Sessions whose whole buffer was asked for, to save, by name.
    saving: HashMap<SessionId, String>,
    /// Tabs where what is typed goes to every pane (`Ctrl+Shift+I`).
    typing_all: std::collections::HashSet<WorkspaceId>,
    /// The hooks were offered on the first-run screen.
    hooks_seen_first: bool,
    /// Where the sidebar (or the rail) is this frame: a pane dropped there
    /// becomes a tab of its own.
    side_rect: Option<egui::Rect>,
    /// The first-run screen was drawn this frame (its hooks' card is in it).
    first_shown: bool,
    /// What the settings screen says of the machine, and its reading.
    facts: Option<facts::Facts>,
    facts_rx: Option<std::sync::mpsc::Receiver<facts::Facts>>,
    /// The settings screen's work on threads (hooks, export, starting at
    /// sign-in): what to say when each is done.
    jobs: (std::sync::mpsc::Sender<JobDone>, std::sync::mpsc::Receiver<JobDone>),
    /// The check for a newer release, once a start (`[general] check_updates`).
    update: Option<std::sync::mpsc::Receiver<Option<String>>>,
    update_asked: bool,
    /// The question was answered "Close": the next request goes through.
    close_ok: bool,
    worktree_removing: Option<(std::path::PathBuf, std::sync::mpsc::Receiver<Result<(), String>>)>,
    /// A few words above the status bar for a few seconds: what was done,
    /// or (true) what went wrong.
    toast: Option<(String, std::time::Instant, bool)>,
    /// `[font] family` as installed, and the file it found.
    font_family: String,
    font_file: Option<std::path::PathBuf>,
    /// The font file's ligatures, when it has a file (Q8).
    shaper: Option<std::sync::Arc<tsumugi_pane::Shaper>>,
    /// Bold, italic and bold italic faces in use; and as installed for the
    /// next frame (egui takes new fonts a frame late, and a face named
    /// before it has them is a panic).
    faces_found: [bool; 3],
    faces_next: [bool; 3],
    /// Fonts being read for a new `[font] family`.
    fonts_rx: Option<std::sync::mpsc::Receiver<fonts::Loaded>>,
    /// The bell list opened this frame: the click that opened it is not a
    /// click outside it.
    bell_opening: bool,
    /// Telling someone who is not looking (the design's 1h).
    alerts: alert::Alerts,
    /// `[notify] spend_day` and `spend_block`, each told once.
    spend: spend::Watch,
    teller: alert::Teller,
    /// Whether the server was last told this window has the keyboard.
    focus_sent: Option<bool>,
    /// What the sidebar shows, and in what order (the design's 1a and 1b).
    filter: sort::Filter,
    view: View,
    /// A tab being dragged to another place in the sidebar.
    dragging_tab: Option<WorkspaceId>,
    /// The search box, while it is open (the design's 1c).
    search: Option<palette::View>,
    /// What is being typed into a tab menu's "Add a tag".
    tag_input: String,
    /// `settings.toml` as read again whenever it changes, and what is wrong
    /// with it.
    settings: std::sync::mpsc::Receiver<Read>,
    /// For a write of the settings to say what it wrote at once.
    settings_tx: std::sync::mpsc::Sender<Read>,
    /// The settings as last read, for the settings screen and the clock.
    settings_now: tsumugi_mux::settings::Settings,
    /// The settings screen, while it is open (the design's 1m).
    prefs: Option<prefs::Screen>,
    /// The input box below the panes (the design's 12, 1l).
    input: inputbox::InputBox,
    /// Stopping a server of another version, and its answer.
    replacing: Option<std::sync::mpsc::Receiver<Result<Client, String>>>,
    /// Whether to replace the older server without asking
    /// (`restart_after_update`) has been looked at: once, so a failure is
    /// shown, not tried again and again.
    replaced_unasked: bool,
    /// Something drawn this frame moves (a breathing ring, a running line):
    /// draw again soon. Nothing moving, nothing drawn until there is news.
    animated: bool,
    /// `TSUMUGI_KEYLOG` is set: print the key presses.
    key_log: bool,
    /// What the key log last said had the keys.
    focus_logged: Option<egui::Id>,
    settings_error: Option<String>,
    /// From the settings and `profiles.toml`, for the new-session dialog.
    tag_rules: Vec<tsumugi_mux::settings::TagRule>,
    profiles: Vec<tsumugi_mux::settings::Profile>,
    /// The new-session dialog, while it is open (the design's 1g).
    new_session: Option<newsession::Dialog>,
    /// The theme (1k): what the settings chose, the theme files, and what
    /// was last put in force.
    theme_choice: (String, String, String),
    theme_files: ThemeFiles,
    /// What is wrong with the theme files as read, and with the theme as
    /// chosen (worked out each frame).
    theme_file_error: Option<String>,
    theme_error: Option<String>,
    theme_applied: Option<theme::Colors>,
    /// The settings' `[open]` and `[menu]`, for the tab's menu (1j).
    open: tsumugi_mux::settings::Open,
    menu: tsumugi_mux::settings::Menu,
    /// Bumped when the sidebar turns into the rail or back, so each starts
    /// at its own width.
    side_gen: u32,
    /// Folders sorted by: the groups closed.
    closed_groups: Vec<std::path::PathBuf>,
    /// A tab being renamed in its menu, and the name so far.
    renaming: Option<(WorkspaceId, String)>,
    /// "Close" was clicked once on a tab with something running.
    close_armed: Option<WorkspaceId>,
}

/// The tab menu's "Open in the editor" (or filer): a click runs the first
/// of `[open]`'s commands; with more than one, ▶ opens them all beside it
/// (as Sakura Editor's menus do), the first still a click away.
fn open_with(ui: &mut egui::Ui, words: &str, commands: &[String], focus: &Info) {
    let run = |c: &String| menu::run(tsumugi_mux::settings::fill(c, &focus.cwd, focus.id));
    match commands {
        [] => {
            ui.add_enabled(false, egui::Button::new(words)).on_disabled_hover_text("None set: Settings → Sessions → Open with");
        }
        [one] => {
            if ui.button(words).on_hover_text(one).clicked() {
                run(one);
                ui.close();
            }
        }
        more => {
            let sub = ui.menu_button(words, |ui| {
                for c in more {
                    if ui.button(tsumugi_mux::settings::program_name(c)).on_hover_text(c).clicked() {
                        run(c);
                        ui.close();
                    }
                }
            });
            // The item itself runs the first, as one alone would; the list
            // opens on the pointer resting on it.
            if sub.response.clicked() {
                run(&more[0]);
                ui.close();
            }
        }
    }
}

/// A tab's row past its first lines (`App::card_lines`).
struct CardLines<'t> {
    card: bool,
    tags: Vec<&'t String>,
    numbers: Option<String>,
    ports: Vec<u16>,
    other_agent: Option<String>,
    noting: bool,
    note_line: bool,
    run_line: bool,
    /// 17 when the third line has nothing to say, and the card is the
    /// shorter by it.
    closed_up: f32,
}

impl CardLines<'_> {
    /// Down to the lines under the tags.
    fn base(&self) -> f32 {
        if !self.card {
            28.0
        } else if self.tags.is_empty() {
            62.0 - self.closed_up
        } else {
            82.0 - self.closed_up
        }
    }

    /// How tall the row is: a line 28; a card 62, 82 with tags, and 18 more
    /// for each line under them.
    fn height(&self) -> f32 {
        self.base() + 18.0 * (usize::from(self.note_line) + usize::from(self.run_line) + usize::from(self.numbers.is_some())) as f32
    }
}

/// A change asked for from the sidebar, made once it is drawn.
enum SideOp {
    Mute(Vec<SessionId>, bool),
    Tag(Vec<SessionId>, String, bool),
    MuteTag(String, bool),
    Move(WorkspaceId, usize),
    Rename(WorkspaceId, String),
    Note(WorkspaceId, String),
    Pin(WorkspaceId, bool),
    Restart(SessionId),
    /// A new tab in this folder; Claude Code in it when it ran here.
    Duplicate(std::path::PathBuf, bool),
    Close(Vec<SessionId>),
    /// Show the changes not committed in this folder, under this name.
    Diff(String, std::path::PathBuf),
    /// Ask for the session's whole buffer, to save it under this name.
    SaveOutput(SessionId, String),
    /// Push the branch in this folder and open a pull request for it.
    CreatePr(std::path::PathBuf),
}

/// Where in the server's order a tab dropped at `gap` among the rows shown
/// goes (the server takes it out, then puts it in); `None` when it stays.
fn drop_index(workspaces: &[Workspace], rows: &[(WorkspaceId, egui::Rect)], dragged: WorkspaceId, gap: usize) -> Option<usize> {
    let at = |id: WorkspaceId| workspaces.iter().position(|w| w.id == id);
    let from = at(dragged)?;
    let mut to = match rows.get(gap) {
        Some((id, _)) => at(*id)?,
        None => at(rows.last()?.0)? + 1,
    };
    if from < to {
        to -= 1;
    }
    (to != from).then_some(to)
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // The font before the first frame, so the panes do not start in
        // another and jump; later changes load on a thread.
        fonts::scan_names();
        let first_settings = tsumugi_mux::settings::default_path().and_then(|p| tsumugi_mux::settings::load(&p).ok()).unwrap_or_default();
        let (first_font, first_window) = (first_settings.font.clone(), first_settings.window.clone());
        let loaded = fonts::load(&first_font.family);
        cc.egui_ctx.set_fonts(loaded.defs);
        let nerd = loaded.nerd;
        let (settings_tx, settings_rx) = std::sync::mpsc::channel();
        watch_settings(cc.egui_ctx.clone(), settings_tx.clone());
        // tsumugi Dark until the settings are read (the first frame).
        let first = theme::colors();
        let palette = first.palette();
        cc.egui_ctx.set_visuals(first.visuals());
        // egui zooms on Ctrl +/-/0 by itself; in a terminal those are keys.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);

        let git_ctx = cc.egui_ctx.clone();
        let git = gitinfo::Watcher::new(true, move || git_ctx.request_repaint());
        let usage_ctx = cc.egui_ctx.clone();
        let usage = usage::Watcher::start(move || usage_ctx.request_repaint());
        let changes_ctx = cc.egui_ctx.clone();
        let changes = gitinfo::Watcher::new(false, move || changes_ctx.request_repaint());
        let ctx = cc.egui_ctx.clone();
        let (client, failed, restore) = match connect(move || ctx.request_repaint()) {
            Ok(client) => match first_session(&client) {
                Ok(restore) => (Some(client), None, restore),
                Err(e) => (Some(client), Some(e), None),
            },
            Err(e) => (None, Some(e), None),
        };
        Self {
            client,
            failed,
            // A window opened by "Move to a new window" shows that tab.
            active: std::env::var(menu::SHOW_TAB).ok().and_then(|v| v.parse().ok()),
            pending: SHOW_SESSION.get().copied(),
            following: None,
            places: Default::default(),
            panes: HashMap::new(),
            views: HashMap::new(),
            zoom: false,
            dragging: None,
            fields_drawn: (false, false),
            moving: None,
            palette,
            font: egui::FontId::monospace(first_font.size),
            font_step: 0.0,
            copy_mode: None,
            find: None,
            paste_ask: None,
            layouts: layouts::load(),
            opening: None,
            title: String::new(),
            had_tabs: false,
            bell_open: None,
            bell_anchor: egui::pos2(20.0, 60.0),
            restore,
            jump_waiting: false,
            nerd,
            git,
            changes,
            usage,
            own_frame: first_window.own_titlebar(),
            system_frame: !first_window.own_titlebar(),
            material: (cfg!(windows) || cfg!(target_os = "macos")) && first_window.material != "none" && material::apply(cc, &first_window.material, theme::colors().light),
            transparent: first_window.opacity < 100,
            backdrop: backdrop::Backdrop::default(),
            quake: quake::Quake::default(),
            known_cwds: HashMap::new(),
            closed: closed_file().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.lines().filter(|l| !l.is_empty()).map(std::path::PathBuf::from).collect()).unwrap_or_default(),
            shown_tab: None,
            shown_panes: std::collections::HashSet::new(),
            tab_at: None,
            pane_at: HashMap::new(),
            peek: None,
            watching: Vec::new(),
            queue: Vec::new(),
            hooks_check: {
                let (tx, rx) = std::sync::mpsc::channel();
                let ctx = cc.egui_ctx.clone();
                let _ = std::thread::Builder::new().name("hooks".into()).spawn(move || {
                    let repaired = hooks::repair_on_disk();
                    let _ = tx.send((hooks::installed(), repaired));
                    ctx.request_repaint();
                });
                Some(rx)
            },
            hooks_offered: false,
            hooks_adding: None,
            queue_hold: HashMap::new(),
            worktrees: worktree::ours(),
            ctx: cc.egui_ctx.clone(),
            worktree_making: None,
            worktree_lived: std::collections::HashSet::new(),
            worktree_ask: None,
            closing: None,
            input_sent: None,
            input_h: 170.0,
            attach: std::sync::mpsc::channel(),
            renaming_card: None,
            noting_card: None,
            lists: None,
            help: None,
            ended: history::load(),
            diff: None,
            typing_all: std::collections::HashSet::new(),
            saving: HashMap::new(),
            parallel: None,
            parallel_made: None,
            menu_open: false,
            hooks_seen_first: false,
            side_rect: None,
            first_shown: false,
            facts: None,
            facts_rx: None,
            jobs: std::sync::mpsc::channel(),
            update: None,
            update_asked: false,
            close_ok: false,
            worktree_removing: None,
            toast: None,
            font_family: first_font.family.clone(),
            font_file: loaded.file,
            shaper: loaded.shaper,
            faces_found: [false; 3],
            faces_next: loaded.faces,
            fonts_rx: None,
            bell_opening: false,
            alerts: alert::Alerts::default(),
            spend: spend::Watch::default(),
            teller: alert::Teller::start(window_handle(cc), {
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            }),
            focus_sent: None,
            filter: sort::Filter::default(),
            view: View::load(),
            dragging_tab: None,
            search: None,
            tag_input: String::new(),
            settings: settings_rx,
            settings_tx: settings_tx.clone(),
            settings_now: tsumugi_mux::settings::Settings::default(),
            prefs: None,
            input: inputbox::InputBox::with_history(load_history(), prompts::load()),
            key_log: std::env::var_os("TSUMUGI_KEYLOG").is_some(),
            focus_logged: None,
            replacing: None,
            replaced_unasked: false,
            animated: false,
            settings_error: None,
            tag_rules: Vec::new(),
            profiles: Vec::new(),
            new_session: None,
            theme_choice: ("dark".into(), "tsumugi Dark".into(), "tsumugi Light".into()),
            theme_files: ThemeFiles::default(),
            theme_file_error: None,
            theme_error: None,
            theme_applied: None,
            open: Default::default(),
            menu: Default::default(),
            renaming: None,
            side_gen: 0,
            closed_groups: Vec::new(),
            close_armed: None,
        }
    }

    /// The tab shown, following a new session to the tab it landed in.
    fn current(&mut self, workspaces: &[Workspace]) -> Option<Workspace> {
        if let Some((tree, first, since)) = &self.opening {
            let ids = tree.leaves();
            if let (Some(client), Some(w)) = (&self.client, workspaces.iter().find(|w| ids.iter().all(|id| w.layout.contains(id)))) {
                client.set_layout(w.id, tree.clone(), *first);
                self.opening = None;
            } else if since.elapsed() > std::time::Duration::from_secs(10) {
                // A pane that never came: the tab stays as the splits made it.
                self.opening = None;
            }
        }
        if let Some((id, left, since)) = self.following {
            if let Some(w) = workspaces.iter().find(|w| w.id != left && w.layout.contains(&id)) {
                self.active = Some(w.id);
                self.following = None;
            } else if since.elapsed() > std::time::Duration::from_secs(5) {
                self.following = None;
            }
        }
        if let Some(new) = self.pending {
            if let Some(w) = workspaces.iter().find(|w| w.layout.contains(&new)) {
                self.active = Some(w.id);
                self.pending = None;
                // `tsumugi attach` names a pane, maybe not the tab's focus.
                if w.focus != new {
                    self.set_focus(w, new);
                }
            }
        }
        let found = self.active.and_then(|id| workspaces.iter().find(|w| w.id == id));
        let w = found.or_else(|| workspaces.first())?.clone();
        self.active = Some(w.id);
        Some(w)
    }

    /// Show the session's tab with the keys in its pane, and count what it
    /// said read.
    fn go_to(&mut self, client: &Client, workspaces: &[Workspace], session: SessionId) {
        if let Some(x) = workspaces.iter().find(|x| x.layout.contains(&session)) {
            self.active = Some(x.id);
            self.set_focus(x, session);
        }
        let unread: Vec<u64> = client.notices().iter().filter(|n| n.session == session && !n.read).map(|n| n.id).collect();
        if !unread.is_empty() {
            client.read_notices(Some(unread));
        }
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
            // The dialog, with this pane's folder in it (the design's 1g).
            keys::Action::NewTab => self.new_session = Some(self.dialog(&here)),
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
            keys::Action::Resize(toward) => {
                // A twentieth of the split a press.
                let mut layout = w.layout.clone();
                if layout.nudge(&w.focus, toward, 0.05) {
                    if let Some(client) = &self.client {
                        client.set_layout(w.id, layout, w.focus);
                    }
                }
            }
            keys::Action::SwapPane => {
                // With the next pane, the last with the first; the keys stay
                // with the pane, now in the other's place.
                let leaves = w.layout.leaves();
                if let Some(k) = leaves.iter().position(|id| *id == w.focus).filter(|_| leaves.len() > 1) {
                    let mut layout = w.layout.clone();
                    layout.swap(&w.focus, &leaves[(k + 1) % leaves.len()]);
                    client.set_layout(w.id, layout, w.focus);
                }
            }
            keys::Action::Equalize => {
                let mut layout = w.layout.clone();
                layout.equalize();
                if layout != w.layout {
                    client.set_layout(w.id, layout, w.focus);
                }
            }
            keys::Action::PaneToTab => {
                // The keys go with it, so its new tab is shown.
                if w.layout.leaves().len() > 1 {
                    client.own_tab(w.focus);
                    self.following = Some((w.focus, w.id, std::time::Instant::now()));
                }
            }
            keys::Action::Zoom => self.zoom = !self.zoom,
            // The server writes the file; its Info says while it does.
            keys::Action::Record => match sessions.iter().find(|i| i.id == w.focus) {
                Some(info) if !info.recording.is_empty() => {
                    client.record(w.focus, None);
                    self.say(format!("Saved the recording to {}", info.recording), false);
                }
                Some(info) => match export::cast_path(&sort::display_title(&info.title, &info.command)) {
                    Some(path) => {
                        self.say(format!("Recording to {} (the same key stops)", path.display()), false);
                        client.record(w.focus, Some(path));
                    }
                    None => self.say("No home folder to record in".into(), true),
                },
                None => {}
            },
            keys::Action::Search => self.search = Some(palette::View::new()),
            keys::Action::Settings => self.open_settings(),
            keys::Action::Input => self.input.toggle(),
            // The tab's name in a field on its card (the menu's Rename).
            keys::Action::Rename => self.renaming_card = Some((w.id, w.name.clone())),
            keys::Action::TypeAll => {
                if !self.typing_all.remove(&w.id) && w.layout.leaves().len() > 1 {
                    self.typing_all.insert(w.id);
                }
            }
            // Under the bell, where a click opens it.
            keys::Action::Notices => {
                self.bell_open = match self.bell_open {
                    Some(_) => None,
                    None => Some(self.bell_anchor),
                };
                self.bell_opening = true;
            }
            keys::Action::Waiting => {
                self.lists = match &self.lists {
                    Some(v) if v.page == lists::Page::Waiting => None,
                    _ => Some(lists::View::new(lists::Page::Waiting)),
                }
            }
            keys::Action::CopyMode => {
                self.copy_mode = match self.copy_mode {
                    Some(_) => {
                        if let Some(pane) = self.panes.get(&w.focus) {
                            tsumugi_pane::Pane::clear_selection(pane);
                        }
                        None
                    }
                    None => self.panes.get(&w.focus).map(|pane| (w.focus, copymode::CopyMode::new(tsumugi_pane::Pane::screen(pane).cursor))),
                };
            }
            keys::Action::CopyOutput => {
                if let Some(pane) = self.panes.get(&w.focus) {
                    pane.copy_output();
                }
            }
            keys::Action::PrevPrompt | keys::Action::NextPrompt => {
                if let Some(pane) = self.panes.get(&w.focus) {
                    pane.jump_prompt(action == keys::Action::PrevPrompt);
                }
            }
            // Pressed again, the field gets the keys back.
            keys::Action::Find => match &mut self.find {
                Some(bar) if bar.id == w.focus => bar.focus = true,
                _ => self.find = Some(find::Bar::new(w.focus)),
            },
            keys::Action::Help => self.help = if self.help.is_some() { None } else { Some(help::View { opening: true }) },
            keys::Action::Overview => {
                self.lists = match &self.lists {
                    Some(v) if v.page == lists::Page::All => None,
                    _ => Some(lists::View::new(lists::Page::All)),
                }
            }
            keys::Action::Duplicate => {
                if let Some(focus) = sessions.iter().find(|i| i.id == w.focus) {
                    let typed = focus.claude.then(|| newsession::Start::Claude.typed(&self.settings_now.sessions.claude)).flatten();
                    if let Ok(pane) = client.spawn_typing(focus.cwd.clone(), None, Size::new(80, 24), (8, 16), Place::NewWorkspace, typed) {
                        self.pending = Some(pane.id());
                    }
                }
            }
            keys::Action::Rail => {
                self.view.rail = !self.view.rail;
                self.view.save();
            }
            keys::Action::FontBigger | keys::Action::FontSmaller | keys::Action::FontReset => {
                let base = self.settings_now.font.size;
                self.font_step = match action {
                    keys::Action::FontBigger => font_size(base, self.font_step + 1.0) - base,
                    keys::Action::FontSmaller => font_size(base, self.font_step - 1.0) - base,
                    _ => 0.0,
                };
                self.font = egui::FontId::monospace(font_size(base, self.font_step));
                let size = self.font.size;
                let words = if self.font_step == 0.0 { format!("Letters at {size} pt, as the settings have them") } else { format!("Letters at {size} pt ({} resets)", keys::label(keys::Action::FontReset)) };
                self.say(words, false);
            }
        }
    }

    /// What the search box lists: every session, every folder the sessions
    /// are in, and the commands.
    fn search_entries(workspaces: &[Workspace], sessions: &[Info], saved: &[prompts::Prompt], kept: &[layouts::Layout]) -> Vec<palette::Entry> {
        let mut out = Vec::new();
        for w in workspaces {
            for id in w.layout.leaves() {
                let Some(i) = sessions.iter().find(|i| i.id == id) else { continue };
                let name = sort::display_title(&i.title, &i.command);
                let mut detail = home_short(&i.cwd);
                if !i.branch.is_empty() {
                    detail.push_str(&format!(" · {}", i.branch));
                }
                for t in &i.tags {
                    detail.push_str(&format!(" · {t}"));
                }
                out.push(palette::Entry { title: name, detail, pick: palette::Pick::Session(id) });
            }
        }
        let mut folders: Vec<&std::path::Path> = Vec::new();
        for i in sessions {
            for f in [i.project.as_path(), i.cwd.as_path()] {
                if !folders.contains(&f) {
                    folders.push(f);
                }
            }
        }
        for f in folders {
            out.push(palette::Entry { title: format!("New session in {}", home_short(f)), detail: "folder".into(), pick: palette::Pick::Folder(f.to_path_buf()) });
        }
        for c in palette::Command::ALL {
            out.push(palette::Entry { title: c.title(), detail: c.key(), pick: palette::Pick::Command(c) });
        }
        for (k, p) in saved.iter().enumerate() {
            let first = p.text.lines().next().unwrap_or_default();
            out.push(palette::Entry { title: format!("Send prompt: {}", p.name), detail: first.chars().take(60).collect(), pick: palette::Pick::Prompt(k) });
        }
        for (k, l) in kept.iter().enumerate() {
            out.push(palette::Entry { title: format!("Open layout: {}", l.name), detail: layouts::words(&l.tree), pick: palette::Pick::Layout(k) });
        }
        out
    }

    /// Every theme: the built-in ones and one's own (a file's missing
    /// colours from tsumugi Dark or Light), and what is wrong with a file.
    fn themes(&self) -> (Vec<theme::Theme>, Option<String>) {
        let mut all = theme::builtin();
        let mut problem = None;
        for (name, table) in &self.theme_files.own {
            let light = table.get("light").and_then(toml::Value::as_bool).unwrap_or(false);
            let base = all[usize::from(light)].colors;
            match theme::from_table(base, table) {
                Ok(colors) => all.push(theme::Theme { name: name.clone(), colors }),
                Err(e) => problem = Some(format!("themes/{name}: {e}")),
            }
        }
        (all, problem)
    }

    /// Put the chosen theme in force when it changed: the drawing code's
    /// colours, the panes' palette and egui's own widgets.
    fn apply_theme(&mut self, ctx: &egui::Context) {
        let os_light = ctx.system_theme() == Some(egui::Theme::Light);
        let (all, problem) = self.themes();
        self.theme_error = self.theme_file_error.clone().or(problem);
        let (choice, dark, light) = &self.theme_choice;
        let mut colors = match theme::pick(&all, choice, dark, light, os_light) {
            Ok(t) => t.colors,
            Err(e) => {
                self.theme_error = Some(e);
                all[0].colors
            }
        };
        if let Some(changes) = &self.theme_files.changes {
            match theme::from_table(colors, changes) {
                Ok(c) => colors = c,
                Err(e) => self.theme_error = Some(format!("theme.toml: {e}")),
            }
        }
        if self.theme_applied != Some(colors) {
            self.theme_applied = Some(colors);
            theme::set(colors);
            self.palette = colors.palette();
            ctx.set_visuals(colors.visuals());
        }
    }

    /// The settings screen in place of the panes, and what it changes.
    fn settings_screen(&mut self, ui: &mut egui::Ui, client: &Client, workspaces: &[Workspace], sessions: &[Info]) {
        let mut tags: Vec<String> = Vec::new();
        for t in workspaces.iter().flat_map(|w| w.layout.leaves()).filter_map(|id| sessions.iter().find(|i| i.id == id)).flat_map(|i| &i.tags) {
            if !tags.contains(t) {
                tags.push(t.clone());
            }
        }
        let (themes, _) = self.themes();
        let shown = |p: Option<std::path::PathBuf>| p.map(|p| p.display().to_string()).unwrap_or_default();
        let muted_tags = client.muted_tags();
        let seen = prefs::Seen {
            settings: &self.settings_now,
            themes: &themes,
            current: theme::colors(),
            profiles: &self.profiles,
            tags: &tags,
            muted_tags: &muted_tags,
            sort: self.view.sort,
            always_restore: always_restore(),
            nerd: self.nerd(),
            font_names: fonts::names(),
            font_file: self.font_file.as_ref().map(|p| p.display().to_string()),
            faces: self.faces_found,
            server_up: chrome::elapsed(chrome::now_ms().saturating_sub(client.started_ms())),
            settings_path: shown(tsumugi_mux::settings::default_path().and_then(|p| p.parent().map(std::path::Path::to_path_buf))),
            state_path: shown(tsumugi_mux::state::default_path()),
            address: Address::for_user().0.display().to_string(),
            facts: self.facts.as_ref(),
        };
        let Some(screen) = &mut self.prefs else { return };
        let changes = prefs::show(ui, &self.palette, screen, &seen);
        for change in changes {
            match change {
                prefs::Change::Set(table, key, value) => edit_settings(self.settings_tx.clone(), move |t| tsumugi_mux::settings::set_key(t, table, key, &value)),
                prefs::Change::SetIn(table, key, value) => edit_settings(self.settings_tx.clone(), move |t| match &value {
                    Some(v) => tsumugi_mux::settings::set_key(t, Some(&table), &key, v),
                    None => tsumugi_mux::settings::remove_key(t, Some(&table), &key),
                }),
                prefs::Change::Rules(rules) => edit_settings(self.settings_tx.clone(), move |t| write_rules(t, &rules)),
                prefs::Change::MenuItems(items) => edit_settings(self.settings_tx.clone(), move |t| {
                    let blocks: Vec<Vec<(&str, String)>> = items.iter().map(|i| vec![("name", tsumugi_mux::settings::quote(&i.name)), ("command", tsumugi_mux::settings::quote(&i.command))]).collect();
                    tsumugi_mux::settings::set_tables(t, "menu.session", &blocks)
                }),
                prefs::Change::RenameTag(old, new) => self.rename_tag(client, sessions, old, new),
                prefs::Change::PlaySound(name) => sound::play(&name),
                prefs::Change::SaveProfile(was, profile) => {
                    match self.profiles.iter_mut().find(|p| Some(&p.name) == was.as_ref()) {
                        Some(p) => *p = profile,
                        None => self.profiles.push(profile),
                    }
                    self.save_profiles();
                }
                prefs::Change::Autostart(on) => self.job(move || {
                    autostart::set(on)?;
                    Ok(if on { "The server now starts when you sign in".into() } else { "The server no longer starts at sign-in".into() })
                }),
                prefs::Change::Hooks(true) => self.add_hooks(),
                prefs::Change::Hooks(false) => self.job(|| hooks::uninstall().map(|p| format!("tsumugi's hooks are out of {}; the old file is beside it", home_short(&p)))),
                prefs::Change::ShellHook(shell, on) => self.job(move || {
                    let p = shellhook::set_installed(&shell, on)?;
                    Ok(if on { format!("The shell integration is in {}: new sessions have it", home_short(&p)) } else { format!("The shell integration is out of {}", home_short(&p)) })
                }),
                prefs::Change::RestartServer => self.restart_server(ui.ctx()),
                prefs::Change::Export => self.job(|| {
                    let dir = tsumugi_mux::settings::default_path().and_then(|p| p.parent().map(std::path::Path::to_path_buf)).ok_or("no settings folder")?;
                    let to = export::place().ok_or("no home folder")?;
                    export::export(&dir, &to).map(|f| format!("Exported to {}", home_short(&f)))
                }),
                prefs::Change::Import(from) => self.job(move || {
                    let dir = tsumugi_mux::settings::default_path().and_then(|p| p.parent().map(std::path::Path::to_path_buf)).ok_or("no settings folder")?;
                    export::import(&dir, std::path::Path::new(&from)).map(|n| format!("Read {n} files back; those there before are kept as .bak. Profiles come back when the window opens next"))
                }),
                prefs::Change::GoTo(_) => {}
                prefs::Change::MuteTag(tag, on) => client.mute_tag(tag, on),
                prefs::Change::TestNotification => self.teller.send(alert::Out::Notify {
                    session: 0,
                    title: "tsumugi".into(),
                    body: "A test notification: this is how a session tells you".into(),
                }),
                prefs::Change::DeleteProfile(name) => {
                    self.profiles.retain(|p| p.name != name);
                    self.save_profiles();
                }
                prefs::Change::Sort(s) => {
                    self.view.sort = s;
                    self.view.save();
                }
                prefs::Change::AlwaysRestore(on) => set_always_restore(on),
                prefs::Change::OpenFolder => {
                    if let Some(dir) = tsumugi_mux::settings::default_path().and_then(|p| p.parent().map(std::path::Path::to_path_buf)) {
                        let _ = std::fs::create_dir_all(&dir);
                        menu::open_with_system(&dir);
                    }
                }
                prefs::Change::OpenFile => {
                    if let Some(p) = tsumugi_mux::settings::default_path() {
                        menu::open_with_system(&p);
                    }
                }
                prefs::Change::Copy(text) => ui.ctx().copy_text(text),
                prefs::Change::Close => self.prefs = None,
            }
        }
    }

    /// The folders the new-session dialog offers: the pane with the keys'
    /// first, then where the other sessions are, the most recently busy
    /// first, each with its branch.
    fn recents(sessions: &[Info], current: Option<&Workspace>, closed: &[std::path::PathBuf]) -> Vec<newsession::Recent> {
        let mut by_time: Vec<&Info> = sessions.iter().collect();
        by_time.sort_by_key(|i| std::cmp::Reverse(i.since_ms));
        if let Some(focus) = current.and_then(|w| sessions.iter().find(|i| i.id == w.focus)) {
            by_time.insert(0, focus);
        }
        let mut out: Vec<newsession::Recent> = Vec::new();
        for (k, i) in by_time.into_iter().enumerate() {
            for f in [&i.cwd, &i.project] {
                if !out.iter().any(|r| r.folder == *f) {
                    let note = if k == 0 && current.is_some() { format!("{}  · now", i.branch) } else { i.branch.clone() };
                    out.push(newsession::Recent { folder: f.clone(), note: note.trim().to_owned() });
                }
            }
        }
        // Then where sessions ended, newest first.
        for f in closed {
            if !out.iter().any(|r| r.folder == *f) {
                out.push(newsession::Recent { folder: f.clone(), note: "closed".into() });
            }
        }
        out
    }

    /// Note the folders of sessions that have ended, for the new-session
    /// dialog's list: kept beside the state, the newest first.
    fn remember_closed(&mut self, sessions: &[Info]) {
        let mut ended = Vec::new();
        self.known_cwds.retain(|id, cwd| {
            let alive = sessions.iter().any(|i| i.id == *id);
            if !alive {
                ended.push(cwd.clone());
            }
            alive
        });
        for i in sessions {
            self.known_cwds.insert(i.id, i.cwd.clone());
        }
        if ended.is_empty() {
            return;
        }
        for f in ended {
            self.closed.retain(|c| *c != f);
            self.closed.insert(0, f);
        }
        self.closed.truncate(CLOSED_KEPT);
        let list = self.closed.clone();
        let _ = std::thread::Builder::new().name("closed".into()).spawn(move || {
            let Some(p) = closed_file() else { return };
            let text: String = list.iter().map(|f| format!("{}\n", f.display())).collect();
            let _ = std::fs::write(p, text);
        });
    }

    /// Start what the new-session dialog asked for.
    fn create(&mut self, client: &Client, mut c: newsession::Create, current: Option<&Workspace>) {
        // The worktree first, on a thread (git can take a moment); the
        // session starts in it when it is made.
        if let Some(branch) = c.worktree.take() {
            let (tx, rx) = std::sync::mpsc::channel();
            let (folder, ctx) = (c.folder.clone(), self.ctx.clone());
            let _ = std::thread::Builder::new().name("worktree".into()).spawn(move || {
                let _ = tx.send(worktree::add(&folder, &branch));
                ctx.request_repaint();
            });
            self.worktree_making = Some((rx, c));
            return;
        }
        let place = match current {
            Some(w) if c.split => Place::Split { beside: w.focus, dir: Dir::Right },
            _ => Place::NewWorkspace,
        };
        let shell = c.on.shell();
        match client.spawn_typing(c.folder.clone(), shell, Size::new(80, 24), (8, 16), place, c.start.typed(&self.settings_now.sessions.claude)) {
            Ok(pane) => {
                let id = pane.id();
                self.pending = Some(id);
                // The server gives the rules' tags itself; the rest go on,
                // and the rules' taken off in the dialog come off.
                for t in c.tags.iter().cloned() {
                    client.tag(vec![id], t, true);
                }
                for t in c.dropped.iter().cloned() {
                    client.tag(vec![id], t, false);
                }
                // The profile's other panes, beside it in the same tab.
                let mut made = Vec::new();
                for (k, s) in c.more.iter().enumerate() {
                    let (beside, dir) = newsession::more_place(k, id, &made);
                    let shell = c.on.shell();
                    match client.spawn_typing(c.folder.clone(), shell, Size::new(80, 24), (8, 16), Place::Split { beside, dir }, s.typed(&self.settings_now.sessions.claude)) {
                        Ok(pane) => {
                            made.push(pane.id());
                            for t in c.tags.iter().cloned() {
                                client.tag(vec![pane.id()], t, true);
                            }
                        }
                        Err(e) => self.say(format!("A pane did not start: {e}"), true),
                    }
                }
            }
            Err(e) => self.failed = Some(format!("the shell did not start: {e}")),
        }
        if let Some(name) = c.save_as {
            let profile = tsumugi_mux::settings::Profile {
                name,
                folder: home_short(&c.folder),
                start: c.start.word().into(),
                tags: c.tags.clone(),
                panes: c.more.iter().map(|s| s.word().to_owned()).collect(),
                place: c.on.word(),
            };
            save_profile(self.profiles.clone(), profile);
        }
    }

    /// Keep the tab's shape and what each pane runs (Claude Code or a
    /// shell) under the tab's name, replacing a layout of that name.
    fn save_layout(&mut self, client: &Client, w: &Workspace) {
        let sessions = client.sessions();
        let tree = w.layout.clone().map(&mut |id| match sessions.iter().find(|i| i.id == id) {
            Some(i) if i.claude => newsession::Start::Claude,
            _ => newsession::Start::Shell,
        });
        let name = if !w.name.trim().is_empty() {
            w.name.trim().to_owned()
        } else {
            let project = sessions.iter().find(|i| i.id == w.focus).and_then(|i| i.project.file_name().map(|n| n.to_string_lossy().into_owned()));
            format!("{} · {} panes", project.unwrap_or_else(|| "Tab".into()), tree.leaves().len())
        };
        let words = layouts::words(&tree);
        layouts::put(&mut self.layouts, layouts::Layout { name: name.clone(), tree });
        layouts::save(self.layouts.clone());
        self.say(format!("Saved layout \u{201c}{name}\u{201d} ({words}); open it from the search box"), false);
    }

    /// A saved layout as a new tab in `folder`: its first pane as the tab,
    /// the rest split from it, then the saved shape set once all are there.
    fn open_layout(&mut self, client: &Client, l: &layouts::Layout, folder: std::path::PathBuf) {
        let claude = self.settings_now.sessions.claude.clone();
        let starts = l.tree.leaves();
        let mut made: Vec<SessionId> = Vec::new();
        for (k, s) in starts.iter().enumerate() {
            let place = match made.first() {
                None => Place::NewWorkspace,
                Some(first) => Place::Split { beside: *first, dir: Dir::Right },
            };
            match client.spawn_typing(folder.clone(), None, Size::new(80, 24), (8, 16), place, s.typed(&claude)) {
                Ok(pane) => made.push(pane.id()),
                Err(e) if k == 0 => {
                    self.failed = Some(format!("the shell did not start: {e}"));
                    return;
                }
                Err(e) => self.say(format!("A pane did not start: {e}"), true),
            }
        }
        let Some(&first) = made.first() else { return };
        self.pending = Some(first);
        if made.len() == starts.len() && made.len() > 1 {
            let mut ids = made.into_iter();
            let tree = l.tree.clone().map(&mut |_| ids.next().unwrap_or(first));
            self.opening = Some((tree, first, std::time::Instant::now()));
        }
    }

    /// Do what the search box picked.
    fn picked(&mut self, pick: palette::Pick, current: Option<&Workspace>, workspaces: &[Workspace], area: Rect) {
        let Some(client) = self.client.clone() else { return };
        match pick {
            palette::Pick::Session(id) => self.go_to(&client, workspaces, id),
            palette::Pick::Line { id, line, col, len } => {
                self.go_to(&client, workspaces, id);
                client.reveal(id, line, col, len);
            }
            palette::Pick::Folder(dir) => match new_session(&client, dir, Place::NewWorkspace) {
                Ok(pane) => self.pending = Some(pane.id()),
                Err(e) => self.failed = Some(e),
            },
            palette::Pick::Prompt(k) => {
                let (Some(p), Some(w)) = (self.input.prompts.get(k).cloned(), current) else { return };
                let sessions = client.sessions();
                let to = if self.typing_all.contains(&w.id) { w.layout.leaves() } else { vec![w.focus] };
                for id in &to {
                    let text = match sessions.iter().find(|i| i.id == *id) {
                        Some(i) => prompts::fill(&p.text, &i.cwd, &i.project, &i.branch),
                        None => p.text.clone(),
                    };
                    client.send_prompt(*id, text);
                }
                let whom = if to.len() > 1 { format!("{} panes", to.len()) } else { "this pane".to_owned() };
                self.say(format!("Sent \u{201c}{}\u{201d} to {whom}", p.name), false);
            }
            palette::Pick::Layout(k) => {
                let Some(l) = self.layouts.get(k).cloned() else { return };
                let focus = current.and_then(|w| client.sessions().into_iter().find(|i| i.id == w.focus));
                let folder = focus.map(|i| i.cwd).or_else(|| std::env::current_dir().ok()).unwrap_or_default();
                self.open_layout(&client, &l, folder);
            }
            palette::Pick::Command(palette::Command::SaveLayout) => {
                if let Some(w) = current {
                    self.save_layout(&client, w);
                }
            }
            palette::Pick::Command(palette::Command::Closed) => self.lists = Some(lists::View::new(lists::Page::Closed)),
            palette::Pick::Command(palette::Command::Parallel) => {
                let focus = current.and_then(|w| client.sessions().into_iter().find(|i| i.id == w.focus));
                let folder = focus.map(|i| i.project).or_else(|| std::env::current_dir().ok()).unwrap_or_default();
                self.parallel = Some(parallel::View::new(&folder));
            }
            palette::Pick::Command(palette::Command::SaveOutput) => {
                let focus = current.and_then(|w| client.sessions().into_iter().find(|i| i.id == w.focus));
                if let Some(i) = focus {
                    self.saving.insert(i.id, sort::display_title(&i.title, &i.command));
                    client.all_text(i.id);
                }
            }
            palette::Pick::Command(palette::Command::Changes) => {
                let focus = current.and_then(|w| client.sessions().into_iter().find(|i| i.id == w.focus));
                if let Some(i) = focus {
                    let ctx = self.ctx.clone();
                    self.diff = Some(diffview::View::open(sort::display_title(&i.title, &i.command), i.cwd, move || ctx.request_repaint()));
                }
            }
            palette::Pick::Command(palette::Command::Sort(s)) => {
                self.view.sort = s;
                self.view.save();
            }
            palette::Pick::Command(c) => {
                let action = match c {
                    palette::Command::NewSession => keys::Action::NewTab,
                    // (opens the dialog)
                    palette::Command::SplitRight => keys::Action::SplitRight,
                    palette::Command::SplitDown => keys::Action::SplitDown,
                    palette::Command::Zoom => keys::Action::Zoom,
                    palette::Command::NextWaiting => keys::Action::NextWaiting,
                    palette::Command::CloseTab => keys::Action::CloseTab,
                    palette::Command::Settings => keys::Action::Settings,
                    palette::Command::InputBox => keys::Action::Input,
                    palette::Command::Waiting => keys::Action::Waiting,
                    palette::Command::TypeAll => keys::Action::TypeAll,
                    palette::Command::Notices => keys::Action::Notices,
                    palette::Command::Help => keys::Action::Help,
                    palette::Command::Sort(_) | palette::Command::Closed | palette::Command::Changes | palette::Command::SaveOutput | palette::Command::Parallel | palette::Command::SaveLayout => return,
                };
                if let Some(w) = current {
                    self.act(action, w, workspaces, area);
                }
            }
        }
    }

    /// The sidebar, or the rail, down the left; the tab picked there becomes
    /// the one shown. `top`: room kept above it, which moves the window as
    /// the band does.
    fn side_panel(&mut self, ui: &mut egui::Ui, workspaces: &[Workspace], sessions: &[Info], top: f32) {
        // Room at the top for macOS's traffic lights, when they sit on it.
        let side = egui::Frame::NONE.fill(self.chrome_fill(self.palette.on_cursor)).inner_margin(egui::Margin { top: top as i8, ..Default::default() });
        // Each switch between the two starts the panel at its own width: a
        // fresh id, since egui keeps a panel's width by its id.
        self.side_rect = None;
        self.fields_drawn = (false, false);
        let picked = if self.prefs.is_some() {
            None
        } else if self.view.rail {
            let shown = egui::Panel::left(egui::Id::new(("rail", self.side_gen)))
                .resizable(true)
                .default_size(RAIL)
                .size_range(RAIL..=RAIL_AT + 40.0)
                .frame(side)
                .show(ui, |ui| self.rail(ui, workspaces, sessions));
            if shown.response.rect.width() > RAIL_AT {
                self.view.rail = false;
                self.side_gen += 1;
                self.view.save();
            }
            self.side_rect = Some(shown.response.rect);
            shown.inner
        } else {
            let shown = egui::Panel::left(egui::Id::new(("sessions", self.side_gen)))
                .resizable(true)
                .default_size(SIDEBAR)
                .size_range(SIDEBAR_RANGE)
                .frame(side)
                .show(ui, |ui| self.sidebar(ui, workspaces, sessions));
            if shown.response.rect.width() < RAIL_AT {
                self.view.rail = true;
                self.side_gen += 1;
                self.view.save();
            }
            self.side_rect = Some(shown.response.rect);
            shown.inner
        };
        if let Some(id) = picked {
            self.active = Some(id);
        }
        // A card's note or name field not drawn this frame (its card
        // filtered out, the rail, the settings) is let go: while it is set
        // the pane gets no keys (the source review, 2026-10-07).
        if !self.fields_drawn.0 {
            self.noting_card = None;
        }
        if !self.fields_drawn.1 {
            self.renaming_card = None;
        }
        if let (true, Some(side)) = (top > 0.0, self.side_rect) {
            let strip = egui::Rect::from_min_size(side.min, egui::vec2(side.width(), top));
            let drag = ui.interact(strip, egui::Id::new("lights-strip"), egui::Sense::CLICK | egui::Sense::DRAG);
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            } else if drag.double_clicked() {
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
        }
    }

    /// What a tab's row carries under its first lines, which sets how tall
    /// it is: worked out once to fit the list, and again to draw it. `None`
    /// for a tab with no pane.
    fn card_lines<'t>(&self, tab: &sort::Tab<'t>, used: &usage::Usage, now: u64, as_card: bool) -> Option<CardLines<'t>> {
        let w = tab.workspace;
        let infos = &tab.infos;
        let urgent = tab.urgent()?;
        let mut tags: Vec<&String> = Vec::new();
        for t in infos.iter().flat_map(|i| &i.tags) {
            if !tags.contains(&t) {
                tags.push(t);
            }
        }
        // More lines on a card: its pull request, when one is known for its
        // branch, and on the selected card its conversation's numbers (the
        // design's Sidebar, 4 and 5).
        let numbers = (as_card && Some(w.id) == self.active).then(|| {
            let c = &urgent.conversation;
            let talk = used.talks.get(c)?;
            let tokens = used.conversations.get(c).map_or(0, usage::Tokens::total);
            let prompts = if talk.prompts == 1 { "1 prompt".to_owned() } else { format!("{} prompts", talk.prompts) };
            let cost = used.costs.get(c).filter(|c| **c > 0.0).map(|c| format!(" · ≈{}", price::dollars(*c))).unwrap_or_default();
            Some(format!("{prompts} · {} · {} tokens{cost}", chrome::elapsed(talk.ms), usage::short(tokens)))
        }).flatten();
        // What runs in it other than Claude Code (`codex`) and the ports its
        // programs listen on, a click away in the browser.
        let ports: Vec<u16> = infos.iter().flat_map(|i| i.ports.iter().copied()).take(4).collect();
        let other_agent = infos.iter().map(|i| i.agent.as_str()).find(|a| !a.is_empty() && *a != "claude").map(str::to_owned);
        let run_line = as_card && (other_agent.is_some() || !ports.is_empty());
        // A note of one's own (the tab menu's Note…), and room for the field
        // while it is written.
        let noting = self.noting_card.as_ref().is_some_and(|(id, _)| *id == w.id);
        let note_line = as_card && (!w.note.is_empty() || noting);
        // A card with nothing to say on its third line (a shell) is the
        // shorter by it.
        let no_third = as_card && chrome::card_words(urgent, now).is_empty();
        let closed_up = if no_third { 17.0 } else { 0.0 };
        Some(CardLines { card: as_card, tags, numbers, ports, other_agent, noting, note_line, run_line, closed_up })
    }

    /// The header (SESSIONS and the bell), one row per tab, and the jump to
    /// what waits at the bottom. A row is the design's sidebar: the state of
    /// its most urgent pane and the title of the pane with the keys; its
    /// folder and branch; what it is doing and for how long, or what it said.
    fn sidebar(&mut self, ui: &mut egui::Ui, workspaces: &[Workspace], sessions: &[Info]) -> Option<WorkspaceId> {
        let pal = self.palette;
        let mut picked = None;
        let mut ops = Vec::new();
        let now = chrome::now_ms();
        let muted_tags = self.client.as_ref().map(Client::muted_tags).unwrap_or_default();
        let shown_tags = self.settings_now.tags.shown();
        let used = self.usage.get();
        let tabs: Vec<sort::Tab> = workspaces.iter().map(|w| sort::Tab::new(w, sessions)).collect();
        // Every tag in use, in the order the tabs show them.
        let mut all_tags: Vec<String> = Vec::new();
        for t in tabs.iter().flat_map(|t| t.infos.iter().flat_map(|i| &i.tags)) {
            if !all_tags.contains(t) {
                all_tags.push(t.clone());
            }
        }
        let projects = sort::projects(&tabs);
        // A filter on something no longer there lets everything through.
        if self.filter.tag.as_ref().is_some_and(|t| !all_tags.contains(t)) {
            self.filter.tag = None;
        }
        if self.filter.project.as_ref().is_some_and(|p| !projects.iter().any(|(q, _)| q == p)) {
            self.filter.project = None;
        }
        let shown = sort::arrange(&tabs, self.view.sort, &self.filter);

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let count = format!("{} of {}", shown.len(), tabs.len());
            ui.label(egui::RichText::new("SESSIONS").size(11.0).strong().color(pal.fg_dim));
            ui.label(egui::RichText::new(count).size(11.0).color(crate::theme::colors().faint()));
            // Right to left: `+` at the end, the order before it.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if chrome::plus_button(ui, &pal).clicked() {
                    self.new_session = Some(self.dialog(&self.here(workspaces, sessions)));
                }
                ui.add_space(2.0);
                // The order (the design's 1b): a button with the choice's
                // short name, opening the five.
                let button = chrome::sort_button(ui, &pal, self.view.sort.short());
                egui::Popup::menu(&button).show(|ui| {
                    for s in sort::Sort::ALL {
                        if ui.selectable_label(self.view.sort == s, s.label()).clicked() {
                            self.view.sort = s;
                            self.view.save();
                            ui.close();
                        }
                    }
                    // How the rows look (the design's 1i).
                    ui.separator();
                    let lines = self.view.density == sort::Density::Lines;
                    if ui.selectable_label(lines, "One line each").clicked() {
                        self.view.density = if lines { sort::Density::Cards } else { sort::Density::Lines };
                        self.view.save();
                        ui.close();
                    }
                    let key = keys::label(keys::Action::Rail);
                    if ui.selectable_label(false, format!("Narrow rail   {key}")).clicked() {
                        self.view.rail = true;
                        self.view.save();
                        ui.close();
                    }
                });
            });
        });
        ui.add_space(4.0);
        // The tags, to show only the tabs with one (the design's 1a).
        if !all_tags.is_empty() {
            // A margin rather than a space, so a second line lines up too.
            let margin = egui::Margin { left: 14, right: 10, top: 0, bottom: 0 };
            egui::Frame::NONE.inner_margin(margin).show(ui, |ui| ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
                for t in &all_tags {
                    let faded = muted_tags.contains(t);
                    let size = egui::vec2(ui.fonts_mut(|f| f.layout_no_wrap(t.clone(), egui::FontId::proportional(11.0), pal.fg).size().x) + 12.0, 16.0);
                    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
                    let on = self.filter.tag.as_ref() == Some(t);
                    let p = ui.painter();
                    let dim = self.filter.tag.is_some() && !on;
                    chrome::tag_chip(p, rect.min, t, faded || dim);
                    if on {
                        p.rect_stroke(rect.expand(1.5), 5.0, egui::Stroke::new(1.0, pal.fg), egui::StrokeKind::Outside);
                    }
                    let hint = if faded { format!("Show only {t} (muted)") } else { format!("Show only {t}") };
                    let resp = resp.on_hover_text(hint);
                    if resp.clicked() {
                        self.filter.tag = if on { None } else { Some(t.clone()) };
                    }
                    resp.context_menu(|ui| {
                        let label = if faded { format!("Unmute notifications for {t}") } else { format!("Mute notifications for {t}") };
                        if ui.button(label).clicked() {
                            ops.push(SideOp::MuteTag(t.clone(), !faded));
                            ui.close();
                        }
                    });
                }
            }));
            ui.add_space(6.0);
        }

        // The foot: the filters by state and by folder, and what waits one
        // key away (the design's sidebar foot).
        egui::Panel::bottom("side-foot").frame(egui::Frame::NONE).show(ui, |ui| {
            let (line, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
            ui.painter().rect_filled(line, 0.0, crate::theme::colors().border);
            ui.add_space(8.0);
            let margin = egui::Margin { left: 12, right: 8, top: 0, bottom: 0 };
            egui::Frame::NONE.inner_margin(margin).show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
                ui.horizontal_wrapped(|ui| {
                    let all = self.filter.kind.is_none();
                    if chrome::filter_button(ui, &pal, &format!("All {}", tabs.len()), None, all).clicked() {
                        self.filter.kind = None;
                    }
                    for k in sort::Kind::ALL {
                        let n = tabs.iter().filter(|t| t.kind() == Some(k)).count();
                        let on = self.filter.kind == Some(k);
                        if n == 0 && !on {
                            continue;
                        }
                        if chrome::filter_button(ui, &pal, &format!("{} {n}", k.label()), Some(k.state().map_or_else(chrome::grey, state_color)), on).clicked() {
                            self.filter.kind = if on { None } else { Some(k) };
                        }
                    }
                });
                if projects.len() > 1 || self.filter.project.is_some() {
                    ui.horizontal_wrapped(|ui| {
                        if chrome::filter_button(ui, &pal, &format!("All folders {}", tabs.len()), None, self.filter.project.is_none()).clicked() {
                            self.filter.project = None;
                        }
                        for (p, n) in &projects {
                            let on = self.filter.project.as_ref() == Some(p);
                            let name = p.file_name().map_or_else(|| p.display().to_string(), |f| f.to_string_lossy().into_owned());
                            let b = chrome::filter_button(ui, &pal, &format!("{name} {n}"), None, on).on_hover_text(home_short(p));
                            if b.clicked() {
                                self.filter.project = if on { None } else { Some(p.clone()) };
                            }
                        }
                    });
                }
            });
            let waiting = sessions.iter().filter(|i| matches!(i.state, State::Waiting | State::MaybeWaiting)).count();
            if waiting > 0 {
                // The design's: under a line, the key as a cap and the words
                // in grey; the key the one in force (`[keys] next_waiting`).
                let (line, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 9.0), egui::Sense::hover());
                ui.painter().hline(line.x_range().shrink(12.0), line.center().y, egui::Stroke::new(1.0, crate::theme::colors().border));
                let key = keys::label(keys::Action::NextWaiting);
                let resp = ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    let cap = egui::Button::new(egui::RichText::new(&key).font(egui::FontId::monospace(11.0)).color(crate::theme::colors().strong()))
                        .fill(crate::theme::colors().panel)
                        .stroke(egui::Stroke::new(1.0, crate::theme::colors().border_strong()))
                        .corner_radius(5.0);
                    // No cap when the key was given back to the shell.
                    let a = key != "none" && ui.add(cap).clicked();
                    let words = egui::RichText::new(format!("Jump to waiting · {waiting}")).size(12.0).color(pal.fg_dim);
                    let b = ui.add(egui::Label::new(words).sense(egui::Sense::click()));
                    // All of them in a list, to answer together.
                    let list = ui
                        .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(12.0);
                            let words = egui::RichText::new("List ›").size(12.0).color(chrome::ink(chrome::cyan()));
                            ui.add(egui::Label::new(words).sense(egui::Sense::click())).on_hover_text(format!("The waiting sessions, answered together ({})", keys::label(keys::Action::Waiting))).clicked()
                        })
                        .inner;
                    (a || b.clicked(), list)
                });
                if resp.inner.0 {
                    self.jump_waiting = true;
                }
                if resp.inner.1 {
                    self.lists = Some(lists::View::new(lists::Page::Waiting));
                }
            }
            ui.add_space(6.0);
        });

        // Nothing yet: what will be here (the design's First run).
        if tabs.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(ui.available_width() - 40.0);
                ui.label(egui::RichText::new("Sessions you start show here, with what each one is doing.").size(12.5).color(pal.fg_dim));
            });
        }
        // Rows can be dragged into another order in `Manual` only: in the
        // others the order is the rule's.
        let manual = self.view.sort == sort::Sort::Manual;
        let sense = if manual { egui::Sense::click_and_drag() } else { egui::Sense::click() };
        let mut rows: Vec<(WorkspaceId, egui::Rect)> = Vec::new();
        // No scrollbar: every card at full size while they fit, and the last
        // ones as lines when they do not (the wheel still moves a list too
        // long even as lines).
        egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            let mut listed = sort::items(&shown, self.view.sort, self.view.density, &self.closed_groups, self.active);
            // Room between the cards for their glow (the design's Sidebar).
            if self.view.density == sort::Density::Cards {
                ui.spacing_mut().item_spacing.y = 6.0;
            }
            let gap = ui.spacing().item_spacing.y;
            let fits: Vec<sort::Row> = listed
                .iter()
                .map(|item| match item {
                    sort::Item::Group { .. } => sort::Row { card: 28.0 + gap, line: 28.0 + gap, keep: true },
                    sort::Item::Tab { tab, card } => {
                        let tall = |as_card| self.card_lines(tab, &used, now, as_card).map_or(0.0, |l| l.height() + gap);
                        sort::Row { card: tall(*card), line: tall(false), keep: Some(tab.workspace.id) == self.active }
                    }
                })
                .collect();
            for (item, stays) in listed.iter_mut().zip(sort::fit(&fits, ui.available_height())) {
                if let sort::Item::Tab { card, .. } = item {
                    *card &= stays;
                }
            }
            for item in &listed {
                let (tab, as_card) = match item {
                    // A project's heading (the design's 1i B): a click
                    // opens or closes it.
                    sort::Item::Group { project, total, kinds, open } => {
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::click());
                        let p = ui.painter_at(rect);
                        if resp.hovered() {
                            p.rect_filled(rect.shrink2(egui::vec2(6.0, 1.0)), 6.0, pal.selection.gamma_multiply(0.4));
                        }
                        chrome::triangle(&p, egui::pos2(rect.left() + 18.0, rect.center().y), *open, pal.fg_dim);
                        let name = project.file_name().map_or_else(|| project.display().to_string(), |f| f.to_string_lossy().into_owned());
                        let name = p.layout_no_wrap(name, egui::FontId::proportional(13.0), crate::theme::colors().strong());
                        let x = rect.left() + 30.0 + name.size().x;
                        p.galley(egui::pos2(rect.left() + 28.0, rect.center().y - name.size().y / 2.0), name, egui::Color32::WHITE);
                        p.text(egui::pos2(x + 8.0, rect.center().y), egui::Align2::LEFT_CENTER, total.to_string(), egui::FontId::proportional(12.0), pal.fg_dim);
                        let mut right = rect.right() - 14.0;
                        if *open {
                            for (k, color) in [(sort::Kind::Error, chrome::red()), (sort::Kind::Waiting, chrome::gold())] {
                                let n = kinds.iter().filter(|x| **x == k).count();
                                if n > 0 {
                                    let r = p.text(egui::pos2(right, rect.center().y), egui::Align2::RIGHT_CENTER, format!("{n} {}", k.label().to_lowercase()), egui::FontId::proportional(11.5), color);
                                    right = r.left() - 8.0;
                                }
                            }
                        } else {
                            // Closed: only a dot each.
                            for k in kinds.iter().rev().take(12) {
                                p.circle_filled(egui::pos2(right - 3.0, rect.center().y), 3.0, k.state().map_or_else(chrome::grey, state_color));
                                right -= 9.0;
                            }
                        }
                        if resp.on_hover_text(home_short(project)).clicked() {
                            if *open {
                                self.closed_groups.push(project.clone());
                            } else {
                                self.closed_groups.retain(|q| q != project);
                            }
                        }
                        continue;
                    }
                    sort::Item::Tab { tab, card } => (*tab, *card),
                };
                let w = tab.workspace;
                let infos = &tab.infos;
                let (Some(focus), Some(urgent), Some(lines)) = (tab.focus(), tab.urgent(), self.card_lines(tab, &used, now, as_card)) else { continue };
                let (height, base) = (lines.height(), lines.base());
                let CardLines { tags, numbers, ports, other_agent, noting, note_line, run_line, closed_up, .. } = lines;
                let pr = as_card.then(|| self.git.known(&focus.cwd, &focus.branch).and_then(|g| g.pr)).flatten();
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), sense);
                rows.push((w.id, rect));
                let painter = ui.painter_at(rect);
                let muted = infos.iter().all(|i| i.muted);
                let quiet_tab = infos.iter().all(|i| quiet(i, &muted_tags));
                // The pointer on the changes' count: their list, not the preview.
                let mut over_changes = false;
                if as_card {
                let card = rect.shrink2(egui::vec2(6.0, 2.0));
                // Each state its ring (the design's States): waiting in gold
                // and breathing, the one to look at, its glow past the card.
                let look = chrome::look(urgent);
                match look {
                    chrome::Look::State(State::Waiting) => {
                        painter.rect_filled(card, 8.0, crate::theme::colors().wait_bg());
                    }
                    chrome::Look::State(State::Done) => chrome::done_ground(&painter, card, 8.0),
                    _ => {}
                }
                let moves = matches!(look, chrome::Look::State(State::Waiting | State::Running));
                let t = if moves { self.moving(ui) } else { None };
                chrome::state_ring(&ui.painter_at(rect.expand(4.0)), card, 8.0, look, t);
                // The hover comes and goes over a moment rather than at once.
                let hover = if self.settings_now.appearance.animations {
                    ui.ctx().animate_bool_with_time(egui::Id::new(("card-hover", w.id)), resp.hovered(), 0.12)
                } else {
                    f32::from(u8::from(resp.hovered()))
                };
                // The selected card keeps its dark ground: its ring in its
                // state's colour, brighter, and a soft glow past it.
                if Some(w.id) == self.active {
                    chrome::selected_ring(&ui.painter_at(rect.expand(4.0)), card, 8.0, look);
                } else if hover > 0.0 {
                    painter.rect_filled(card, 8.0, pal.selection.gamma_multiply(0.3 * hover));
                }
                if self.dragging_tab == Some(w.id) {
                    painter.rect_stroke(card, 8.0, egui::Stroke::new(1.0, chrome::cyan()), egui::StrokeKind::Inside);
                }
                // The state's mark, its shape saying it without the colour.
                chrome::state_mark(&painter, egui::pos2(rect.left() + 20.0, rect.top() + 14.0), look, t);
                let name = tab.name();
                let name = if infos.len() > 1 { format!("{name}  ·{}", infos.len()) } else { name };
                let left = rect.left() + 32.0;
                let width = rect.right() - left - 12.0;
                let line = |text: String, y: f32, font: egui::FontId, color: egui::Color32, width: f32| {
                    let galley = ui.fonts_mut(|f| {
                        let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
                        job.wrap = egui::text::TextWrapping::truncate_at_width(width);
                        f.layout_job(job)
                    });
                    painter.galley(egui::pos2(left, rect.top() + y), galley, color);
                };
                let marks = usize::from(quiet_tab) + usize::from(w.pinned);
                // A waiting card's name in the warm white the design gives it.
                let name_color = if look == chrome::Look::State(State::Waiting) { crate::theme::colors().wait_text() } else { pal.fg };
                // Bold and small in the window's own letters (not the
                // terminal's); a finished one's in the plain weight.
                let name_font = if look == chrome::Look::State(State::Done) { egui::FontId::proportional(13.0) } else { fonts::bold(13.0) };
                line(name, 6.0, name_font, name_color, width - 18.0 * marks as f32);
                let mut mark_x = rect.right() - 22.0;
                if quiet_tab {
                    chrome::muted_mark(&painter, egui::pos2(mark_x, rect.top() + 14.0), pal.fg_dim);
                    mark_x -= 18.0;
                }
                if w.pinned {
                    chrome::pin_mark(&painter, egui::pos2(mark_x, rect.top() + 14.0), pal.fg_dim);
                }
                // Where to take hold of it, shown only on the way there.
                if manual && resp.hovered() {
                    chrome::grip(&painter, egui::pos2(rect.right() - 16.0, rect.center().y), pal.fg_dim);
                }
                // The folder, then ` · ` and the branch, then its pull
                // request in its checks' colour; the folder gives way first
                // when the row is narrow.
                let mono = egui::FontId::monospace(11.0);
                let branch = (!focus.branch.is_empty() || pr.is_some()).then(|| {
                    ui.fonts_mut(|f| {
                        let mut job = egui::text::LayoutJob::default();
                        let plain = egui::TextFormat::simple(mono.clone(), pal.fg_dim);
                        if !focus.branch.is_empty() {
                            job.append(&format!(" · {}", focus.branch), 0.0, plain.clone());
                        }
                        if let Some(pr) = &pr {
                            job.append(" · ", 0.0, plain);
                            job.append(&format!("PR #{}", pr.number), 0.0, egui::TextFormat::simple(mono.clone(), chrome::pr_line(pr).1));
                        }
                        job.wrap = egui::text::TextWrapping::truncate_at_width(width - 70.0);
                        f.layout_job(job)
                    })
                });
                // What it has changed and not committed, on the right: lines
                // added and removed, or how many files when git counts none.
                let changed = self.changes.get(&focus.cwd, &focus.branch).and_then(|g| g.changes).filter(|c| !c.files.is_empty());
                let diff = changed.as_ref().map(|c| {
                    let mut job = egui::text::LayoutJob::default();
                    let f = |color| egui::TextFormat::simple(mono.clone(), color);
                    if c.added + c.removed > 0 {
                        job.append(&format!("+{}", c.added), 0.0, f(chrome::green()));
                        job.append(&format!("−{}", c.removed), 4.0, f(chrome::red()));
                    } else {
                        job.append(&format!("{} new", c.files.len()), 0.0, f(chrome::green()));
                    }
                    ui.fonts_mut(|x| x.layout_job(job))
                });
                let diff_w = diff.as_ref().map_or(0.0, |g| g.size().x + 10.0);
                let room = width - 6.0 - diff_w - branch.as_ref().map_or(0.0, |b| b.size().x);
                let folder = ui.fonts_mut(|f| {
                    let mut job = egui::text::LayoutJob::simple_singleline(home_short(&focus.cwd), mono.clone(), pal.fg_dim);
                    job.wrap = egui::text::TextWrapping::truncate_at_width(room.max(20.0));
                    f.layout_job(job)
                });
                let y = rect.top() + 25.0;
                let folder_w = folder.size().x;
                painter.galley(egui::pos2(left, y), folder, pal.fg_dim);
                if let Some(b) = branch {
                    let at = egui::pos2(left + folder_w, y);
                    if let Some(pr) = &pr {
                        let r = egui::Rect::from_min_size(at, b.size());
                        ui.interact(r, egui::Id::new(("pr", w.id)), egui::Sense::hover()).on_hover_text(chrome::pr_line(pr).0);
                    }
                    painter.galley(at, b, pal.fg_dim);
                }
                if let (Some(g), Some(c)) = (diff, &changed) {
                    let at = egui::pos2(rect.right() - 12.0 - g.size().x, y);
                    let r = egui::Rect::from_min_size(at, g.size());
                    painter.galley(at, g, pal.fg);
                    let list: Vec<String> = c.files.iter().take(15).cloned().collect();
                    let more = c.files.len().saturating_sub(15);
                    let over = ui.interact(r, egui::Id::new(("changes", w.id)), egui::Sense::click());
                    over_changes = over.hovered();
                    if over.clicked() {
                        ops.push(SideOp::Diff(sort::display_title(&focus.title, &focus.command), focus.cwd.clone()));
                    }
                    over.on_hover_ui(|ui| {
                        ui.label(egui::RichText::new(format!("{} file(s) changed, not committed", c.files.len())).strong());
                        ui.label(egui::RichText::new(list.join("\n")).monospace().size(11.0).color(pal.fg_dim));
                        if more > 0 {
                            ui.label(egui::RichText::new(format!("and {more} more")).size(11.0).color(pal.fg_dim));
                        }
                        ui.label(egui::RichText::new("Click to see the diff").size(11.0).color(pal.fg_dim));
                    });
                }
                let words = chrome::card_words(urgent, now);
                let mut third = match urgent.state {
                    State::Waiting | State::Error | State::Done if !urgent.note.is_empty() && !words.is_empty() => format!("{words} · {}", urgent.note),
                    _ => words,
                };
                // A working agent's tokens so far (the design's Main).
                if urgent.state == State::Running {
                    if let Some(t) = used.conversations.get(&urgent.conversation).filter(|t| t.total() > 0) {
                        third = format!("{third} · {} tokens", usage::short(t.total()));
                    }
                }
                let queued = self.queue.iter().filter(|q| infos.iter().any(|i| i.id == q.id)).count();
                if queued > 0 {
                    third = format!("{queued} queued · {third}");
                }
                // In its state's colour; probably waiting stays quiet.
                let third_color = match urgent.state {
                    State::MaybeWaiting => pal.fg_dim,
                    s => chrome::state_ink(s),
                };
                // A question on its screen: its choices as buttons, so it is
                // answered without going there (the third line gives way).
                let mut room = width;
                if urgent.state == State::Waiting {
                    let found = answer::choices(&self.watch_lines(urgent.id));
                    let mut x = rect.right() - 12.0;
                    for c in found.iter().take(3).rev() {
                        let label = format!("{} {}", c.key, answer::short(&c.text));
                        let g = ui.fonts_mut(|f| f.layout_no_wrap(label, egui::FontId::proportional(11.0), pal.fg));
                        let r = egui::Rect::from_min_size(egui::pos2(x - g.size().x - 12.0, rect.top() + 40.0), egui::vec2(g.size().x + 12.0, 18.0));
                        if r.left() < left + 40.0 {
                            break;
                        }
                        let b = ui.interact(r, egui::Id::new(("answer", urgent.id, c.key)), egui::Sense::click()).on_hover_text(format!("Type {}: {}", c.key, c.text));
                        let first = c.key == '1';
                        let fill = match (first, b.hovered()) {
                            (true, true) => chrome::cyan(),
                            (true, false) => chrome::cyan().gamma_multiply(0.8),
                            (false, true) => crate::theme::colors().hover(),
                            (false, false) => crate::theme::colors().panel,
                        };
                        painter.rect_filled(r, 5.0, fill);
                        painter.rect_stroke(r, 5.0, egui::Stroke::new(1.0, crate::theme::colors().border), egui::StrokeKind::Inside);
                        let color = if first { crate::theme::colors().on_accent() } else { pal.fg };
                        painter.galley(egui::pos2(r.left() + 6.0, r.center().y - g.size().y / 2.0), g, color);
                        if b.clicked() {
                            if let Some(pane) = self.panes.get(&urgent.id) {
                                tsumugi_pane::Pane::send(pane, vec![c.key as u8]);
                            }
                        }
                        x = r.left() - 4.0;
                    }
                    room = (x - left - 6.0).max(20.0);
                }
                line(third, 42.0, egui::FontId::proportional(11.5), third_color, room);
                // Up to `[tags] shown` tags, the rest as +N (the design's 1o).
                let mut x = left;
                for (k, t) in tags.iter().enumerate() {
                    let at = egui::pos2(x, rect.top() + 61.0 - closed_up);
                    let rest = tags.len() - k;
                    let w_chip = ui.fonts_mut(|f| f.layout_no_wrap(t.to_string(), egui::FontId::proportional(11.0), pal.fg).size().x) + 12.0;
                    if k == shown_tags || (rest > 1 && x + w_chip + 34.0 > left + width) || x + w_chip > left + width {
                        chrome::more_chip(&painter, at, rest, pal.fg_dim);
                        break;
                    }
                    x = chrome::tag_chip(&painter, at, t, muted_tags.contains(*t)).right() + 5.0;
                }
                // Under the third line, or under the tags when there are.
                let mut y = base - 2.0;
                let note_y = y;
                if note_line {
                    if !noting {
                        line(format!("“{}”", w.note), y, egui::FontId::proportional(11.5), pal.fg, width);
                    }
                    y += 18.0;
                }
                if run_line {
                    let mut x = left;
                    if let Some(a) = &other_agent {
                        let g = ui.fonts_mut(|f| f.layout_no_wrap(a.clone(), egui::FontId::proportional(11.5), pal.fg));
                        let w_a = g.size().x;
                        painter.galley(egui::pos2(x, rect.top() + y), g, pal.fg);
                        x += w_a + 10.0;
                    }
                    for port in &ports {
                        let g = ui.fonts_mut(|f| f.layout_no_wrap(format!(":{port}"), egui::FontId::monospace(11.0), chrome::ink(chrome::cyan())));
                        let r = egui::Rect::from_min_size(egui::pos2(x, rect.top() + y - 1.0), g.size() + egui::vec2(8.0, 2.0));
                        if r.right() > left + width {
                            break;
                        }
                        let b = ui.interact(r, egui::Id::new(("port", w.id, *port)), egui::Sense::click()).on_hover_text(format!("Open http://localhost:{port} in the browser"));
                        painter.rect_filled(r, 4.0, if b.hovered() { crate::theme::colors().hover() } else { crate::theme::colors().panel });
                        painter.galley(egui::pos2(r.left() + 4.0, r.top() + 1.0), g, chrome::ink(chrome::cyan()));
                        if b.clicked() {
                            menu::open_url(&format!("http://localhost:{port}"));
                        }
                        x = r.right() + 5.0;
                    }
                    y += 18.0;
                }
                if let Some(n) = numbers {
                    line(n, y, egui::FontId::monospace(11.0), pal.fg_dim, width);
                }
                // The note in a field on its line, Enter to keep (empty takes
                // it off), Esc to leave it as it was.
                if let Some((id, text)) = &mut self.noting_card {
                    if *id == w.id {
                        self.fields_drawn.0 = true;
                        let at = egui::Rect::from_min_size(egui::pos2(left - 4.0, rect.top() + note_y - 3.0), egui::vec2(width - 10.0, 20.0));
                        let field = ui.put(at, egui::TextEdit::singleline(text).id(egui::Id::new(("note-card", w.id))).hint_text("A note: what this tab is for").font(egui::FontId::proportional(12.0)));
                        keep_focus(&field);
                        let (enter, esc) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
                        if enter {
                            ops.push(SideOp::Note(w.id, text.clone()));
                            self.noting_card = None;
                        } else if esc || field.lost_focus() {
                            self.noting_card = None;
                        }
                    }
                }
                // F2: the name in a field over it, Enter to keep, Esc to leave.
                if let Some((id, text)) = &mut self.renaming_card {
                    if *id == w.id {
                        self.fields_drawn.1 = true;
                        let at = egui::Rect::from_min_size(egui::pos2(left - 4.0, rect.top() + 3.0), egui::vec2(width - 10.0, 22.0));
                        let field = ui.put(at, egui::TextEdit::singleline(text).id(egui::Id::new(("rename-card", w.id))).hint_text("The tab's name").font(egui::FontId::proportional(13.5)));
                        keep_focus(&field);
                        let (enter, esc) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
                        if enter {
                            ops.push(SideOp::Rename(w.id, text.clone()));
                            self.renaming_card = None;
                        } else if esc || field.lost_focus() {
                            self.renaming_card = None;
                        }
                    }
                }
                } else {
                    // One line (the design's 1i C): the dot, the name, the
                    // project and how long; rings for what wants a person.
                    let r = rect.shrink2(egui::vec2(6.0, 1.0));
                    // A ring only for what wants a person; a running agent
                    // its line.
                    let look = chrome::look(urgent);
                    let t = match look {
                        chrome::Look::State(State::Waiting) => {
                            painter.rect_filled(r, 6.0, crate::theme::colors().wait_bg());
                            let t = self.moving(ui);
                            chrome::wait_ring(&ui.painter_at(rect.expand(3.0)), r, 6.0, t);
                            t
                        }
                        chrome::Look::State(State::Running) => {
                            let t = self.moving(ui);
                            if let Some(t) = t {
                                chrome::run_line(&painter, r, 6.0, t);
                            }
                            t
                        }
                        chrome::Look::State(State::Error) => {
                            painter.rect_stroke(r, 6.0, egui::Stroke::new(1.0, chrome::red().gamma_multiply(0.6)), egui::StrokeKind::Inside);
                            None
                        }
                        _ => None,
                    };
                    if resp.hovered() {
                        painter.rect_filled(r, 6.0, pal.selection.gamma_multiply(0.4));
                    }
                    if self.dragging_tab == Some(w.id) {
                        painter.rect_stroke(r, 6.0, egui::Stroke::new(1.0, chrome::cyan()), egui::StrokeKind::Inside);
                    }
                    chrome::state_mark(&painter, egui::pos2(r.left() + 12.0, r.center().y), look, t);
                    let since = if matches!(urgent.state, State::Running) { String::new() } else { chrome::elapsed(now.saturating_sub(urgent.since_ms)) };
                    let time_color = if urgent.state == State::Waiting { chrome::ink(chrome::gold()) } else { chrome::grey() };
                    let t = painter.text(egui::pos2(r.right() - 8.0, r.center().y), egui::Align2::RIGHT_CENTER, since, egui::FontId::proportional(11.0), time_color);
                    let project = focus.project.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    let pj = painter.text(egui::pos2(t.left() - 8.0, r.center().y), egui::Align2::RIGHT_CENTER, project, egui::FontId::proportional(11.0), chrome::grey());
                    let name_color = match urgent.state {
                        State::Waiting => crate::theme::colors().wait_text(),
                        State::Done => pal.fg_dim,
                        _ => pal.fg,
                    };
                    let mut job = egui::text::LayoutJob::simple_singleline(tab.name(), egui::FontId::proportional(12.5), name_color);
                    job.wrap = egui::text::TextWrapping::truncate_at_width((pj.left() - r.left() - 32.0).max(10.0));
                    let g = ui.fonts_mut(|f| f.layout_job(job));
                    painter.galley(egui::pos2(r.left() + 22.0, r.center().y - g.size().y / 2.0), g, name_color);
                }
                // Its last lines on the way past, without going there.
                let resp = if resp.hovered() && self.dragging_tab.is_none() && !over_changes {
                    let lines = self.peek_lines(urgent.id);
                    resp.on_hover_ui(|ui| chrome::peek(ui, &pal, &lines))
                } else {
                    resp
                };
                if resp.clicked() {
                    picked = Some(w.id);
                }
                if resp.drag_started() {
                    self.dragging_tab = Some(w.id);
                }
                // The tab's menu (the design's 1j), in four groups: how it
                // looks, starting it again, its folder, closing it.
                let ids: Vec<SessionId> = infos.iter().map(|i| i.id).collect();
                let shown_item = |word: &str| !self.menu.hide.iter().any(|h| h == word);
                // Open until a click outside it: a click into one of its
                // fields must not close it.
                let menu = egui::Popup::context_menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
                menu.show(|ui| {
                    // The design's: 300 wide, rows of about 32.
                    ui.set_min_width(300.0);
                    ui.spacing_mut().button_padding = egui::vec2(10.0, 8.0);
                    // In the settings' order (`[menu] order`), a line between
                    // groups; one's own items before "Close the session".
                    let mut last_group = None;
                    for word in tsumugi_mux::settings::menu_words(&self.menu.order) {
                        if word == "close" && !self.menu.session.is_empty() {
                            ui.separator();
                            for item in &self.menu.session {
                                if ui.button(&item.name).on_hover_text(&item.command).clicked() {
                                    menu::run(tsumugi_mux::settings::fill(&item.command, &focus.cwd, focus.id));
                                    ui.close();
                                }
                            }
                            last_group = Some("own");
                        }
                        if !shown_item(word) {
                            continue;
                        }
                        let group = tsumugi_mux::settings::menu_group(word);
                        if last_group.is_some_and(|g| g != group) {
                            ui.separator();
                        }
                        last_group = Some(group);
                        match word {
                            "rename" => {
                                match &mut self.renaming {
                                    Some((id, text)) if *id == w.id => {
                                        let edit = ui.add(egui::TextEdit::singleline(text).id(egui::Id::new(("rename", w.id))).hint_text("The tab's name").desired_width(220.0));
                                        keep_focus(&edit);
                                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                            ops.push(SideOp::Rename(w.id, text.clone()));
                                            self.renaming = None;
                                            ui.close();
                                        }
                                    }
                                    _ => {
                                        if ui.add(egui::Button::new("Rename…").shortcut_text(keys::label(keys::Action::Rename))).clicked() {
                                            self.renaming = Some((w.id, w.name.clone()));
                                        }
                                    }
                                }
                            }
                            "note" => {
                                let label = if w.note.is_empty() { "Note…" } else { "Edit the note…" };
                                if ui.button(label).on_hover_text("A line of your own on the card: what the tab is for").clicked() {
                                    self.noting_card = Some((w.id, w.note.clone()));
                                    self.fields_drawn.0 = true;
                                    ui.close();
                                }
                            }
                            "tags" => {
                                for t in &tags {
                                    if ui.button(format!("Remove tag {t}")).clicked() {
                                        ops.push(SideOp::Tag(ids.clone(), t.to_string(), false));
                                    }
                                }
                                if tags.len() < tsumugi_mux::proto::MAX_TAGS {
                                    let edit = ui.add(egui::TextEdit::singleline(&mut self.tag_input).id(egui::Id::new(("tag-input", w.id))).hint_text("Add a tag").desired_width(220.0));
                                    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                        if let Some(t) = tsumugi_mux::proto::tag_name(&self.tag_input) {
                                            ops.push(SideOp::Tag(ids.clone(), t, true));
                                        }
                                        self.tag_input.clear();
                                        edit.request_focus();
                                    }
                                } else {
                                    ui.label(egui::RichText::new(format!("{} tags at most", tsumugi_mux::proto::MAX_TAGS)).color(pal.fg_dim));
                                }
                            }
                            "mute" => {
                                let label = if muted { "Unmute notifications" } else { "Mute notifications" };
                                if ui.button(label).on_hover_text("Muted: in the bell only, no system notification or taskbar number").clicked() {
                                    ops.push(SideOp::Mute(ids.clone(), !muted));
                                    ui.close();
                                }
                            }
                            "pin" => {
                                let label = if w.pinned { "Unpin" } else { "Pin to top" };
                                if ui.button(label).clicked() {
                                    ops.push(SideOp::Pin(w.id, !w.pinned));
                                    ui.close();
                                }
                            }
                            "restart" => {
                                let label = if focus.claude { "Restart (resume the conversation)" } else { "Restart" };
                                if ui.button(label).clicked() {
                                    ops.push(SideOp::Restart(focus.id));
                                    ui.close();
                                }
                            }
                            "duplicate" => {
                                if ui.add(egui::Button::new("Duplicate in the same folder").shortcut_text(keys::label(keys::Action::Duplicate))).clicked() {
                                    ops.push(SideOp::Duplicate(focus.cwd.clone(), focus.claude));
                                    ui.close();
                                }
                            }
                            "new-window" => {
                                if ui.button("Move to a new window").clicked() {
                                    menu::new_window(w.id);
                                    ui.close();
                                }
                            }
                            "filer" => open_with(ui, "Open the folder in filer", &self.open.filer, focus),
                            "editor" => open_with(ui, "Open in the editor", &self.open.editor, focus),
                            "copy-path" => {
                                if ui.button("Copy the folder path").clicked() {
                                    ui.ctx().copy_text(focus.cwd.display().to_string());
                                    ui.close();
                                }
                            }
                            "pr" => {
                                let shown = !focus.branch.is_empty() && !matches!(focus.branch.as_str(), "main" | "master");
                                if shown && ui.button("Create a pull request").on_hover_text(format!("Push {} and open a pull request with gh, titled from its commits", focus.branch)).clicked() {
                                    ops.push(SideOp::CreatePr(focus.cwd.clone()));
                                    ui.close();
                                }
                            }
                            "save-output" => {
                                if ui.button("Save the output to a file").on_hover_text("The scrollback and the screen of the pane with the keys, as text in Downloads").clicked() {
                                    ops.push(SideOp::SaveOutput(focus.id, sort::display_title(&focus.title, &focus.command)));
                                    ui.close();
                                }
                            }
                            "close" => {
                                // Something running in it: a second click, to be sure.
                                let busy = infos.iter().any(|i| matches!(i.state, State::Running | State::MaybeWaiting));
                                let armed = self.close_armed == Some(w.id);
                                let label = if armed { "Click again to close: it is running" } else { "Close the session" };
                                if ui.add(egui::Button::new(egui::RichText::new(label).color(crate::theme::colors().err)).shortcut_text(keys::label(keys::Action::CloseTab))).clicked() {
                                    if busy && !armed {
                                        self.close_armed = Some(w.id);
                                    } else {
                                        ops.push(SideOp::Close(ids.clone()));
                                        self.close_armed = None;
                                        ui.close();
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                });
            }
        });
        // A tab being dragged: a line where it would go, and there on release.
        if let Some(dragged) = self.dragging_tab {
            let pointer = ui.ctx().pointer_latest_pos();
            let gap = pointer.map(|p| rows.iter().take_while(|(_, r)| p.y > r.center().y).count());
            if let (Some(gap), Some((_, first))) = (gap, rows.first()) {
                let y = match rows.get(gap) {
                    Some((_, r)) => r.top(),
                    None => rows.last().map_or(first.bottom(), |(_, r)| r.bottom()),
                };
                let x = first.x_range();
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                ui.painter().line_segment([egui::pos2(x.min + 8.0, y), egui::pos2(x.max - 8.0, y)], egui::Stroke::new(2.0, chrome::cyan()));
            }
            if ui.input(|i| !i.pointer.any_down()) {
                if let Some(gap) = gap {
                    if let Some(to) = drop_index(workspaces, &rows, dragged, gap) {
                        ops.push(SideOp::Move(dragged, to));
                    }
                }
                self.dragging_tab = None;
            }
        }
        if let Some(client) = &self.client {
            for op in ops {
                match op {
                    SideOp::Mute(ids, on) => client.mute(ids, on),
                    SideOp::Tag(ids, tag, on) => client.tag(ids, tag, on),
                    SideOp::MuteTag(tag, on) => client.mute_tag(tag, on),
                    SideOp::Move(id, to) => client.move_workspace(id, to),
                    SideOp::Rename(id, name) => client.rename_workspace(id, name),
                    SideOp::Note(id, note) => client.note_workspace(id, note),
                    SideOp::CreatePr(cwd) => {
                        let _ = self.jobs.0.send(Ok("Pushing and opening a pull request…".into()));
                        self.job(move || {
                            let url = gitinfo::create_pr(&cwd)?;
                            menu::open_url(&url);
                            Ok(format!("Opened the pull request {url}"))
                        });
                    }
                    SideOp::SaveOutput(id, name) => {
                        self.saving.insert(id, name);
                        client.all_text(id);
                    }
                    SideOp::Diff(title, cwd) => {
                        let ctx = ui.ctx().clone();
                        self.diff = Some(diffview::View::open(title, cwd, move || ctx.request_repaint()));
                    }
                    SideOp::Pin(id, on) => client.pin_workspace(id, on),
                    SideOp::Restart(id) => client.restart(id),
                    SideOp::Duplicate(cwd, claude) => {
                        let shell = None;
                        let typed = claude.then(|| newsession::Start::Claude.typed(&self.settings_now.sessions.claude)).flatten();
                        if let Ok(pane) = client.spawn_typing(cwd, shell, Size::new(80, 24), (8, 16), Place::NewWorkspace, typed) {
                            self.pending = Some(pane.id());
                        }
                    }
                    SideOp::Close(ids) => {
                        for id in ids {
                            client.attach(id).kill();
                        }
                    }
                }
            }
        }
        picked
    }

    /// The narrow rail (the design's 1i A): a square per tab with its
    /// project's first letter, ringed in its state's colour, its card on
    /// hover, and how many wait at the foot.
    /// Where a new session starts: the folder of the pane with the keys,
    /// else `[general] default_folder`, else the home folder.
    fn here(&self, workspaces: &[Workspace], sessions: &[Info]) -> std::path::PathBuf {
        let w = self.active.and_then(|id| workspaces.iter().find(|w| w.id == id));
        w.and_then(|w| sessions.iter().find(|i| i.id == w.focus)).map(|i| i.cwd.clone()).unwrap_or_else(|| self.default_folder())
    }

    fn default_folder(&self) -> std::path::PathBuf {
        let set = self.settings_now.general.default_folder.trim();
        if set.is_empty() {
            tsumugi_mux::settings::home().or_else(|| std::env::current_dir().ok()).unwrap_or_else(|| ".".into())
        } else {
            newsession::expand(set)
        }
    }

    /// The new-session dialog in `folder`, the settings' start chosen.
    fn dialog(&self, folder: &std::path::Path) -> newsession::Dialog {
        let start = newsession::Start::from_word(&self.settings_now.sessions.start).unwrap_or(newsession::Start::Claude);
        remote::find(self.places.clone(), self.ctx.clone());
        newsession::Dialog::starting(folder, start)
    }

    fn rail(&mut self, ui: &mut egui::Ui, workspaces: &[Workspace], sessions: &[Info]) -> Option<WorkspaceId> {
        let pal = self.palette;
        let now = chrome::now_ms();
        let tabs: Vec<sort::Tab> = workspaces.iter().map(|w| sort::Tab::new(w, sessions)).collect();
        let shown = sort::arrange(&tabs, self.view.sort, &self.filter);
        let area = ui.max_rect();
        let foot = area.bottom() - 44.0;
        let mut picked = None;
        // A new session, at the top as in the sidebar's header.
        let plus = egui::Rect::from_center_size(egui::pos2(area.center().x, area.top() + 22.0), egui::vec2(26.0, 24.0));
        if ui.scope_builder(egui::UiBuilder::new().max_rect(plus), |ui| chrome::plus_button(ui, &pal)).inner.clicked() {
            self.new_session = Some(self.dialog(&self.here(workspaces, sessions)));
        }
        let mut y = area.top() + 44.0;
        for (k, tab) in shown.iter().enumerate() {
            if y + 36.0 > foot - 22.0 {
                ui.painter().text(egui::pos2(area.center().x, y + 6.0), egui::Align2::CENTER_CENTER, format!("+{}", shown.len() - k), egui::FontId::proportional(11.0), pal.fg_dim);
                break;
            }
            let (Some(focus), Some(urgent)) = (tab.focus(), tab.urgent()) else { continue };
            let rect = egui::Rect::from_center_size(egui::pos2(area.center().x, y + 18.0), egui::vec2(36.0, 36.0));
            y += 46.0;
            let resp = ui.interact(rect, egui::Id::new(("rail-tab", tab.workspace.id)), egui::Sense::click());
            let p = ui.painter();
            let fill = if urgent.state == State::Waiting {
                crate::theme::colors().wait_bg()
            } else if Some(tab.workspace.id) == self.active {
                pal.selection
            } else if resp.hovered() {
                crate::theme::colors().hover()
            } else {
                crate::theme::colors().panel
            };
            p.rect_filled(rect, 9.0, fill);
            let look = chrome::look(urgent);
            let moves = matches!(look, chrome::Look::State(State::Waiting | State::Running));
            let t = if moves { self.moving(ui) } else { None };
            chrome::state_ring(ui.painter(), rect, 9.0, look, t);
            let p = ui.painter();
            let letter = focus.project.file_name().and_then(|f| f.to_string_lossy().chars().next()).map_or('?', |c| c.to_ascii_uppercase());
            // The letter in the state's colour (the design's rail), made to
            // read on the tile; a shell's and a finished one's grey.
            let color = match look {
                chrome::Look::State(State::Waiting) => crate::theme::colors().wait_text(),
                chrome::Look::State(State::Done) | chrome::Look::Shell => pal.fg_dim,
                l => chrome::ink(chrome::look_color(l)),
            };
            p.text(rect.center(), egui::Align2::CENTER_CENTER, letter, egui::FontId::proportional(14.0), color);
            let lines = if resp.hovered() { self.peek_lines(urgent.id) } else { Vec::new() };
            let resp = resp.on_hover_ui(|ui| {
                ui.label(egui::RichText::new(tab.name()).strong());
                let mut place = home_short(&focus.cwd);
                if !focus.branch.is_empty() {
                    place.push_str(&format!(" · {}", focus.branch));
                }
                ui.label(egui::RichText::new(place).monospace().size(11.0).color(pal.fg_dim));
                let words = chrome::card_words(urgent, now);
                if !words.is_empty() {
                    ui.label(egui::RichText::new(words).size(11.5).color(chrome::state_ink(urgent.state)));
                }
                if !lines.is_empty() {
                    ui.separator();
                    chrome::peek(ui, &pal, &lines);
                }
            });
            if resp.clicked() {
                picked = Some(tab.workspace.id);
            }
        }
        // How many wait, a click away.
        let waiting = sessions.iter().filter(|i| matches!(i.state, State::Waiting | State::MaybeWaiting)).count();
        if waiting > 0 {
            let pill = egui::Rect::from_center_size(egui::pos2(area.center().x, foot + 18.0), egui::vec2(36.0, 22.0));
            let resp = ui.interact(pill, egui::Id::new("rail-waiting"), egui::Sense::click()).on_hover_text("Jump to waiting");
            ui.painter().rect_filled(pill, 11.0, chrome::gold());
            ui.painter().text(pill.center(), egui::Align2::CENTER_CENTER, waiting.to_string(), egui::FontId::proportional(12.0), crate::theme::colors().on_accent());
            if resp.clicked() {
                self.jump_waiting = true;
            }
        }
        picked
    }

    /// The band, sidebar and status bar's fill: see-through over Mica or
    /// Acrylic, so the desktop's colour comes through; else as it is.
    fn chrome_fill(&self, c: egui::Color32) -> egui::Color32 {
        let a = self.alpha();
        if self.material { c.gamma_multiply(a.min(0.55)) } else { c.gamma_multiply(a) }
    }

    /// Quake mode's key: down from the top of the screen and given the keys,
    /// or, when it has them, away (hidden, off the taskbar too).
    fn quake(&mut self, ctx: &egui::Context) {
        let word = self.settings_now.window.quake.clone();
        if let Some(e) = self.quake.follow(ctx, &word) {
            self.say(e, true);
        }
        if !self.quake.pressed() {
            return;
        }
        let (focused, shown, monitor) = ctx.input(|i| {
            let v = i.viewport();
            (v.focused.unwrap_or(false), v.visible().unwrap_or(true) && !v.minimized.unwrap_or(false), v.monitor_size)
        });
        if focused && shown {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        if let Some(m) = monitor {
            let r = quake::drop_rect(m, self.settings_now.window.quake_height);
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(r.min));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(r.size()));
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    /// How much of the window covers the desktop (`[window] opacity`, once
    /// the window was opened see-through).
    fn alpha(&self) -> f32 {
        if self.transparent { self.settings_now.window.alpha() } else { 1.0 }
    }

    /// The worktrees' comings and goings: start the session in one just
    /// made; when the last session in one of ours ends, ask whether to
    /// remove it; say how the removing went.
    fn worktrees(&mut self, ctx: &egui::Context, client: &Client, sessions: &[Info], current: Option<&Workspace>) {
        if let Some((rx, _)) = &self.worktree_making {
            if let Ok(made) = rx.try_recv() {
                let (_, mut c) = self.worktree_making.take().expect("there");
                match made {
                    Ok(path) => {
                        self.say(format!("Worktree made: {}", home_short(&path)), false);
                        self.worktrees.push(path.clone());
                        c.folder = path;
                        self.create(client, c, current);
                    }
                    Err(e) => self.say(e, true),
                }
            }
        }
        for w in &self.worktrees {
            let alive = sessions.iter().any(|i| i.cwd.starts_with(w));
            if alive {
                self.worktree_lived.insert(w.clone());
            } else if self.worktree_lived.remove(w) && self.worktree_ask.is_none() {
                self.worktree_ask = Some(w.clone());
            }
        }
        if let Some((path, rx)) = &self.worktree_removing {
            if let Ok(done) = rx.try_recv() {
                let path = path.clone();
                self.worktree_removing = None;
                match done {
                    Ok(()) => {
                        self.say(format!("Worktree removed: {} (its branch stays)", home_short(&path)), false);
                        self.worktrees.retain(|w| *w != path);
                    }
                    Err(e) => self.say(e, true),
                }
            }
        }
        let Some(path) = self.worktree_ask.clone() else { return };
        let mut answer = None;
        egui::Area::new(egui::Id::new("worktree-ask")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
            egui::Frame::NONE
                .fill(crate::theme::colors().panel)
                .stroke(egui::Stroke::new(1.0, crate::theme::colors().border_strong()))
                .corner_radius(12.0)
                .inner_margin(egui::Margin::symmetric(20, 16))
                .show(ui, |ui| {
                    ui.set_max_width(460.0);
                    ui.label(egui::RichText::new("Remove the worktree?").size(15.0).strong().color(crate::theme::colors().strong()));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("The last session in {} has ended. Its branch stays; git refuses if anything is not committed.", home_short(&path))).size(12.5).color(crate::theme::colors().dim));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let remove = egui::Button::new(egui::RichText::new("Remove").color(crate::theme::colors().on_accent()).strong()).fill(chrome::red());
                        let (remove, keep) = chrome::foot(ui, remove, true, "Keep");
                        if remove.clicked() {
                            answer = Some(true);
                        }
                        if keep.clicked() {
                            answer = Some(false);
                        }
                    });
                });
        });
        match answer {
            Some(true) => {
                let (tx, rx) = std::sync::mpsc::channel();
                let (p, ctx) = (path.clone(), ctx.clone());
                let _ = std::thread::Builder::new().name("worktree".into()).spawn(move || {
                    let _ = tx.send(worktree::remove(&p));
                    ctx.request_repaint();
                });
                self.worktree_removing = Some((path, rx));
                self.worktree_ask = None;
            }
            Some(false) => self.worktree_ask = None,
            None => {}
        }
    }

    /// The window is closing: ask first when sessions are still running and
    /// `[general] ask_before_close` is on, and stop them all when
    /// `keep_sessions` is off (else the server keeps them for next time).
    fn close_window(&mut self, ctx: &egui::Context, client: &Client, sessions: &[Info]) {
        let general = self.settings_now.general.clone();
        if ctx.input(|i| i.viewport().close_requested()) {
            let running = sessions.iter().filter(|s| s.state == tsumugi_mux::proto::State::Running).count();
            if general.ask_before_close && running > 0 && !self.close_ok {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.closing = Some(running);
            } else if !general.keep_sessions {
                for s in sessions {
                    client.attach(s.id).kill();
                }
            }
        }
        let Some(running) = self.closing else { return };
        let mut answer = None;
        egui::Area::new(egui::Id::new("close-ask")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
            egui::Frame::NONE
                .fill(crate::theme::colors().panel)
                .stroke(egui::Stroke::new(1.0, crate::theme::colors().border_strong()))
                .corner_radius(12.0)
                .inner_margin(egui::Margin::symmetric(20, 16))
                .show(ui, |ui| {
                    ui.set_max_width(460.0);
                    let title = if running == 1 { "A session is still running".to_string() } else { format!("{running} sessions are still running") };
                    ui.label(egui::RichText::new(title).size(15.0).strong().color(crate::theme::colors().strong()));
                    ui.add_space(4.0);
                    let words = if general.keep_sessions {
                        "They go on in the background; open tsumugi again to see them."
                    } else {
                        "Closing stops them. Turn on \"Keep sessions running\" in Settings to keep them going."
                    };
                    ui.label(egui::RichText::new(words).size(12.5).color(crate::theme::colors().dim));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let fill = if general.keep_sessions { chrome::gold() } else { chrome::red() };
                        let close = egui::Button::new(egui::RichText::new("Close").color(crate::theme::colors().on_accent()).strong()).fill(fill);
                        let (close, cancel) = chrome::foot(ui, close, true, "Cancel");
                        if close.clicked() {
                            answer = Some(true);
                        }
                        if cancel.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            answer = Some(false);
                        }
                    });
                });
        });
        match answer {
            Some(true) => {
                self.closing = None;
                self.close_ok = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(false) => self.closing = None,
            None => {}
        }
    }

    /// Queued prompts go to sessions that have finished: done, or waiting
    /// with no question on their screens (a prompt into Claude Code's
    /// permission menu would answer it). One at a time: after one goes, the
    /// session's next is held for a few seconds, for it to start running.
    fn send_queued(&mut self, client: &Client, sessions: &[Info]) {
        if self.queue.is_empty() {
            return;
        }
        let ids: Vec<SessionId> = self.queue.iter().map(|q| q.id).collect();
        let mut gone = Vec::new();
        for id in ids {
            let Some(info) = sessions.iter().find(|i| i.id == id) else {
                gone.push(id);
                continue;
            };
            if matches!(info.state, State::Running | State::MaybeWaiting) {
                self.queue_hold.remove(&id);
                continue;
            }
            if self.queue_hold.get(&id).is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(5)) {
                continue;
            }
            // Waiting is ready only once its screen is here and asks no
            // question: a pane just watched has none yet, and an empty
            // screen read as "no choices" sent the prompt into Claude
            // Code's 1. Yes / 2. No (the source review, 2026-10-07).
            let ready = info.state == State::Done || info.state == State::Waiting && {
                let lines = self.watch_lines(id);
                !lines.iter().all(|l| l.trim().is_empty()) && answer::choices(&lines).is_empty()
            };
            if !ready {
                continue;
            }
            if let Some(k) = self.queue.iter().position(|q| q.id == id) {
                let q = self.queue.remove(k);
                client.send_prompt(id, q.text);
                self.queue_hold.insert(id, std::time::Instant::now());
                let name = sort::display_title(&info.title, &info.command);
                self.say(format!("Sent the queued prompt to {name}"), false);
            }
        }
        if !gone.is_empty() {
            let n = self.queue.iter().filter(|q| gone.contains(&q.id)).count();
            self.queue.retain(|q| !gone.contains(&q.id));
            self.say(format!("{n} queued prompt(s) dropped: the session ended"), true);
        }
        // Looked at again within the second, not only on output.
        self.ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }

    /// Claude Code's hooks: whether they are there (asked once at the start,
    /// on a thread), the offer to add them while they are not, and adding.
    fn hooks_offer(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.hooks_check {
            if let Ok((there, repaired)) = rx.try_recv() {
                self.hooks_check = None;
                self.hooks_offered = !there && !self.view.hooks_asked;
                match repaired {
                    Ok(Some(path)) => self.say(format!("Claude Code's hooks in {} could not find tsumugi; they run this one now (the old file kept beside it)", home_short(&path)), false),
                    Ok(None) => {}
                    Err(e) => self.say(format!("Claude Code's hooks could not find tsumugi, and were left: {e}"), true),
                }
            }
        }
        if let Some(rx) = &self.hooks_adding {
            if let Ok(done) = rx.try_recv() {
                self.hooks_adding = None;
                match done {
                    Ok(path) => self.say(format!("Claude Code's hooks added to {} (the old file kept beside it)", home_short(&path)), false),
                    Err(e) => self.say(e, true),
                }
                if self.prefs.is_some() {
                    self.read_facts();
                }
            }
        }
        // Not over the settings, which have the same question on a page, nor
        // over the new-session dialog; on the first-run screen it is in the
        // middle of it (`first_run`). Else at the bottom right, clear of the
        // status bar and the input box.
        if !self.hooks_offered || self.prefs.is_some() || self.new_session.is_some() || self.first_shown {
            return;
        }
        // Offered in the middle of the first-run screen and passed by: not
        // again over the panes in this window.
        if self.hooks_seen_first {
            self.hooks_offered = false;
            return;
        }
        let lift = if self.input.open { -150.0 } else { -40.0 };
        let answer = egui::Area::new(egui::Id::new("hooks-offer"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, lift))
            .show(ctx, |ui| hooks_card(ui, 400.0, false))
            .inner;
        self.hooks_answer(answer);
    }

    /// What was answered on the hooks' card: add them, not now, never.
    fn hooks_answer(&mut self, answer: Option<u8>) {
        match answer {
            Some(0) => {
                self.add_hooks();
                self.view.hooks_asked = true;
                self.view.save();
            }
            Some(1) => self.hooks_offered = false,
            Some(_) => {
                self.hooks_offered = false;
                self.view.hooks_asked = true;
                self.view.save();
            }
            None => {}
        }
    }

    /// No session at all: "Start your first session" (the design's First
    /// run). The folders it was started in, the default and the last ones
    /// closed; a click (or Enter on the first) starts the settings' program
    /// there; another folder opens the new-session dialog. The hooks' card
    /// under them while they are not in.
    fn first_run(&mut self, ui: &mut egui::Ui, client: &Client) {
        self.first_shown = true;
        let c = theme::colors();
        let mut folders: Vec<std::path::PathBuf> = Vec::new();
        for f in std::env::current_dir().ok().into_iter().chain([self.default_folder()]).chain(self.closed.iter().rev().cloned()) {
            if !folders.contains(&f) {
                folders.push(f);
            }
        }
        folders.truncate(5);
        let start = newsession::Start::from_word(&self.settings_now.sessions.start).unwrap_or(newsession::Start::Claude);
        let mut picked = None;
        let mut other = false;
        let enter = self.new_session.is_none() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, c.bg);
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(40.0)).layout(egui::Layout::top_down(egui::Align::Center)));
        egui::ScrollArea::vertical().id_salt("first-run").show(&mut inner, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((rect.height() * 0.12).min(80.0));
                ui.label(egui::RichText::new("Start your first session").size(26.0).strong().color(c.strong()));
                ui.add_space(6.0);
                let what = match start {
                    newsession::Start::Shell => "Pick a folder. A shell starts there, and keeps running even when this window closes.",
                    _ => "Pick a folder. Claude Code starts there, and keeps running even when this window closes.",
                };
                ui.label(egui::RichText::new(what).size(14.0).color(c.dim));
                ui.add_space(24.0);
                ui.allocate_ui(egui::vec2(520.0, 0.0), |ui| {
                    ui.set_width(520.0);
                    for (k, f) in folders.iter().enumerate() {
                        let (r, resp) = ui.allocate_exact_size(egui::vec2(520.0, 40.0), egui::Sense::click());
                        let p = ui.painter();
                        if k == 0 || resp.hovered() {
                            p.rect_filled(r, 8.0, c.chosen());
                        }
                        p.text(egui::pos2(r.left() + 14.0, r.center().y), egui::Align2::LEFT_CENTER, home_short(f), egui::FontId::monospace(13.0), c.strong());
                        if k == 0 {
                            p.text(egui::pos2(r.right() - 14.0, r.center().y), egui::Align2::RIGHT_CENTER, "↵", egui::FontId::monospace(11.0), c.dim);
                        }
                        if resp.clicked() {
                            picked = Some(f.clone());
                        }
                        ui.add_space(4.0);
                    }
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(520.0, 40.0), egui::Sense::click());
                    let p = ui.painter();
                    chrome::dashed_outline(p, r, if resp.hovered() { c.dim } else { c.border_strong() });
                    p.text(egui::pos2(r.left() + 14.0, r.center().y), egui::Align2::LEFT_CENTER, "Choose another folder…", egui::FontId::proportional(13.0), c.dim);
                    other = resp.clicked();
                });
                if self.hooks_offered {
                    ui.add_space(28.0);
                    let answer = hooks_card(ui, 560.0, true);
                    self.hooks_seen_first = true;
                    self.hooks_answer(answer);
                }
            });
        });
        if enter && picked.is_none() {
            picked = folders.first().cloned();
        }
        if let Some(folder) = picked {
            let made = newsession::Dialog::quick(&folder, start, &self.tag_rules);
            self.create(client, made, None);
        } else if other {
            self.new_session = Some(self.dialog(&folders.first().cloned().unwrap_or_else(|| self.default_folder())));
        }
    }

    /// Open the settings screen, and read what it says of the machine.
    fn open_settings(&mut self) {
        self.prefs = Some(prefs::Screen::default());
        self.read_facts();
    }

    fn read_facts(&mut self) {
        let ctx = self.ctx.clone();
        let shell = Some(self.settings_now.shell.program.clone());
        self.facts_rx = Some(facts::read(shell, move || ctx.request_repaint()));
    }

    /// Work of the settings screen's on a thread: what it says goes to the
    /// toast, and the screen's facts are read again.
    fn job(&self, work: impl FnOnce() -> Result<String, String> + Send + 'static) {
        let (tx, ctx) = (self.jobs.0.clone(), self.ctx.clone());
        let _ = std::thread::Builder::new().name("settings-job".into()).spawn(move || {
            let _ = tx.send(work());
            ctx.request_repaint();
        });
    }

    /// "Start in parallel": the dialog while open; then the worktrees made
    /// on a thread, one after another (git locks the repository), and a tab
    /// with Claude Code on its prompt in each as it comes.
    fn start_parallel(&mut self, ctx: &egui::Context, client: &Client) {
        if let Some(view) = &mut self.parallel {
            match parallel::show(ctx, view, &theme::colors()) {
                Some(parallel::Answer::Start(start)) => {
                    self.parallel = None;
                    let (tx, rx) = std::sync::mpsc::channel();
                    let wake = ctx.clone();
                    let stamp = worktree::default_branch(chrono::Local::now());
                    let _ = std::thread::Builder::new().name("parallel".into()).spawn(move || {
                        for (k, task) in start.tasks.into_iter().enumerate() {
                            let made = worktree::add(&start.folder, &parallel::branch(&stamp, k)).map(|path| (path, task));
                            let failed = made.is_err();
                            let _ = tx.send(made);
                            wake.request_repaint();
                            if failed {
                                break;
                            }
                        }
                    });
                    self.parallel_made = Some(rx);
                }
                Some(parallel::Answer::Close) => self.parallel = None,
                None => {}
            }
        }
        let Some(rx) = &self.parallel_made else { return };
        let made: Vec<Result<(std::path::PathBuf, String), String>> = rx.try_iter().collect();
        let program = self.settings_now.shell.command().map(|(p, _)| p).unwrap_or_else(tsumugi_pane::default_program);
        let how = tsumugi_pane::Quoting::for_shell(Some(&program));
        let claude = newsession::Start::Claude.typed(&self.settings_now.sessions.claude).unwrap_or_else(|| "claude".into());
        for m in made {
            match m {
                Ok((path, task)) => {
                    if let Err(e) = client.spawn_typing(path, None, Size::new(80, 24), (8, 16), Place::NewWorkspace, Some(parallel::typed(&claude, &task, how))) {
                        self.say(format!("A parallel session did not start: {e}"), true);
                    }
                }
                Err(e) => self.say(format!("Start in parallel stopped: {e}"), true),
            }
        }
    }

    /// Write a session's output to Downloads on a thread; a toast says where.
    fn save_output(&self, name: String, text: String) {
        self.job(move || {
            let to = export::place().ok_or("no home folder to save in")?;
            export::save_output(&to, &name, &text).map(|f| format!("Saved the output to {}", f.display()))
        });
    }

    fn save_profiles(&self) {
        let all = self.profiles.clone();
        let _ = std::thread::Builder::new().name("profiles".into()).spawn(move || {
            if let Some(path) = tsumugi_mux::settings::profiles_path() {
                let _ = tsumugi_mux::settings::save_profiles(&path, &all);
            }
        });
    }

    /// A tag's new name: in the rules and the colours of the settings, on the
    /// sessions that have it, and quiet if it was.
    fn rename_tag(&mut self, client: &Client, sessions: &[Info], old: String, new: String) {
        let ids: Vec<SessionId> = sessions.iter().filter(|i| i.tags.contains(&old)).map(|i| i.id).collect();
        if !ids.is_empty() {
            client.tag(ids.clone(), old.clone(), false);
            client.tag(ids, new.clone(), true);
        }
        if client.muted_tags().contains(&old) {
            client.mute_tag(old.clone(), false);
            client.mute_tag(new.clone(), true);
        }
        let s = &self.settings_now.tags;
        let rules: Vec<tsumugi_mux::settings::TagRule> = s.rule.iter().cloned().map(|mut r| {
            if r.tag == old {
                r.tag = new.clone();
            }
            r
        }).collect();
        let colour = s.colors.get(&old).cloned();
        let key = |n: &str| if n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') { n.to_owned() } else { tsumugi_mux::settings::quote(n) };
        let (old_key, new_key) = (key(&old), key(&new));
        edit_settings(self.settings_tx.clone(), move |t| {
            let mut t = write_rules(t, &rules)?;
            if let Some(c) = colour {
                t = tsumugi_mux::settings::remove_key(&t, Some("tags.colors"), &old_key)?;
                t = tsumugi_mux::settings::set_key(&t, Some("tags.colors"), &new_key, &tsumugi_mux::settings::quote(&c))?;
            }
            Ok(t)
        });
    }

    /// Stop the server and start it again: the sessions are written down and
    /// come back, as after an update.
    fn restart_server(&mut self, ctx: &egui::Context) {
        self.prefs = None;
        self.panes.clear();
        self.client = None;
        self.failed = Some("Restarting the server…".into());
        let ctx = ctx.clone();
        self.replacing = Some(replace_server(move || ctx.request_repaint()));
    }

    fn add_hooks(&mut self) {
        self.hooks_offered = false;
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        let _ = std::thread::Builder::new().name("hooks".into()).spawn(move || {
            let _ = tx.send(hooks::install());
            ctx.request_repaint();
        });
        self.hooks_adding = Some(rx);
    }

    /// Show `words` under the band for a while.
    /// A Ctrl+click on an address or a file's path a pane printed.
    fn open_link(&mut self, link: tsumugi_pane::Link, cwd: &std::path::Path) {
        let (path, line, column) = match link {
            tsumugi_pane::Link::Url(url) => match menu::file_uri(&url) {
                Some(path) => (path, None, None),
                None => {
                    if !menu::open_url(&url) {
                        self.say(format!("Not opened: {url} has characters the shell would read"), true);
                    }
                    return;
                }
            },
            tsumugi_pane::Link::Path { path, line, column } => (path, line, column),
        };
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(std::path::PathBuf::from);
        let path = menu::resolve(&path, cwd, home.as_deref());
        menu::open_path(path, line, column, self.open.file.clone(), self.jobs.0.clone());
    }

    fn say(&mut self, words: String, error: bool) {
        self.toast = Some((words, std::time::Instant::now(), error));
    }

    fn show_toast(&mut self, ctx: &egui::Context) {
        let Some((words, at, error)) = &self.toast else { return };
        let left = std::time::Duration::from_secs(if *error { 10 } else { 5 }).saturating_sub(at.elapsed());
        if left.is_zero() {
            self.toast = None;
            return;
        }
        // In and out over a moment, when things move at all.
        let shown = at.elapsed().as_secs_f32();
        let alpha = if self.settings_now.appearance.animations { (shown / FADE.as_secs_f32()).min(left.as_secs_f32() / FADE.as_secs_f32()).clamp(0.0, 1.0) } else { 1.0 };
        ctx.request_repaint_after(if alpha < 1.0 { std::time::Duration::from_millis(16) } else { left.saturating_sub(FADE) });
        let c = crate::theme::colors();
        let (fill, color) = if *error { (crate::theme::mix(c.panel, c.err, 0.15), chrome::red()) } else { (c.raised(), c.strong()) };
        egui::Area::new(egui::Id::new("toast")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 52.0)).show(ctx, |ui| {
            ui.set_opacity(alpha);
            egui::Frame::NONE.fill(fill).stroke(egui::Stroke::new(1.0, c.border)).corner_radius(6.0).inner_margin(egui::Margin::symmetric(12, 6)).show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(words.as_str()).size(12.5).color(color)).wrap_mode(egui::TextWrapMode::Extend));
            });
        });
    }

    /// The last lines of session `id`'s screen, for the card's preview: the
    /// session is watched while the pointer is on it (kept among the panes
    /// on screen, so a pane shown there is not watched twice).
    fn peek_lines(&mut self, id: SessionId) -> Vec<String> {
        self.peek = Some(id);
        if let Some(client) = &self.client {
            self.panes.entry(id).or_insert_with(|| client.attach(id));
        }
        let Some(pane) = self.panes.get(&id) else { return Vec::new() };
        last_lines(&tsumugi_pane::Pane::screen(pane).rows, PEEK_LINES)
    }

    /// The last lines of a waiting session's screen, to find a question
    /// in: watched while it waits.
    fn watch_lines(&mut self, id: SessionId) -> Vec<String> {
        self.watching.push(id);
        if let Some(client) = &self.client {
            self.panes.entry(id).or_insert_with(|| client.attach(id));
        }
        let Some(pane) = self.panes.get(&id) else { return Vec::new() };
        last_lines(&tsumugi_pane::Pane::screen(pane).rows, 20)
    }

    /// The panes' faces: the regular one, and bold and italic where found.
    fn faces(&self) -> tsumugi_pane::Faces {
        let size = self.font.size;
        let face = |k: usize, name: &str| self.faces_found[k].then(|| egui::FontId::new(size, egui::FontFamily::Name(name.into())));
        let shaper = self.shaper.clone().filter(|_| self.settings_now.font.ligatures);
        tsumugi_pane::Faces { regular: self.font.clone(), bold: face(0, fonts::BOLD), italic: face(1, fonts::ITALIC), bold_italic: face(2, fonts::BOLD_ITALIC), shaper, cursor: tsumugi_pane::CursorStyle::from_word(&self.settings_now.appearance.cursor) }
    }

    /// A pane too small for a terminal: its state's dot and its name, along
    /// the strip's length (turned on its side when it is tall and thin).
    fn strip(&self, ui: &egui::Ui, rect: egui::Rect, info: Option<&Info>, focused: bool) -> egui::Response {
        let resp = ui.interact(rect, egui::Id::new(("strip", info.map(|i| i.id))), egui::Sense::click_and_drag());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 0.0, (if focused { crate::theme::colors().hover() } else { crate::theme::colors().panel }).gamma_multiply(self.alpha()));
        let Some(info) = info else { return resp };
        let name = sort::display_title(&info.title, &info.command);
        let color = if focused { self.palette.fg } else { self.palette.fg_dim };
        let upright = rect.height() > rect.width() * 1.5;
        let room = if upright { rect.height() } else { rect.width() } - 28.0;
        let galley = ui.fonts_mut(|f| {
            let mut job = egui::text::LayoutJob::simple_singleline(name, egui::FontId::proportional(12.0), color);
            job.wrap = egui::text::TextWrapping::truncate_at_width(room.max(8.0));
            f.layout_job(job)
        });
        let size = galley.size();
        if upright {
            // Top to bottom, the dot above the name.
            let x = rect.center().x;
            p.circle_filled(egui::pos2(x, rect.top() + 12.0), 4.0, state_color(info.state));
            let at = egui::pos2(x + size.y / 2.0, rect.top() + 22.0);
            p.add(egui::epaint::TextShape::new(at, galley, color).with_angle(std::f32::consts::FRAC_PI_2));
        } else {
            let y = rect.center().y;
            p.circle_filled(egui::pos2(rect.left() + 12.0, y), 4.0, state_color(info.state));
            p.galley(egui::pos2(rect.left() + 22.0, y - size.y / 2.0), galley, color);
        }
        resp.on_hover_text(chrome::state_words(info, chrome::now_ms()))
    }

    /// A pane carried by its header: where it would land, shown on the pane
    /// under the pointer, and on release the tab's new shape (1f).
    fn carry(&mut self, ui: &egui::Ui, w: &Workspace, rects: &[(SessionId, Rect)]) {
        let Some((tab, moving)) = self.moving else { return };
        if tab != w.id || !rects.iter().any(|(id, _)| *id == moving) {
            self.moving = None;
            return;
        }
        let ctx = ui.ctx().clone();
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        let (pointer, released) = ctx.input(|i| (i.pointer.latest_pos(), !i.pointer.primary_down()));
        // Its name with the pointer, so what is carried is plain.
        if let Some(p) = pointer {
            let name = self.client.as_ref().and_then(|c| c.sessions().into_iter().find(|i| i.id == moving)).map(|i| sort::display_title(&i.title, &i.command)).unwrap_or_default();
            egui::Area::new(egui::Id::new("carried")).order(egui::Order::Tooltip).fixed_pos(p + egui::vec2(14.0, 12.0)).interactable(false).show(&ctx, |ui| {
                egui::Frame::NONE.fill(crate::theme::colors().raised()).stroke(egui::Stroke::new(1.0, chrome::cyan())).corner_radius(6.0).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
                    ui.label(egui::RichText::new(name).size(12.0).color(crate::theme::colors().strong()));
                });
            });
        }
        // On the sidebar, with others in the tab: a tab of its own there.
        let side = self.side_rect.filter(|_| rects.len() > 1);
        if let (Some(side), Some(p)) = (side, pointer) {
            if side.contains(p) {
                let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("own-tab")));
                let show = side.shrink(6.0);
                painter.rect_filled(show, 8.0, chrome::cyan().gamma_multiply(0.12));
                painter.rect_stroke(show, 8.0, egui::Stroke::new(1.5, chrome::cyan()), egui::StrokeKind::Inside);
                painter.text(show.center(), egui::Align2::CENTER_CENTER, "A tab of its own", egui::FontId::proportional(13.0), chrome::cyan());
                if released {
                    if let Some(client) = &self.client {
                        client.own_tab(moving);
                    }
                    self.moving = None;
                }
                return;
            }
        }
        let target = pointer.and_then(|p| rects.iter().find(|(id, r)| *id != moving && from_rect(*r).contains(p)).map(|(id, r)| (p, *id, from_rect(*r))));
        if let Some((p, to, r)) = target {
            let z = drop::zone(r, p);
            let show = drop::preview(r, z).shrink(2.0);
            let painter = ui.painter();
            painter.rect_filled(show, 6.0, chrome::cyan().gamma_multiply(0.16));
            painter.rect_stroke(show, 6.0, egui::Stroke::new(1.5, chrome::cyan()), egui::StrokeKind::Inside);
            let words = if z == drop::Zone::Swap { "Swap" } else { "Move here" };
            painter.text(show.center(), egui::Align2::CENTER_CENTER, words, egui::FontId::proportional(13.0), chrome::cyan());
            if let (true, Some(client)) = (released, &self.client) {
                let layout = match z {
                    drop::Zone::Swap => {
                        let mut l = w.layout.clone();
                        l.swap(&moving, &to);
                        l
                    }
                    drop::Zone::Side(toward) => w.layout.clone().moved(&moving, &to, toward),
                };
                client.set_layout(w.id, layout, moving);
            }
        }
        if released {
            self.moving = None;
        }
    }

    /// The tab's panes in `area`, each with its own view, and the dividers
    /// between them to drag.
    fn panes(&mut self, ui: &mut egui::Ui, w: &Workspace, sessions: &[Info]) {
        // The panes are cards with room round them (the design's Main).
        let area = to_rect(ui.max_rect().shrink(MARGIN));
        // A drag whose release was never seen here (the zoom or the tab
        // changed under it, or the panes changed) is let go, so the tab
        // does not keep showing the split as it was (the source review,
        // 2026-10-07).
        let held = ui.input(|i| i.pointer.primary_down());
        if self.dragging.as_ref().is_some_and(|(id, l)| !held || *id != w.id || self.zoom || l.leaves() != w.layout.leaves()) {
            self.dragging = None;
        }
        let layout = match &self.dragging {
            Some((id, l)) if *id == w.id => l.clone(),
            _ => w.layout.clone(),
        };
        let rects = if self.zoom { vec![(w.focus, area)] } else { layout.layout(area, GAP) };
        let alpha = self.alpha();
        if self.transparent {
            let holes: Vec<egui::Rect> = rects.iter().map(|(_, r)| from_rect(*r)).collect();
            ui.painter().add(backdrop::around(ui.max_rect(), &holes, self.palette.on_cursor.gamma_multiply(alpha)));
        }
        // The picture behind the panes, and the panes' colour over it.
        let image = self.backdrop.texture(ui.ctx(), &self.settings_now.window.image).cloned();
        if let Some(e) = self.backdrop.error.take() {
            self.say(e, true);
        }
        let mut pal = self.palette;
        let through = if image.is_some() { f32::from(self.settings_now.window.image_opacity.min(100)) / 100.0 } else { 0.0 };
        pal.bg = pal.bg.gamma_multiply(alpha * (1.0 - through));
        // Attach what is on screen, and let the rest go.
        if let Some(client) = &self.client {
            for (id, _) in &rects {
                self.panes.entry(*id).or_insert_with(|| client.attach(*id));
            }
        }
        let (peek, watching) = (self.peek, std::mem::take(&mut self.watching));
        self.panes.retain(|id, _| rects.iter().any(|(r, _)| r == id) || Some(*id) == peek || watching.contains(id));
        self.views.retain(|id, _| rects.iter().any(|(r, _)| r == id));

        let row_h = (ui.fonts_mut(|f| f.row_height(&self.font)) * self.settings_now.font.line_height).ceil();
        let faces = self.faces();
        // What came into view: the tab when it changed, a pane when it
        // appeared (a split, a new session). Each fades in over a moment.
        let now_t = std::time::Instant::now();
        let first_frame = self.shown_panes.is_empty() && self.shown_tab.is_none();
        if self.shown_tab != Some(w.id) {
            if !first_frame {
                self.tab_at = Some(now_t);
            }
            self.shown_tab = Some(w.id);
        }
        for (id, _) in &rects {
            if !self.shown_panes.contains(id) && !first_frame && self.tab_at.is_none_or(|t| t.elapsed() > FADE) {
                self.pane_at.insert(*id, now_t);
            }
        }
        self.shown_panes = rects.iter().map(|(id, _)| *id).collect();
        self.pane_at.retain(|_, t| t.elapsed() < FADE);
        let ctx = ui.ctx().clone();
        let mut focus_to = None;
        // Every pane has its heading, one alone too (the design's Main and
        // InputBox); only a split folds a pane too small into a strip.
        let split = rects.len() > 1;
        let now = chrome::now_ms();
        let cell_w = ui.fonts_mut(|f| f.glyph_width(&self.font, 'M'));
        for (id, r) in &rects {
            let mut rect = from_rect(*r);
            let card = rect;
            let focused = *id == w.focus;
            // Narrower than 20 columns or lower than 4 rows: no room for a
            // terminal, so a strip with its name and state (1f), which is
            // also the handle to carry it by.
            let narrow = split && (rect.width() < 20.0 * cell_w || rect.height() - HEADER < 4.0 * row_h);
            if narrow {
                let info = sessions.iter().find(|i| i.id == *id);
                let resp = self.strip(ui, rect, info, focused);
                if resp.drag_started() {
                    self.moving = Some((w.id, *id));
                } else if resp.clicked() && !focused {
                    focus_to = Some(*id);
                }
            }
            if !narrow {
                // Each pane of a split says what it is (the design's 1f): its
                // program or title, its folder, and what it is doing.
                let head = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), HEADER));
                rect.min.y += HEADER;
                let p = ui.painter_at(head);
                // Not lit for the keys (the design's Focus, B is not used):
                // the dimming and the cursor say it.
                // Square when see-through: the corners would show the desktop.
                let round = if self.transparent { 0 } else { 8 };
                p.rect_filled(head, egui::CornerRadius { nw: round, ne: round, sw: 0, se: 0 }, self.palette.on_cursor.gamma_multiply(alpha));
                p.hline(head.x_range(), head.bottom() - 0.5, egui::Stroke::new(1.0, crate::theme::colors().border));
                if let Some(info) = sessions.iter().find(|i| i.id == *id) {
                    // The state's mark on the left; its words, short, on the
                    // right (none for a shell); the name and folder get what
                    // is left, cut short rather than run into it.
                    let look = chrome::look(info);
                    let t = if look == chrome::Look::State(State::Running) { self.moving(ui) } else { None };
                    chrome::state_mark(&p, egui::pos2(head.left() + 14.0, head.center().y), look, t);
                    let mut right_x = head.right() - 10.0;
                    // Typing into every pane: said on each, in gold.
                    if self.typing_all.contains(&w.id) {
                        let r = p.text(egui::pos2(right_x, head.center().y), egui::Align2::RIGHT_CENTER, "TYPING INTO ALL", egui::FontId::proportional(10.5), chrome::ink(chrome::gold()));
                        right_x = r.left() - 10.0;
                    }
                    // Being recorded to a .cast file (Ctrl+Shift+R stops).
                    if !info.recording.is_empty() {
                        let r = p.text(egui::pos2(right_x, head.center().y), egui::Align2::RIGHT_CENTER, "● REC", egui::FontId::proportional(10.5), chrome::ink(chrome::red()));
                        right_x = r.left() - 10.0;
                    }
                    // A character set other than UTF-8, said where it applies.
                    if !info.charset.is_empty() && info.charset != "UTF-8" {
                        let r = p.text(egui::pos2(right_x, head.center().y), egui::Align2::RIGHT_CENTER, &info.charset, egui::FontId::monospace(10.5), chrome::ink(chrome::gold()));
                        right_x = r.left() - 10.0;
                    }
                    if self.zoom {
                        right_x = chrome::zoom_mark(&p, egui::pos2(right_x, head.center().y), self.palette.fg_dim).left() - 10.0;
                    }
                    let words = chrome::short_words(info, now);
                    let right = p.text(egui::pos2(right_x, head.center().y), egui::Align2::RIGHT_CENTER, words, egui::FontId::proportional(11.5), chrome::state_ink(info.state));
                    let name = sort::display_title(&info.title, &info.command);
                    let color = if focused { self.palette.fg } else { self.palette.fg_dim };
                    let room = (right.left() - head.left() - 40.0).max(0.0);
                    let galley = ui.fonts_mut(|f| {
                        let mut job = egui::text::LayoutJob::default();
                        job.append(&name, 0.0, egui::TextFormat::simple(egui::FontId::proportional(12.0), color));
                        job.append(&home_short(&info.cwd), 10.0, egui::TextFormat::simple(egui::FontId::monospace(11.0), self.palette.fg_dim));
                        job.wrap = egui::text::TextWrapping::truncate_at_width(room);
                        f.layout_job(job)
                    });
                    p.galley(egui::pos2(head.left() + 26.0, head.center().y - galley.size().y / 2.0), galley, color);
                }
                // The header is the handle to carry the pane by (1f).
                let resp = ui.interact(head, egui::Id::new(("pane-head", id)), egui::Sense::click_and_drag());
                if resp.drag_started() {
                    self.moving = Some((w.id, *id));
                } else if resp.clicked() && !focused {
                    focus_to = Some(*id);
                }
                if resp.hovered() && self.moving.is_none() {
                    ctx.set_cursor_icon(egui::CursorIcon::Grab);
                }
            }
            // The input box inside the pane with the keys, 12 from its edges
            // (the design's InputBox); the terminal gets the room above it.
            let boxed = !narrow && focused && self.input.open && self.prefs.is_none();
            let box_rect = boxed.then(|| {
                let h = self.input_h.clamp(120.0, (rect.height() - 4.0 * row_h).max(120.0));
                let r = egui::Rect::from_min_max(egui::pos2(rect.left() + 12.0, rect.bottom() - 12.0 - h), egui::pos2(rect.right() - 12.0, rect.bottom() - 12.0));
                rect.max.y = r.top() - 8.0;
                r
            });
            if !narrow {
                let (Some(pane), view) = (self.panes.get_mut(id), self.views.entry(*id).or_default()) else { continue };
                let opts = ViewOptions { focused, wheel: true };
                if self.transparent && rect.bottom() < card.bottom() {
                    // Round the input box: the window's colour, as when solid.
                    let below = egui::Rect::from_min_max(egui::pos2(card.left(), rect.bottom()), card.max);
                    ui.painter().rect_filled(below, 0.0, self.palette.on_cursor.gamma_multiply(alpha));
                }
                if let Some(t) = &image {
                    backdrop::paint(&ui.painter_at(rect), rect, t, alpha);
                }
                let shown = ui.push_id(id, |ui| tsumugi_pane::show_faces(ui, Some(pane), view, rect, &faces, row_h, &pal, opts)).inner;
                if !focused {
                    // The panes without the keys sit back; their marks do not (1e).
                    let dim = f32::from(self.settings_now.appearance.dim) / 100.0;
                    ui.painter().rect_filled(rect, 0.0, self.palette.on_cursor.gamma_multiply(dim));
                }
                if shown.focus && !focused {
                    focus_to = Some(*id);
                }
                // Copy mode's cursor, and what its keys do.
                if let (Some((on, mode)), Some((origin, cell))) = (self.copy_mode, shown.grid) {
                    if on == *id {
                        let at = origin + egui::vec2(mode.cursor.0 as f32 * cell.x, mode.cursor.1 as f32 * cell.y);
                        let p = ui.painter_at(rect);
                        p.rect_stroke(egui::Rect::from_min_size(at, cell), 1.0, egui::Stroke::new(2.0, chrome::gold()), egui::StrokeKind::Inside);
                        let words = if mode.marking { "COPY · selecting · y copies · v drops it · Esc leaves" } else { "COPY · arrows or hjkl move · v selects · y copies the line · Esc leaves" };
                        let galley = p.layout_no_wrap(words.into(), egui::FontId::proportional(11.5), crate::theme::colors().on_accent());
                        let badge = egui::Rect::from_min_size(egui::pos2(rect.right() - galley.size().x - 22.0, rect.top() + 6.0), galley.size() + egui::vec2(14.0, 6.0));
                        p.rect_filled(badge, 6.0, chrome::gold());
                        p.galley(badge.min + egui::vec2(7.0, 3.0), galley, egui::Color32::WHITE);
                    }
                }
                if let Some(bar) = self.find.as_mut().filter(|b| b.id == *id) {
                    if let Some(said) = pane.take_found() {
                        bar.said = Some(said);
                    }
                    for ask in find::show(&ctx, rect, bar, &theme::colors()) {
                        match ask {
                            find::Ask::Find { needle, back } => pane.find(&needle, back),
                            find::Ask::Restart => {
                                pane.find("", true);
                                tsumugi_pane::Pane::clear_selection(pane);
                            }
                            find::Ask::Close => {
                                pane.find("", true);
                                tsumugi_pane::Pane::clear_selection(pane);
                                self.find = None;
                                break;
                            }
                        }
                    }
                }
                if let Some(text) = shown.copy {
                    ctx.copy_text(text);
                }
                if shown.paste {
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }
                if let Some(link) = shown.open {
                    let cwd = sessions.iter().find(|i| i.id == *id).map(|i| i.cwd.clone()).unwrap_or_default();
                    self.open_link(link, &cwd);
                }
            }
            if let Some(at) = box_rect {
                let info = sessions.iter().find(|i| i.id == *id);
                let named = |i: &tsumugi_mux::Info| {
                    let n = sort::display_title(&i.title, &i.command);
                    let project = i.project.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    if project.is_empty() { n } else { format!("{n} · {project}") }
                };
                let name = info.map(named);
                let mut tags: Vec<String> = Vec::new();
                for t in sessions.iter().flat_map(|i| &i.tags) {
                    if !tags.contains(t) {
                        tags.push(t.clone());
                    }
                }
                let others: Vec<(SessionId, String)> = sessions.iter().filter(|i| i.id != *id).map(|i| (i.id, named(i))).collect();
                let colors = theme::colors();
                let queued = self.queue.iter().filter(|q| q.id == *id).count();
                let shown = ui.scope_builder(egui::UiBuilder::new().max_rect(at).layout(egui::Layout::bottom_up(egui::Align::Min)), |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| self.input.show(ui, &colors, *id, name.as_deref().unwrap_or("this pane"), &tags, &others, queued)).inner
                });
                self.input_h = shown.response.rect.height().max(1.0);
                if let Some(send) = shown.inner {
                    self.input_sent = Some(send);
                }
            }
            // Coming in: the pane's colour over it, thinning to nothing.
            if self.settings_now.appearance.animations {
                let since = [self.tab_at, self.pane_at.get(id).copied()].into_iter().flatten().map(|t| t.elapsed()).min();
                if let Some(since) = since.filter(|s| *s < FADE) {
                    let left = 1.0 - since.as_secs_f32() / FADE.as_secs_f32();
                    ui.painter().rect_filled(from_rect(*r), 0.0, self.palette.bg.gamma_multiply(left * left));
                    ctx.request_repaint();
                }
            }
            // Every pane is ringed by its state, the one with the keys too
            // (the design's States): a waiting one breathes in gold, so it
            // is seen while working in another.
            if let Some(info) = sessions.iter().find(|i| i.id == *id) {
                let look = chrome::look(info);
                let whole = from_rect(*r);
                let t = if matches!(look, chrome::Look::State(State::Waiting | State::Running)) { self.moving(ui) } else { None };
                chrome::state_ring(&ui.painter_at(whole.expand(4.0)), whole, 8.0, look, t);
            }
        }
        if let Some(id) = focus_to {
            self.set_focus(w, id);
        }
        self.carry(ui, w, &rects);

        if self.zoom {
            // A pane hidden by the zoom that wants a person: a gold-ringed
            // note at the bottom right with the key to go there, which a
            // click does too (the design's Splits).
            let hidden = layout.leaves().into_iter().filter(|id| *id != w.focus);
            let waiting = hidden.filter(|id| sessions.iter().any(|i| i.id == *id && i.state == State::Waiting)).count();
            if waiting > 0 {
                let key = keys::label(keys::Action::NextWaiting);
                let text = if key == "none" { format!("{waiting} waiting behind") } else { format!("{waiting} waiting behind · {key}") };
                let galley = ui.fonts_mut(|f| f.layout_no_wrap(text, egui::FontId::proportional(12.0), chrome::ink(chrome::gold())));
                let size = galley.size() + egui::vec2(20.0, 12.0);
                let note = egui::Rect::from_min_size(egui::pos2(area.x + area.w - 12.0 - size.x, area.y + area.h - 12.0 - size.y), size);
                let resp = ui.interact(note, egui::Id::new("waiting-behind"), egui::Sense::click());
                let p = ui.painter();
                p.rect_filled(note, 8.0, crate::theme::colors().wait_bg());
                chrome::wait_ring(p, note, 8.0, None);
                p.galley(note.min + egui::vec2(10.0, 6.0), galley, chrome::gold());
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    self.jump_waiting = true;
                }
            }
            return;
        }

        // The dividers (tsumugi-layout's, shared with filer): only a gap
        // until the pointer comes near; dragged, the split follows;
        // double-clicked, it halves.
        let look = tsumugi_layout::ui::Look { gap: GAP, grab: GRAB, line: self.palette.cursor };
        match tsumugi_layout::ui::dividers(ui, ui.id(), &layout, area, look) {
            Some(tsumugi_layout::ui::Moved::Dragging(l)) => self.dragging = Some((w.id, l)),
            Some(tsumugi_layout::ui::Moved::Halved(l)) => {
                if let Some(client) = &self.client {
                    client.set_layout(w.id, l, w.focus);
                }
            }
            Some(tsumugi_layout::ui::Moved::Released) => {
                if let (Some((_, l)), Some(client)) = (self.dragging.take(), &self.client) {
                    client.set_layout(w.id, l, w.focus);
                }
            }
            None => {}
        }
    }
}

use tsumugi_layout::ui::{from_egui as to_rect, to_egui as from_rect};

use chrome::state_color;


/// `pwsh` out of `C:\Program Files\PowerShell\7\pwsh.exe`.
pub(crate) fn program_name(command: &str) -> String {
    sort::program_name(command)
}

/// A folder with the home folder said as `~`.
pub(crate) fn home_short(path: &std::path::Path) -> String {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(std::path::PathBuf::from);
    match home.as_deref().and_then(|h| path.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => path.display().to_string(),
    }
}

/// What the settings thread read.
enum Read {
    // Boxed: the settings are much the largest.
    Settings(Box<Result<tsumugi_mux::settings::Settings, String>>),
    Profiles(Result<Vec<tsumugi_mux::settings::Profile>, String>),
    /// Themes of one's own (`themes/*.toml`) and the changes to the theme
    /// in force (`theme.toml`), as their files' tables.
    Themes(Result<ThemeFiles, String>),
}

#[derive(Default)]
struct ThemeFiles {
    own: Vec<(String, std::collections::BTreeMap<String, toml::Value>)>,
    changes: Option<std::collections::BTreeMap<String, toml::Value>>,
}

/// `theme.toml` and `themes/*.toml` beside the settings; a theme's name is
/// its `name`, else its file's.
fn read_themes(dir: &std::path::Path) -> Result<ThemeFiles, String> {
    let table = |p: &std::path::Path| -> Result<std::collections::BTreeMap<String, toml::Value>, String> {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        toml::from_str(&text).map_err(|e| format!("{}: {}", p.display(), e.message()))
    };
    let mut out = ThemeFiles::default();
    let changes = dir.join("theme.toml");
    if changes.exists() {
        out.changes = Some(table(&changes)?);
    }
    if let Ok(entries) = std::fs::read_dir(dir.join("themes")) {
        let mut files: Vec<std::path::PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "toml" || x == "json" || x == "itermcolors")).collect();
        files.sort();
        for f in files {
            // Other terminals' schemes, as they come (1k).
            let stem = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let read = || std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()));
            match f.extension().and_then(|x| x.to_str()) {
                Some("json") => {
                    for s in import::windows_terminal(&read()?, &stem).map_err(|e| format!("{}: {e}", f.display()))? {
                        out.own.push((s.name.clone(), import::table(&s)));
                    }
                    continue;
                }
                Some("itermcolors") => {
                    let s = import::iterm(&read()?, &stem).map_err(|e| format!("{}: {e}", f.display()))?;
                    out.own.push((s.name.clone(), import::table(&s)));
                    continue;
                }
                _ => {}
            }
            let t = table(&f)?;
            let name = t.get("name").and_then(|v| v.as_str()).map(str::to_owned).unwrap_or_else(|| f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
            out.own.push((name, t));
        }
    }
    Ok(out)
}

/// When the theme files last changed, to read them again only then.
fn themes_stamp(dir: &std::path::Path) -> Vec<(std::path::PathBuf, Option<std::time::SystemTime>)> {
    let mut out = vec![(dir.join("theme.toml"), tsumugi_mux::settings::stamp(&dir.join("theme.toml")))];
    if let Ok(entries) = std::fs::read_dir(dir.join("themes")) {
        for e in entries.flatten() {
            out.push((e.path(), tsumugi_mux::settings::stamp(&e.path())));
        }
    }
    out.sort();
    out
}

/// Read `settings.toml`, `profiles.toml` and the theme files now and
/// whenever they change, on a thread of its own (no disk on the window's
/// thread).
fn watch_settings(ctx: egui::Context, tx: std::sync::mpsc::Sender<Read>) {
    use tsumugi_mux::settings;
    let _ = std::thread::Builder::new().name("settings".into()).spawn(move || {
        let (Some(path), Some(profiles)) = (settings::default_path(), settings::profiles_path()) else { return };
        let (mut seen, mut seen_profiles, mut seen_themes) = (None, None, None);
        loop {
            let stamp = settings::stamp(&path);
            let mut sent = false;
            if seen != Some(stamp) {
                seen = Some(stamp);
                sent = true;
                if tx.send(Read::Settings(Box::new(settings::load(&path)))).is_err() {
                    return;
                }
            }
            let stamp = settings::stamp(&profiles);
            if seen_profiles != Some(stamp) {
                seen_profiles = Some(stamp);
                sent = true;
                if tx.send(Read::Profiles(settings::load_profiles(&profiles))).is_err() {
                    return;
                }
            }
            if let Some(dir) = path.parent() {
                let stamp = themes_stamp(dir);
                if seen_themes.as_ref() != Some(&stamp) {
                    seen_themes = Some(stamp);
                    sent = true;
                    if tx.send(Read::Themes(read_themes(dir))).is_err() {
                        return;
                    }
                }
            }
            if sent {
                ctx.request_repaint();
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

/// Change one key of `settings.toml`, on a thread of its own, and say what
/// the file now holds at once rather than at the next look.
fn edit_settings(tx: std::sync::mpsc::Sender<Read>, change: impl FnOnce(&str) -> Result<String, String> + Send + 'static) {
    // One change at a time, each on the file the last one wrote: several in
    // one frame (the environment's Save) no longer write over each other.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _ = std::thread::Builder::new().name("write-setting".into()).spawn(move || {
        let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let Some(path) = tsumugi_mux::settings::default_path() else { return };
        // A file that is there and does not read, or does not parse, is
        // left alone and said so; never taken as empty and written over.
        let written = files::read_or_empty(&path).and_then(|text| change(&text)).and_then(|next| files::write_atomic(&path, next));
        let read = match written {
            Ok(()) => tsumugi_mux::settings::load(&path),
            Err(e) => Err(e),
        };
        let _ = tx.send(Read::Settings(Box::new(read)));
    });
}

/// The settings with every `[[tags.rule]]` replaced by `rules`.
fn write_rules(text: &str, rules: &[tsumugi_mux::settings::TagRule]) -> Result<String, String> {
    use tsumugi_mux::settings::quote;
    let blocks: Vec<Vec<(&str, String)>> = rules
        .iter()
        .map(|r| {
            let mut b = Vec::new();
            if !r.folder.is_empty() {
                b.push(("folder", quote(&r.folder)));
            }
            if !r.branch.is_empty() {
                b.push(("branch", quote(&r.branch)));
            }
            b.push(("tag", quote(&r.tag)));
            b
        })
        .collect();
    tsumugi_mux::settings::set_tables(text, "tags.rule", &blocks)
}

/// How many folders of ended sessions are kept.
const CLOSED_KEPT: usize = 20;

/// The folders of sessions that ended, beside the state file.
fn closed_file() -> Option<std::path::PathBuf> {
    tsumugi_mux::state::default_path().map(|p| p.with_file_name("closed-folders"))
}

/// The input box's history, kept beside the state file: one prompt a line,
/// its backslashes and line breaks escaped.
fn history_file() -> Option<std::path::PathBuf> {
    tsumugi_mux::state::default_path().map(|p| p.with_file_name("history"))
}

fn load_history() -> Vec<String> {
    let text = history_file().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    text.lines().map(unescape_line).collect()
}

fn save_history(list: Vec<String>) {
    let _ = std::thread::Builder::new().name("history".into()).spawn(move || {
        let Some(p) = history_file() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text: String = list.iter().map(|l| format!("{}\n", escape_line(l))).collect();
        let _ = std::fs::write(p, text);
    });
}

fn escape_line(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "")
}

fn unescape_line(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match (c, c == '\\') {
            (_, true) => match chars.next() {
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            },
            (c, false) => out.push(c),
        }
    }
    out
}

/// Keep a new profile, on a thread of its own: replaces one of the same name.
fn save_profile(mut profiles: Vec<tsumugi_mux::settings::Profile>, new: tsumugi_mux::settings::Profile) {
    profiles.retain(|p| p.name != new.name);
    profiles.push(new);
    let _ = std::thread::Builder::new().name("profiles".into()).spawn(move || {
        if let Some(path) = tsumugi_mux::settings::profiles_path() {
            let _ = tsumugi_mux::settings::save_profiles(&path, &profiles);
        }
    });
}

/// Told of in the bell only: muted itself, or one of its tags is.
/// A prompt waiting for its session to finish (the input box's "When done").
struct Queued {
    id: SessionId,
    text: String,
}

/// How long something coming into view takes to fade in.
const FADE: std::time::Duration = std::time::Duration::from_millis(150);

/// How many lines a card's preview shows.
const PEEK_LINES: usize = 12;

/// Copy mode's key for a key pressed: the arrows and vi's letters.
fn copy_key(key: egui::Key, m: egui::Modifiers) -> Option<copymode::Key> {
    use copymode::Key as K;
    use egui::Key;
    if m.ctrl || m.alt || m.mac_cmd {
        return None;
    }
    Some(match key {
        Key::ArrowLeft | Key::H => K::Left,
        Key::ArrowRight | Key::L => K::Right,
        Key::ArrowUp | Key::K => K::Up,
        Key::ArrowDown | Key::J => K::Down,
        Key::PageUp => K::PageUp,
        Key::PageDown => K::PageDown,
        Key::G if m.shift => K::Bottom,
        Key::G => K::Top,
        Key::Home | Key::Num0 => K::LineStart,
        Key::End => K::LineEnd,
        Key::Num4 if m.shift => K::LineEnd,
        Key::V | Key::Space => K::Mark,
        Key::Y | Key::Enter => K::Copy,
        Key::Escape | Key::Q => K::Exit,
        _ => return None,
    })
}

/// The last `n` lines of a screen with anything on them, their trailing
/// blanks cut.
fn last_lines(rows: &[Vec<tsumugi_pane::CellView>], n: usize) -> Vec<String> {
    let mut lines: Vec<String> = rows.iter().map(|r| r.iter().map(|c| if c.c == '\0' { ' ' } else { c.c }).collect::<String>().trim_end().to_owned()).collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    let from = lines.len().saturating_sub(n);
    lines.split_off(from)
}

fn quiet(i: &Info, muted_tags: &[String]) -> bool {
    i.muted || i.tags.iter().any(|t| muted_tags.contains(t))
}

/// The window's own handle, for the taskbar's number (Windows).
fn window_handle(cc: &eframe::CreationContext<'_>) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        keys::physical_minus(&mut raw.events);
    }

    /// See-through where nothing is drawn when the desktop is to show
    /// through (Mica or Acrylic).
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if self.material || self.transparent { [0.0; 4] } else { visuals.panel_fill.to_normalized_gamma_f32() }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.animated = false;
        self.peek = None;
        self.watching.clear();
        self.frame(ui, frame);
        self.quake(ui.ctx());
        if !cfg!(target_os = "macos") {
            // The system's frame while there is no band to be one (the
            // screen saying the server cannot be reached), or when asked.
            let frame = !self.own_frame || self.client.is_none();
            if frame != self.system_frame {
                self.system_frame = frame;
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Decorations(frame));
            }
            // Last, so its pointer is the one shown over the edges.
            let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            if !frame && !maximized {
                chrome::resize_edges(ui.ctx());
            }
        }
        if self.animated {
            // About 30 frames a second while looked at (slow movement needs
            // no more); a few a second behind other windows.
            let seen = ui.ctx().input(|i| i.focused);
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(if seen { 33 } else { 100 }));
        }
    }
}

impl App {
    /// The branch mark is the Nerd Font's character: one is installed and
    /// the settings did not turn it off.
    fn nerd(&self) -> bool {
        self.nerd && self.settings_now.appearance.nerd_icons
    }

    /// The clock for something that moves, when the animations are on; and
    /// a note to draw the next frame soon.
    fn moving(&mut self, ui: &egui::Ui) -> Option<f64> {
        if !self.settings_now.appearance.animations {
            return None;
        }
        self.animated = true;
        Some(ui.input(|i| i.time))
    }

    fn frame(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // A menu open as the frame begins has the keys: an Esc closing it,
        // or what is typed in it, does not reach the shell.
        self.menu_open = egui::Popup::is_any_open(&ctx);
        let own = self.new_session.is_some() || self.search.is_some() || self.lists.is_some() || self.help.is_some() || self.diff.is_some() || self.parallel.is_some() || self.prefs.is_some() || self.menu_open || self.renaming_card.is_some() || self.noting_card.is_some() || self.find.as_ref().is_some_and(find::Bar::keyed) || self.paste_ask.is_some();
        keep_tab_for_pane(&ctx, own);
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
        self.faces_found = self.faces_next;
        if let Some(loaded) = self.fonts_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            ctx.set_fonts(loaded.defs);
            self.nerd = loaded.nerd;
            self.faces_next = loaded.faces;
            self.font_file = loaded.file;
            self.shaper = loaded.shaper;
            self.fonts_rx = None;
        }
        if let Some(Ok(f)) = self.facts_rx.as_ref().map(|rx| rx.try_recv()) {
            self.facts = Some(f);
            self.facts_rx = None;
        }
        let done: Vec<Result<String, String>> = self.jobs.1.try_iter().collect();
        for r in done {
            match r {
                Ok(words) => self.say(words, false),
                Err(e) => self.say(e, true),
            }
            if self.prefs.is_some() {
                self.read_facts();
            }
        }
        if let Some(Ok(answer)) = self.update.as_ref().map(|rx| rx.try_recv()) {
            self.update = None;
            if let Some(v) = answer {
                self.say(format!("tsumugi {v} is out (this is {}): github.com/uchmk/tsumugi/releases", env!("CARGO_PKG_VERSION")), false);
            }
        }
        let mut facts_again = false;
        let reads: Vec<Read> = self.settings.try_iter().collect();
        for read in reads {
            match read {
                Read::Settings(read) => match *read {
                    Ok(s) => {
                        if let Err(e) = keys::set_bindings(&s.keys) {
                            self.settings_error = Some(e);
                            continue;
                        }
                        keys::set_cmd_on_mac(s.general.cmd_on_mac);
                        if s.general.check_updates && !self.update_asked {
                            self.update_asked = true;
                            let wake = ctx.clone();
                            self.update = Some(update::check(move || wake.request_repaint()));
                        }
                        chrome::set_tag_colors(&s.tags.colors);
                        self.font = egui::FontId::monospace(font_size(s.font.size, self.font_step));
                        let own = s.window.own_titlebar();
                        if !cfg!(target_os = "macos") {
                            self.own_frame = own;
                        }
                        if s.font.family != self.font_family {
                            self.font_family = s.font.family.clone();
                            let (tx, rx) = std::sync::mpsc::channel();
                            let (family, wake) = (s.font.family.clone(), ctx.clone());
                            let _ = std::thread::Builder::new().name("fonts".into()).spawn(move || {
                                let _ = tx.send(fonts::load(&family));
                                wake.request_repaint();
                            });
                            self.fonts_rx = Some(rx);
                        }
                        let shell_changed = s.shell.program != self.settings_now.shell.program;
                        self.settings_now = s.clone();
                        price::set(&s.prices);
                        facts_again |= shell_changed && self.prefs.is_some();
                        self.alerts.rules = alert::Rules::from(&s.notify);
                        self.tag_rules = s.tags.rule;
                        self.theme_choice = (s.theme, s.dark_theme, s.light_theme);
                        self.open = s.open;
                        self.menu = s.menu;
                        self.settings_error = None;
                    }
                    Err(e) => self.settings_error = Some(e),
                },
                Read::Profiles(Ok(p)) => self.profiles = p,
                Read::Themes(Ok(t)) => {
                    self.theme_files = t;
                    self.theme_file_error = None;
                }
                Read::Themes(Err(e)) => self.theme_file_error = Some(e),
                Read::Profiles(Err(e)) => self.settings_error = Some(e),
            }
        }
        if facts_again {
            self.read_facts();
        }
        self.apply_theme(&ctx);
        if let Some(e) = self.settings_error.as_ref().or(self.theme_error.as_ref()) {
            // Above the status bar until the file is fixed; the server keeps
            // its tag rules from before too.
            egui::Area::new(egui::Id::new("settings-error")).anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -32.0)).show(&ctx, |ui| {
                egui::Frame::NONE.fill(crate::theme::mix(crate::theme::colors().panel, crate::theme::colors().err, 0.15)).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
                    ui.label(egui::RichText::new(e).size(12.0).color(chrome::red()));
                });
            });
        }
        let here = ctx.input(|i| i.viewport().focused).unwrap_or(true);
        if self.focus_sent != Some(here) {
            client.focus(here);
            self.focus_sent = Some(here);
        }
        let (anyone, teller) = client.attention();
        let muted_tags = client.muted_tags();
        let muted: std::collections::HashSet<SessionId> = sessions.iter().filter(|i| quiet(i, &muted_tags)).map(|i| i.id).collect();
        let places: std::collections::HashMap<SessionId, String> = sessions.iter().map(|i| (i.id, i.project.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default())).collect();
        let seen = alert::Seen { looking: here || anyone, teller, muted: &muted, places: &places };
        for id in self.teller.clicked() {
            // A notification clicked: its pane, in front.
            self.go_to(&client, &workspaces, id);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        let outs = self.alerts.decide(&client.notices(), seen);
        // Focus mode on: the number on the taskbar only (`[notify] focus_mode`).
        let hush = !outs.is_empty() && self.settings_now.notify.focus_mode && sound::quiet_time();
        for out in outs {
            match out {
                alert::Out::Badge(..) => self.teller.send(out),
                _ if hush => {}
                alert::Out::Flash => ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational)),
                alert::Out::Sound(error) => {
                    let n = &self.settings_now.notify;
                    sound::play(if error { &n.sound_error } else { &n.sound_waiting });
                }
                out => self.teller.send(out),
            }
        }
        // The estimated cost past its line: a toast here, and the system's
        // notification for a window not looked at.
        let n = &self.settings_now.notify;
        let (day, block) = (n.spend_day, n.spend_block);
        if day > 0 || block > 0 {
            for words in self.spend.check(&self.usage.get(), day, block) {
                if !here {
                    self.teller.send(alert::Out::Notify { session: 0, title: "tsumugi".into(), body: words.clone() });
                }
                self.say(words, false);
            }
        }
        let current = self.current(&workspaces);
        if std::mem::take(&mut self.jump_waiting) {
            if let Some(w) = &current {
                let area = to_rect(ctx.content_rect());
                self.act(keys::Action::NextWaiting, w, &workspaces, area);
            }
        }

        if let Some(w) = &current {
            let t = sessions.iter().find(|i| i.id == w.focus).map(|i| sort::display_title(&i.title, &i.command)).unwrap_or_default();
            let mut title = if t.is_empty() { "tsumugi".to_owned() } else { format!("{t} — tsumugi") };
            // Where the taskbar cannot carry the number, the title does.
            let n = self.alerts.badge();
            if !alert::TASKBAR_NUMBER && n > 0 {
                title = format!("({n}) {title}");
            }
            if title != self.title {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
                self.title = title;
            }
            let mut actions = Vec::new();
            // A field of the window's own (a menu's "Add a tag") has the
            // keys while it is focused; the pane gets them otherwise.
            // A key being changed in the settings is the settings'.
            let capturing = self.prefs.as_ref().is_some_and(|p| p.edit.capturing.is_some());
            let field = ctx.memory(|m| m.focused().is_some()) || self.new_session.is_some() || self.search.is_some() || self.lists.is_some() || self.help.is_some() || self.diff.is_some() || self.parallel.is_some() || self.menu_open || self.input.had_keys(&ctx) || capturing || self.renaming_card.is_some() || self.noting_card.is_some() || self.find.as_ref().is_some_and(find::Bar::keyed) || self.paste_ask.is_some();
            if self.key_log {
                // `TSUMUGI_KEYLOG=1`: every key press as the window gets it,
                // to see on a real machine why a key does nothing.
                for ev in ctx.input(|i| i.events.clone()) {
                    if let egui::Event::Key { key, pressed: true, modifiers, .. } = ev {
                        let held = ctx.memory(|m| m.focused());
                        eprintln!("key {key:?} {modifiers:?} -> {:?}; a widget has the keys: {held:?}", keys::action(key, modifiers));
                    }
                }
            }
            // The settings screen has the keys while it is open: none reach
            // the shell behind it, and of the window's own only its own key,
            // which closes it again.
            // Whether the settings were open as this frame began: their key
            // closes them, and must not then reach the pane as the key that
            // opens them again (the source review, 2026-10-07).
            let prefs_were_open = self.prefs.is_some();
            if self.prefs.is_some() && !capturing {
                let closing = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. } if keys::action(*key, *modifiers) == Some(keys::Action::Settings))));
                if closing {
                    self.prefs = None;
                }
            }
            // Copy mode and the find bar end when the keys go to another pane.
            if self.copy_mode.is_some_and(|(id, _)| id != w.focus) {
                self.copy_mode = None;
            }
            if self.find.as_ref().is_some_and(|b| b.id != w.focus) {
                self.find = None;
            }
            let copying = self.copy_mode.is_some();
            if let Some(pane) = self.panes.get(&w.focus).filter(|_| copying && !field && !prefs_were_open) {
                let events = ctx.input(|i| i.events.clone());
                let screen = tsumugi_pane::Pane::screen(pane);
                let (cols, rows) = (screen.rows.first().map_or(0, Vec::len), screen.rows.len());
                for e in &events {
                    let egui::Event::Key { key, pressed: true, modifiers, .. } = e else { continue };
                    if let Some(a) = keys::action(*key, *modifiers) {
                        actions.push(a);
                        continue;
                    }
                    let Some(k) = copy_key(*key, *modifiers) else { continue };
                    let Some((_, mode)) = self.copy_mode.as_mut() else { break };
                    for step in mode.press(k, cols, rows) {
                        use tsumugi_pane::alacritty_terminal::grid::Scroll;
                        match step {
                            copymode::Step::Lines(n) => tsumugi_pane::Pane::scroll(pane, Scroll::Delta(n)),
                            copymode::Step::PageUp => tsumugi_pane::Pane::scroll(pane, Scroll::PageUp),
                            copymode::Step::PageDown => tsumugi_pane::Pane::scroll(pane, Scroll::PageDown),
                            copymode::Step::Top => tsumugi_pane::Pane::scroll(pane, Scroll::Top),
                            copymode::Step::Bottom => tsumugi_pane::Pane::scroll(pane, Scroll::Bottom),
                            copymode::Step::Select { cell, start } => tsumugi_pane::Pane::select(pane, cell, true, start),
                            copymode::Step::Unselect => tsumugi_pane::Pane::clear_selection(pane),
                            // A server's pane answers through take_clipboard.
                            copymode::Step::Copy => {
                                if let Some(text) = tsumugi_pane::Pane::selection(pane) {
                                    ctx.copy_text(text);
                                }
                            }
                            copymode::Step::Exit => self.copy_mode = None,
                        }
                    }
                }
            } else if let Some(pane) = self.panes.get(&w.focus).filter(|_| !field && !prefs_were_open) {
                let mut events = ctx.input(|i| i.events.clone());
                // A paste that could run more than was meant waits for an
                // answer instead (`paste`); the rest of the frame goes on.
                let to: Vec<SessionId> = if self.typing_all.contains(&w.id) { w.layout.leaves() } else { vec![w.focus] };
                let bracketed = to.iter().filter_map(|id| self.panes.get(id)).all(tsumugi_pane::Pane::bracketed_paste);
                let general = &self.settings_now.general;
                let mut held = None;
                events.retain(|e| match e {
                    egui::Event::Paste(text) => match paste::why(text, bracketed, general) {
                        Some(why) => {
                            held = Some(paste::Held { to: to.clone(), text: text.clone(), why });
                            false
                        }
                        None => true,
                    },
                    _ => true,
                });
                if held.is_some() {
                    self.paste_ask = held;
                }
                tsumugi_pane::input::feed(pane, &events, |key, m| match keys::action(key, m) {
                    Some(a) => {
                        actions.push(a);
                        true
                    }
                    None => false,
                });
                // Typing into every pane: the same keys to the tab's others
                // (the window's own keys stay the window's, done once).
                if self.typing_all.contains(&w.id) {
                    for other in w.layout.leaves().into_iter().filter(|o| *o != w.focus) {
                        if let Some(p) = self.panes.get(&other) {
                            tsumugi_pane::input::feed(p, &events, |key, m| keys::action(key, m).is_some());
                        }
                    }
                }
            }
            let area = to_rect(ctx.content_rect());
            for a in actions {
                self.act(a, w, &workspaces, area);
            }
        }

        // The status bar along the bottom (the design's 1d).
        let focus_info = current.as_ref().and_then(|w| sessions.iter().find(|i| i.id == w.focus)).cloned();
        let size = current.as_ref().and_then(|w| self.panes.get(&w.focus)).map(|p| {
            let rows = tsumugi_pane::Pane::screen(p).rows;
            (rows.first().map_or(0, Vec::len), rows.len())
        });
        let up = chrome::now_ms().saturating_sub(client.started_ms());
        // Not under the settings, which are a screen of their own (the design).
        let status = if self.prefs.is_some() {
            None
        } else {
            egui::Panel::bottom("status")
            .exact_size(28.0)
            .frame(egui::Frame::NONE.fill(self.chrome_fill(crate::theme::colors().side)))
            .show(ui, |ui| {
                let clock = &self.settings_now.clock;
                let git = focus_info.as_ref().and_then(|i| self.git.get(&i.cwd, &i.branch));
                let used = self.usage.get();
                let conversation = focus_info.as_ref().filter(|i| !i.conversation.is_empty()).and_then(|i| used.conversations.get(&i.conversation).copied());
                let tokens = (used.today.total() > 0 || conversation.is_some()).then_some((conversation, used.today));
                let cost = (focus_info.as_ref().and_then(|i| used.costs.get(&i.conversation).copied()), used.today_cost);
                let extra = chrome::StatusExtra { up_ms: up, nerd: self.nerd(), clock: clock.show.then(|| clock.format()), git, tokens, block: used.block, cost };
                chrome::status_bar(ui, &self.palette, &sessions, focus_info.as_ref(), size, &extra)
            })
            .inner
        };
        match status {
            Some(chrome::StatusClick::Bell) => {
                self.bell_open = Some(egui::pos2(20.0, 60.0));
                self.bell_opening = true;
            }
            Some(chrome::StatusClick::Open(url)) => {
                menu::open_url(&url);
            }
            Some(chrome::StatusClick::Charset(name)) => {
                if let Some(i) = &focus_info {
                    client.set_charset(i.id, name);
                }
            }
            None => {}
        }
        // The clock and the elapsed times move on without any output.
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        // On macOS with tsumugi's own title bar, the sidebar runs up to the
        // window's top and the traffic lights sit on it (the design's Mac
        // window); in full screen they are gone, and so is the room.
        let lights_on_side = cfg!(target_os = "macos") && self.own_frame && self.prefs.is_none();
        if lights_on_side {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            self.side_panel(ui, &workspaces, &sessions, if full { 0.0 } else { 30.0 });
        }
        // The band along the top (the design's 1c): the name, the search
        // box, and the tags of the session with the keys.
        let focus_tags = focus_info.as_ref().map(|i| i.tags.clone()).unwrap_or_default();
        let muted_tags_now = client.muted_tags();
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let band_frame = chrome::BandFrame { own: self.own_frame && !self.system_frame, left: if self.own_frame && cfg!(target_os = "macos") && !lights_on_side { 76.0 } else { 0.0 }, maximized, settings: self.prefs.is_some() };
        let band = egui::Panel::top("band")
            .exact_size(40.0)
            .frame(egui::Frame::NONE.fill(self.chrome_fill(crate::theme::colors().side)))
            .show(ui, |ui| chrome::top_band(ui, &self.palette, &focus_tags, self.settings_now.tags.shown(), &muted_tags_now, &client.notices(), &band_frame))
            .inner;
        if let Some(at) = band.bell_anchor {
            self.bell_anchor = egui::pos2(at.x.max(8.0), at.y);
        }
        if band.bell {
            self.bell_open = match self.bell_open {
                Some(_) => None,
                None => Some(self.bell_anchor),
            };
            self.bell_opening = true;
        }
        if band.search {
            self.search = Some(palette::View::new());
        }
        if band.close_settings {
            self.prefs = None;
        }
        match band.window {
            Some(chrome::WindowOp::Drag) => ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag),
            Some(chrome::WindowOp::ToggleMax) => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized)),
            Some(chrome::WindowOp::Minimize) => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            Some(chrome::WindowOp::Close) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            None => {}
        }


        if !lights_on_side {
            self.side_panel(ui, &workspaces, &sessions, 0.0);
        }

        if let Some(at) = self.bell_open {
            let notices = client.notices();
            match chrome::bell_list(&ctx, &self.palette, at, &notices) {
                Some(chrome::BellAction::Open(session, id)) => {
                    client.read_notices(Some(vec![id]));
                    self.go_to(&client, &workspaces, session);
                    self.bell_open = None;
                }
                Some(chrome::BellAction::ReadAll) => client.read_notices(None),
                Some(chrome::BellAction::Close) if !self.bell_opening => self.bell_open = None,
                _ => {}
            }
        }

        let made = match &mut self.new_session {
            Some(d) => {
                if let Ok(places) = self.places.lock() {
                    d.places.clone_from(&places);
                }
                newsession::show(&ctx, &self.palette, d, &Self::recents(&sessions, current.as_ref(), &self.closed), &self.profiles, &self.tag_rules)
            }
            None => None,
        };
        match made {
            Some(newsession::Answer::Create(c)) => {
                self.new_session = None;
                self.create(&client, c, current.as_ref());
            }
            Some(newsession::Answer::Cancel) => self.new_session = None,
            None => {}
        }
        self.worktrees(&ctx, &client, &sessions, current.as_ref());
        self.close_window(&ctx, &client, &sessions);
        if let Some(held) = &self.paste_ask {
            if let Some(yes) = paste::show(&ctx, held, &theme::colors()) {
                let held = self.paste_ask.take().expect("shown");
                if yes {
                    for p in held.to.iter().filter_map(|id| self.panes.get(id)) {
                        tsumugi_pane::Pane::paste(p, &held.text);
                    }
                }
            }
        }
        self.remember_closed(&sessions);
        self.hooks_offer(&ctx);
        // Set again by the first-run screen, drawn after this.
        self.first_shown = false;
        self.send_queued(&client, &sessions);
        self.show_toast(&ctx);

        let answer = match &mut self.search {
            Some(view) => {
                // The scrollbacks are asked once typing pauses, for three
                // letters or more; their answer comes a frame or two later.
                let q = view.query.trim().to_owned();
                if q.chars().count() >= 3 && q != view.asked && view.changed.elapsed() > std::time::Duration::from_millis(250) {
                    client.search_all(q.clone());
                    view.asked = q.clone();
                } else if q != view.asked {
                    ctx.request_repaint_after(std::time::Duration::from_millis(260));
                }
                let lines: Vec<palette::Entry> = match client.found_all() {
                    Some((asked, hits)) if asked == q && q.chars().count() >= 3 => hits
                        .into_iter()
                        .map(|h| {
                            let name = sessions.iter().find(|i| i.id == h.id).map(|i| sort::display_title(&i.title, &i.command)).unwrap_or_default();
                            let len = q.chars().count();
                            palette::Entry { title: h.text.trim().to_owned(), detail: name, pick: palette::Pick::Line { id: h.id, line: h.line, col: h.col, len } }
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                chrome::search_box(&ctx, &self.palette, view, &Self::search_entries(&workspaces, &sessions, &self.input.prompts, &self.layouts), &lines)
            }
            None => None,
        };
        match answer {
            Some(palette::Answer::Pick(p)) => {
                self.search = None;
                self.picked(p, current.as_ref(), &workspaces, to_rect(ctx.content_rect()));
            }
            Some(palette::Answer::Close) => self.search = None,
            None => {}
        }

        self.waiting_and_closed(&ctx, &client, &workspaces, &sessions);
        if let Some(mut view) = self.help.take() {
            if help::show(&ctx, &mut view, &theme::colors()) {
                self.help = Some(view);
            }
        }
        if let Some(view) = &mut self.diff {
            if !diffview::show(&ctx, view, &theme::colors()) {
                self.diff = None;
            }
        }
        self.start_parallel(&ctx, &client);

        self.bell_opening = false;
        // `TSUMUGI_KEYLOG=1` also says where the keys went when that changes
        // (the control's rectangle, in points), so a check of the Tab order
        // reads it from stderr instead of a picture.
        if self.key_log {
            let now = ctx.memory(|m| m.focused());
            if now != self.focus_logged {
                self.focus_logged = now;
                match now.and_then(|id| ctx.read_response(id)) {
                    Some(r) => eprintln!("focus {:.0},{:.0} {:.0}x{:.0}", r.rect.left(), r.rect.top(), r.rect.width(), r.rect.height()),
                    None => eprintln!("focus none"),
                }
            }
        }
        // Files dropped on the window go with the next prompt.
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if let (false, Some(w)) = (dropped.is_empty(), &current) {
            self.input.drop_files(w.focus, dropped.clone());
            // Their sizes for the chips, read on a thread.
            let (tx, ctx, to) = (self.attach.0.clone(), ctx.clone(), w.focus);
            let _ = std::thread::Builder::new().name("file-sizes".into()).spawn(move || {
                let _ = tx.send((to, clip::sizes(dropped)));
                ctx.request_repaint();
            });
        }
        // See-through: the panes draw the window's colour round themselves
        // (backdrop::around), so it is not twice under them.
        let main_fill = if self.transparent { egui::Color32::TRANSPARENT } else { self.palette.on_cursor };
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(main_fill)).show(ui, |ui| {
            if self.transparent && (self.failed.is_some() || self.restore.is_some() || self.prefs.is_some() || current.is_none()) {
                ui.painter().rect_filled(ui.max_rect(), 0.0, self.palette.on_cursor.gamma_multiply(self.alpha()));
            }
            if self.failed.is_some() {
                self.message(ui);
                return;
            }
            if let Some(view) = &mut self.restore {
                match chrome::restore_screen(ui, &self.palette, view) {
                    Some(chrome::RestoreAnswer::Restore(ids)) => {
                        set_always_restore(view.always);
                        if client.restore_only(Some(ids)).unwrap_or(0) == 0 {
                            let _ = start_here(&client);
                        }
                        self.restore = None;
                    }
                    Some(chrome::RestoreAnswer::Fresh) => {
                        set_always_restore(view.always);
                        if let Err(e) = start_here(&client) {
                            self.failed = Some(e);
                        }
                        self.restore = None;
                    }
                    None => {}
                }
                return;
            }
            if self.prefs.is_some() {
                self.settings_screen(ui, &client, &workspaces, &sessions);
                return;
            }
            if let Some(w) = &current {
                self.panes(ui, w, &sessions);
            } else if workspaces.is_empty() && self.pending.is_none() {
                self.first_run(ui, &client);
            }
        });
        // Prompts kept or forgotten in the input box.
        if std::mem::take(&mut self.input.prompts_changed) {
            prompts::save(self.input.prompts.clone());
        }
        // What the input box in the pane sent (the design's 12).
        if let Some(send) = self.input_sent.take() {
            let targets: Vec<SessionId> = match &send.to {
                inputbox::To::Session(id) => vec![*id],
                inputbox::To::Sessions(ids) => ids.clone(),
                inputbox::To::Tags(want) => sessions.iter().filter(|i| i.tags.iter().any(|t| want.contains(t))).map(|i| i.id).collect(),
            };
            for id in targets {
                // `{folder}`, `{project}`, `{branch}`: each session's own.
                let text = match sessions.iter().find(|i| i.id == id) {
                    Some(i) => prompts::fill(&send.text, &i.cwd, &i.project, &i.branch),
                    None => send.text.clone(),
                };
                if send.later {
                    self.queue.push(Queued { id, text });
                } else {
                    client.send_prompt(id, text);
                }
            }
            save_history(self.input.history.clone());
        }
        // A screenshot pasted into it: read on a thread, a chip when it is
        // a file.
        if self.input.take_image_request() {
            if let (Some(w), Some(dir)) = (&current, clip::folder()) {
                let (tx, ctx, to) = (self.attach.0.clone(), ctx.clone(), w.focus);
                let jobs = self.jobs.0.clone();
                let _ = std::thread::Builder::new().name("paste-image".into()).spawn(move || {
                    match clip::paste_image(&dir) {
                        Ok(Some(file)) => {
                            let _ = tx.send((to, vec![file]));
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = jobs.send(Err(e));
                        }
                    }
                    ctx.request_repaint();
                });
            }
        }
        let arrived: Vec<Attach> = self.attach.1.try_iter().collect();
        for (to, files) in arrived {
            self.input.sized_files(to, files);
        }
    }
}

impl App {
    /// Sessions that ended go on the recently closed list; the waiting list
    /// or that list is drawn when open, and what it asks is done.
    fn waiting_and_closed(&mut self, ctx: &egui::Context, client: &Client, workspaces: &[Workspace], sessions: &[Info]) {
        // A whole buffer asked for: written to Downloads on a thread.
        for (id, text) in client.take_texts() {
            let Some(name) = self.saving.remove(&id) else { continue };
            self.save_output(name, text);
        }
        // Typing into every pane ends with the split.
        self.typing_all.retain(|id| workspaces.iter().any(|w| w.id == *id && w.layout.leaves().len() > 1));
        let ended = client.take_ended();
        if !ended.is_empty() {
            let used = self.usage.get();
            for (info, last) in ended {
                let tokens = used.conversations.get(&info.conversation).map_or(0, usage::Tokens::total);
                let cost = used.costs.get(&info.conversation).copied().unwrap_or(0.0);
                let c = history::Closed {
                    title: info.title,
                    command: info.command,
                    cwd: info.cwd,
                    claude: info.claude,
                    conversation: info.conversation,
                    // How it ended, where that says something: a plain
                    // shell "running" says nothing.
                    state: match info.state {
                        State::Error => "error".to_owned(),
                        State::Done if info.claude => "done".to_owned(),
                        _ => String::new(),
                    },
                    tokens,
                    cost,
                    ended_ms: chrome::now_ms(),
                    last,
                };
                history::push(&mut self.ended, c);
            }
            history::save(self.ended.clone());
        }
        let Some(mut view) = self.lists.take() else { return };
        let mut waiting: Vec<&Info> = sessions.iter().filter(|i| i.state == State::Waiting).collect();
        waiting.sort_by_key(|i| i.since_ms);
        let now = chrome::now_ms();
        let mut rows = Vec::new();
        for i in waiting {
            // Its menu, from its screen (attached for as long as it is shown).
            let lines = self.watch_lines(i.id);
            let choices = answer::choices(&lines);
            let asking = answer::asking(&lines);
            let rule = permit::rule(&asking);
            rows.push(lists::Row {
                id: i.id,
                name: sort::display_title(&i.title, &i.command),
                folder: i.project.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
                note: i.note.clone(),
                waited: chrome::elapsed(now.saturating_sub(i.since_ms)),
                choices,
                asking,
                rule,
                rule_file: permit::file(&i.project),
            });
        }
        // Every session, only while its page shows: each is attached to read
        // its last lines.
        let mut all = Vec::new();
        if view.page == lists::Page::All {
            let used = self.usage.get();
            let mut ordered: Vec<&Info> = sessions.iter().collect();
            ordered.sort_by_key(|i| lists::rank(i.state, i.since_ms));
            for i in ordered {
                let mut lines = self.watch_lines(i.id);
                lines.retain(|l| !l.trim().is_empty());
                let tokens = used.conversations.get(&i.conversation).map_or(0, usage::Tokens::total);
                let mut folder = home_short(&i.cwd);
                if !i.branch.is_empty() {
                    folder = format!("{folder} · {}", i.branch);
                }
                all.push(lists::Card {
                    id: i.id,
                    name: sort::display_title(&i.title, &i.command),
                    state: i.state,
                    words: chrome::state_words(i, now),
                    folder,
                    tags: i.tags.clone(),
                    tokens: if tokens > 0 { format!("{} tokens", usage::short(tokens)) } else { String::new() },
                    last: lines.split_off(lines.len().saturating_sub(2)),
                });
            }
        }
        let colors = theme::colors();
        let mut keep = true;
        for d in lists::show(ctx, &mut view, &all, sessions.len(), &rows, &self.ended, &colors) {
            match d {
                lists::Do::Type(id, key) => {
                    if let Some(pane) = self.panes.get(&id) {
                        tsumugi_pane::Pane::send(pane, vec![key as u8]);
                    }
                }
                lists::Do::Allow(id, rule) => {
                    let Some(info) = sessions.iter().find(|i| i.id == id) else { continue };
                    let yes = rows.iter().find(|r| r.id == id).and_then(|r| answer::yes(&r.choices));
                    let project = info.project.clone();
                    let (tx, ctx2) = (self.jobs.0.clone(), ctx.clone());
                    let _ = std::thread::Builder::new().name("permit".into()).spawn(move || {
                        let _ = tx.send(permit::allow(&project, &rule).map(|p| format!("Claude Code runs {rule} without asking now ({})", p.display())));
                        ctx2.request_repaint();
                    });
                    if let (Some(key), Some(pane)) = (yes, self.panes.get(&id)) {
                        tsumugi_pane::Pane::send(pane, vec![key as u8]);
                    }
                }
                lists::Do::Go(id) => {
                    self.go_to(client, workspaces, id);
                    keep = false;
                }
                lists::Do::StartAgain(k) => {
                    let Some(c) = self.ended.get(k) else { continue };
                    let claude = &self.settings_now.sessions.claude;
                    let typed = if c.claude && c.conversation.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') && !c.conversation.is_empty() {
                        newsession::Start::Claude.typed(claude).map(|cmd| format!("{cmd} --resume {}", c.conversation))
                    } else if c.claude {
                        newsession::Start::Claude.typed(claude)
                    } else {
                        None
                    };
                    match client.spawn_typing(c.cwd.clone(), None, Size::new(80, 24), (8, 16), Place::NewWorkspace, typed) {
                        Ok(pane) => self.pending = Some(pane.id()),
                        Err(e) => self.failed = Some(e.to_string()),
                    }
                    keep = false;
                }
                lists::Do::Copy(text) => ctx.copy_text(text),
                lists::Do::Save(k) => {
                    if let Some(c) = self.ended.get(k) {
                        let mut text = c.last.join("\n");
                        text.push('\n');
                        self.save_output(sort::display_title(&c.title, &c.command), text);
                    }
                }
                lists::Do::Forget(k) => {
                    if k < self.ended.len() {
                        self.ended.remove(k);
                        history::save(self.ended.clone());
                    }
                }
                lists::Do::ForgetAll => {
                    self.ended.clear();
                    history::save(Vec::new());
                }
                lists::Do::Close => keep = false,
            }
        }
        if keep {
            self.lists = Some(view);
        }
    }

    fn message(&mut self, ui: &mut egui::Ui) {
        let Some(why) = self.failed.clone() else { return };
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, theme::colors().bg);
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(40.0)).layout(egui::Layout::top_down(egui::Align::Center)));
        let Some(detail) = why.strip_prefix(OTHER_VERSION) else {
            inner.add_space((rect.height() / 2.0 - 80.0).max(0.0));
            inner.label(egui::RichText::new(&why).font(self.font.clone()).color(self.palette.fg));
            self.replaced();
            if self.replacing.is_some() {
                inner.ctx().request_repaint_after(Duration::from_millis(200));
            }
            return;
        };
        // A server of another version (an update, a new build): what that
        // means, and Enter to stop it -- its tabs written down -- and start
        // this version's, which offers them back.
        let c = theme::colors();
        inner.add_space((rect.height() / 2.0 - 140.0).max(0.0));
        inner.label(egui::RichText::new("tsumugi was updated").size(20.0).strong().color(c.strong()));
        inner.add_space(10.0);
        inner.scope(|ui| {
            ui.set_max_width(560.0);
            let words = "Your sessions are still running in the server of the version you had before, which this version cannot talk to. \
                         Restart the server to open them here: their shells stop, and the tabs come back in their folders, \
                         Claude Code resuming its conversations.";
            ui.label(egui::RichText::new(words).size(13.5).color(c.dim));
        });
        inner.add_space(18.0);
        // Told not to ask (Settings, General): read once, tried once.
        let unasked = !std::mem::replace(&mut self.replaced_unasked, true) && restart_after_update();
        if self.replacing.is_some() {
            inner.ctx().request_repaint_after(Duration::from_millis(200));
            inner.label(egui::RichText::new("Restarting the server…").size(13.5).color(c.dim));
        } else if unasked {
            let ctx = inner.ctx().clone();
            self.replacing = Some(replace_server(move || ctx.request_repaint()));
        } else {
            let (enter, esc) = inner.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
            let restart = inner.add(egui::Button::new(egui::RichText::new("Restart the server   Enter").color(c.on_accent()).strong()).fill(c.run).min_size(egui::vec2(220.0, 32.0)));
            inner.add_space(6.0);
            let later = inner.add(egui::Button::new(egui::RichText::new("Not now   Esc").size(12.5)).frame(false));
            if restart.clicked() || enter {
                let ctx = inner.ctx().clone();
                self.replacing = Some(replace_server(move || ctx.request_repaint()));
            } else if later.on_hover_text("Close this window; the sessions go on running").clicked() || esc {
                inner.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        inner.add_space(24.0);
        inner.label(egui::RichText::new(detail).size(11.0).color(c.faint()));
        self.replaced();
    }

    /// The server replaced (another version's, or a restart): this window
    /// takes the new one.
    fn replaced(&mut self) {
        let Some(answer) = self.replacing.as_ref().and_then(|rx| rx.try_recv().ok()) else { return };
        self.replacing = None;
        match answer.and_then(|client| first_session(&client).map(|restore| (client, restore))) {
            Ok((client, restore)) => {
                // The restart was asked for here (an update, or Restart the
                // server): every tab comes back at once, with no Welcome
                // back to answer again.
                self.restore = if restore.is_some() && client.restore().unwrap_or(0) > 0 { None } else { restore };
                self.client = Some(client);
                self.failed = None;
            }
            Err(e) => self.failed = Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{escape_line, json_string, keep_tab_for_pane, unescape_line, View};

    #[test]
    fn codexs_notice_is_read() {
        let e = r#"{"type":"agent-turn-complete","turn-id":"1","input-messages":["fix it"],"last-assistant-message":"\n  Fixed the bug in main.rs.\nAnd more."}"#;
        assert_eq!(super::codex_event(e), Some((true, "Fixed the bug in main.rs.".to_string())));
        assert_eq!(super::codex_event(r#"{"type":"other"}"#), Some((false, String::new())));
        assert_eq!(super::codex_event("{not json"), None);
    }

    #[test]
    fn a_prompt_of_several_lines_is_one_line_of_the_history() {
        let p = "first\nsecond with a \\n written\\";
        let line = escape_line(p);
        assert!(!line.contains('\n'));
        assert_eq!(unescape_line(&line), p);
    }

    #[test]
    fn the_sidebars_choices_come_back() {
        let v = View { sort: crate::sort::Sort::Needs, density: crate::sort::Density::Lines, rail: true, asked: true, hooks_asked: true };
        assert_eq!(View::parse(&v.text()), v);
        assert_eq!(View::parse("nonsense\nsort=nope"), View::default());
    }

    /// Tab with nothing focused stays the pane's: no button of the window
    /// takes the keys away from the shell (lazygit, 2026-10-08).
    #[test]
    fn tab_stays_with_the_pane() {
        let ctx = egui::Context::default();
        let tab = |shift: bool| {
            let modifiers = if shift { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE };
            vec![egui::Event::Key { key: egui::Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers }]
        };
        let run = |events: Vec<egui::Event>, own: bool| {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                keep_tab_for_pane(ui.ctx(), own);
                let _ = ui.button("one");
                let _ = ui.button("two");
            });
            out.textures_delta.clear();
            ctx.memory(|m| m.focused())
        };
        run(Vec::new(), false);
        assert_eq!(run(tab(false), false), None);
        assert_eq!(run(tab(true), false), None);
        // A screen of the window's own keeps Tab moving between its controls.
        assert!(run(tab(false), true).is_some());
    }

    /// The frames of a field kept focused, and whether each ended an IME
    /// composition: Japanese is typed over several frames, so only the
    /// first may.
    fn interruptions(keep: fn(&egui::Response)) -> Vec<bool> {
        let ctx = egui::Context::default();
        let mut text = String::new();
        (0..4)
            .map(|_| {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| keep(&ui.add(egui::TextEdit::singleline(&mut text).id(egui::Id::new("note")))));
                out.textures_delta.clear();
                out.platform_output.ime.is_some_and(|ime| ime.should_interrupt_composition)
            })
            .collect()
    }

    #[test]
    fn a_kept_field_lets_a_composition_run() {
        assert!(!interruptions(super::keep_focus)[1..].contains(&true), "the note field ends Japanese being typed");
        // What it was before: asked again every frame, every frame ended it.
        assert!(interruptions(|r| r.request_focus())[1..].iter().all(|&i| i));
    }

    #[test]
    fn a_hooks_message_is_read() {
        let event = r#"{"session_id":"x","hook_event_name":"Notification","message":"Claude needs your \"OK\"\u3002"}"#;
        assert_eq!(json_string(event, "message").as_deref(), Some("Claude needs your \"OK\"\u{3002}"));
        assert_eq!(json_string(event, "missing"), None);
    }
}
