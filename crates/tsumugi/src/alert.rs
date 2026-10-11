//! Telling someone who is not looking at the window (the design's 1h): the
//! system's notification, a number on the taskbar button, and flashing it.
//!
//! What to do is decided here from the server's notices, the same list the
//! bell shows; doing it is the system's part (`os`), on a thread of its own
//! so a slow notification service never holds up a frame.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, mpsc};

use tsumugi_mux::{Notice, SessionId, State};

/// Which ways to tell, per state: `[notify]` in `config.toml`.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    /// The system's notification.
    pub notify: Kinds,
    /// Counted in the number on the taskbar button.
    pub badge: Kinds,
    /// Flash the taskbar button.
    pub flash: Kinds,
    /// Play the system's sound.
    pub sound: Kinds,
}

#[derive(Clone, Copy, Debug)]
pub struct Kinds {
    pub waiting: bool,
    pub error: bool,
    /// Only the long runs make a notice at all (the server's `LONG_RUN`).
    pub done: bool,
}

impl Kinds {
    fn has(self, state: State) -> bool {
        match state {
            State::Waiting | State::MaybeWaiting => self.waiting,
            State::Error => self.error,
            State::Done => self.done,
            State::Running => false,
        }
    }
}

impl Kinds {
    /// From the settings' words (checked there): `waiting`, `error`, `done`.
    fn from_words(words: &[String]) -> Self {
        let has = |w: &str| words.iter().any(|x| x == w);
        Self { waiting: has("waiting"), error: has("error"), done: has("done") }
    }
}

impl From<&tsumugi_mux::settings::Notify> for Rules {
    fn from(n: &tsumugi_mux::settings::Notify) -> Self {
        Self { notify: Kinds::from_words(&n.system), badge: Kinds::from_words(&n.taskbar), flash: Kinds::from_words(&n.flash), sound: Kinds::from_words(&n.sound) }
    }
}

impl Default for Rules {
    /// The settings' defaults (the design's 1h).
    fn default() -> Self {
        Self::from(&tsumugi_mux::settings::Notify::default())
    }
}

/// One thing to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    /// `session`: where a click on it goes. `state`: what the session is
    /// in, for a notification that looks like it (None: a plain one).
    Notify { session: SessionId, title: String, body: String, state: Option<State> },
    Flash,
    /// The sound for a session that failed (`true`) or wants a person.
    Sound(bool),
    /// How many, and whether one of them failed (the number is red then,
    /// else gold, as the design has it).
    Badge(usize, bool),
}

/// What has been told so far.
#[derive(Default)]
pub struct Alerts {
    pub rules: Rules,
    /// Notices below this id have been seen; `None` before the first list,
    /// whose notices are from before this window and are not told again.
    next: Option<u64>,
    /// The number last put on the taskbar, and whether it was red.
    badge: (usize, bool),
}

/// Who is where, for [`Alerts::decide`].
pub struct Seen<'a> {
    /// Someone is at a window of this server: nothing needs telling.
    pub looking: bool,
    /// This window is the one to tell (the one that had the keyboard last),
    /// so two windows do not tell twice. The number is on every window's
    /// taskbar button all the same.
    pub teller: bool,
    /// Sessions told of in the bell only.
    pub muted: &'a HashSet<SessionId>,
    /// Each session's folder by name (`filer`), for the notification's
    /// title; a session missing here is called by its title.
    pub places: &'a HashMap<SessionId, String>,
    /// What each running session has used since it began running (`12k
    /// tokens · $0.35`), put after a finished run's words.
    pub spent: &'a HashMap<SessionId, String>,
}

impl Alerts {
    /// What the notices that arrived since the last call ask for.
    pub fn decide(&mut self, notices: &[Notice], seen: Seen) -> Vec<Out> {
        let Seen { looking, teller, muted, places, spent } = seen;
        let heard = |n: &&Notice| !muted.contains(&n.session);
        let mut out = Vec::new();
        let next = notices.iter().map(|n| n.id + 1).max().unwrap_or(0);
        let fresh: Vec<&Notice> = match self.next {
            Some(from) => notices.iter().filter(|n| n.id >= from && !n.read).filter(heard).collect(),
            None => Vec::new(),
        };
        self.next = Some(next.max(self.next.unwrap_or(0)));
        if !looking && teller {
            // A trigger with `notify` asked to be told; the rest by its state.
            for n in fresh.iter().filter(|n| n.trigger || self.rules.notify.has(n.state)) {
                let (title, mut body) = wording(n, places.get(&n.session).map(String::as_str));
                if let Some(used) = spent.get(&n.session).filter(|_| n.state == State::Done && !n.trigger) {
                    if !body.is_empty() {
                        body.push_str(" · ");
                    }
                    body.push_str(used);
                }
                out.push(Out::Notify { session: n.session, title, body, state: (!n.trigger).then_some(n.state) });
            }
            let fresh: Vec<&Notice> = fresh.into_iter().filter(|n| !n.trigger).collect();
            if fresh.iter().any(|n| self.rules.flash.has(n.state)) {
                out.push(Out::Flash);
            }
            let sounding: Vec<&&Notice> = fresh.iter().filter(|n| self.rules.sound.has(n.state)).collect();
            if !sounding.is_empty() {
                out.push(Out::Sound(sounding.iter().any(|n| n.state == State::Error)));
            }
        }
        let counted: Vec<&Notice> = if looking { Vec::new() } else { notices.iter().filter(heard).filter(|n| !n.read && !n.trigger && self.rules.badge.has(n.state)).collect() };
        let badge = (counted.len(), counted.iter().any(|n| n.state == State::Error));
        if badge != self.badge {
            self.badge = badge;
            out.push(Out::Badge(badge.0, badge.1));
        }
        out
    }

    /// The number on the taskbar now.
    pub fn badge(&self) -> usize {
        self.badge.0
    }
}

/// The design's words: the folder and what it is doing in the title
/// (`filer is waiting for you`), the work's name and what it said below.
/// Without a folder, the work's name is the title.
fn wording(n: &Notice, place: Option<&str>) -> (String, String) {
    let work = crate::sort::without_spinner(&n.title);
    let who = place.filter(|p| !p.is_empty()).unwrap_or(work);
    let title = match n.state {
        _ if n.trigger => format!("{who}: a trigger matched"),
        State::Waiting | State::MaybeWaiting => format!("{who} is waiting for you"),
        State::Error => format!("{who} failed"),
        State::Done => format!("{who} is done"),
        State::Running => format!("{who} is running"),
    };
    let named = place.is_some_and(|p| !p.is_empty()) && !work.is_empty();
    let body = match (named, n.note.is_empty()) {
        (true, true) => work.to_owned(),
        (true, false) => format!("{work} · {}", n.note),
        (false, _) => n.note.clone(),
    };
    (title, body)
}

/// The title with a mark for the state in front: the notification says at a
/// glance whether it wants a person (⏳), failed (❌) or finished (✅).
fn mark(state: Option<State>, title: &str) -> String {
    match state {
        Some(State::Waiting | State::MaybeWaiting) => format!("⏳ {title}"),
        Some(State::Error) => format!("❌ {title}"),
        Some(State::Done) => format!("✅ {title}"),
        _ => title.to_owned(),
    }
}

/// Whether a waiting session's notification carries Allow and Deny
/// (`[notify] answer_buttons`), read when the toast is made.
static ANSWER_BUTTONS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_answer_buttons(on: bool) {
    ANSWER_BUTTONS.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The window out of the taskbar and in front (a click on a notification
/// while it is minimized; the click gives the right to the front).
#[cfg(windows)]
fn restore_window(handle: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{IsIconic, SW_RESTORE, SetForegroundWindow, ShowWindow};
    let hwnd = HWND(handle as *mut _);
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
}

/// What a click on a notification asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    /// The notification itself, or its Open: go to the session.
    Open,
    /// Allow: the answer a waiting prompt's default takes (Enter).
    Allow,
    /// Deny: Esc.
    Deny,
}

/// Called with the session of a notification that was clicked, and what the
/// click asked for, from whichever thread the system tells it on.
type Click = Arc<dyn Fn(SessionId, Reply) + Send + Sync>;

/// Where the system's part runs.
pub struct Teller {
    tx: mpsc::Sender<Out>,
    clicked: mpsc::Receiver<(SessionId, Reply)>,
}

impl Teller {
    /// `window`: the window's own handle, for the taskbar (Windows). `wake`:
    /// a click came back, to have the window look.
    pub fn start(window: Option<isize>, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let (tx, rx) = mpsc::channel::<Out>();
        let (click_tx, clicked) = mpsc::channel::<(SessionId, Reply)>();
        let click_tx = std::sync::Mutex::new(click_tx);
        let click: Click = Arc::new(move |id, reply| {
            let _ = click_tx.lock().unwrap_or_else(|e| e.into_inner()).send((id, reply));
            // A minimized window draws no frame, so the wake below would
            // reach nobody: bring it back from here.
            #[cfg(windows)]
            if let (Reply::Open, Some(h)) = (reply, window) {
                restore_window(h);
            }
            wake();
        });
        let _ = std::thread::Builder::new().name("alerts".into()).spawn(move || {
            let mut os = os::System::new(window, click);
            for out in rx {
                os.tell(out);
            }
        });
        Self { tx, clicked }
    }

    pub fn send(&self, out: Out) {
        let _ = self.tx.send(out);
    }

    /// The sessions whose notification was clicked since the last call, and
    /// what each click asked for.
    pub fn clicked(&self) -> Vec<(SessionId, Reply)> {
        self.clicked.try_iter().collect()
    }
}

/// Whether the taskbar can carry the number itself; elsewhere the window's
/// title starts with it, which the taskbar and the window switcher show.
pub const TASKBAR_NUMBER: bool = cfg!(windows);

/// The number as a 16 × 16 picture, BGRA top-down with alpha: a gold disc
/// with dark digits, or a red one with white when one failed (`99` at most), for the taskbar
/// button's overlay.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn badge_pixels(n: usize, error: bool) -> Vec<u32> {
    const S: usize = 16;
    // 3 × 5 digits, a row per three bits.
    const DIGITS: [[u8; 5]; 10] = [
        [7, 5, 5, 5, 7],
        [2, 6, 2, 2, 7],
        [7, 1, 7, 4, 7],
        [7, 1, 7, 1, 7],
        [5, 5, 7, 1, 1],
        [7, 4, 7, 1, 7],
        [7, 4, 7, 5, 7],
        [7, 1, 1, 1, 1],
        [7, 5, 7, 5, 7],
        [7, 5, 7, 1, 7],
    ];
    let mut px = vec![0u32; S * S];
    let c = (S as f32 - 1.0) / 2.0;
    for y in 0..S {
        for x in 0..S {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            // A soft edge: alpha falls off over the last pixel.
            let a = ((S as f32 / 2.0 - d).clamp(0.0, 1.0) * 255.0) as u32;
            if a > 0 {
                // Premultiplied, as the icon wants: #e5484d, or #e8c87a.
                let (r, g, b) = if error { (0xe5, 0x48, 0x4d) } else { (0xe8, 0xc8, 0x7a) };
                let (r, g, b) = (r * a / 255, g * a / 255, b * a / 255);
                px[y * S + x] = a << 24 | r << 16 | g << 8 | b;
            }
        }
    }
    let text = n.min(99).to_string();
    let width = text.len() * 4 - 1;
    let (x0, y0) = ((S - width) / 2, (S - 5) / 2);
    // White on red; dark on gold, which white does not read on.
    let ink = if error { 0xffff_ffff } else { 0xff14_1413 };
    for (i, ch) in text.bytes().enumerate() {
        let glyph = DIGITS[(ch - b'0') as usize];
        for (dy, row) in glyph.iter().enumerate() {
            for dx in 0..3 {
                if row >> (2 - dx) & 1 == 1 {
                    px[(y0 + dy) * S + x0 + i * 4 + dx] = ink;
                }
            }
        }
    }
    px
}

/// Text put inside the notification's XML (Windows).
#[cfg_attr(not(windows), allow(dead_code))]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Text put inside an AppleScript string (macOS).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(windows)]
mod os {
    //! A toast through the notification service, and the number as an
    //! overlay on the taskbar button (`ITaskbarList3`).

    use super::{Click, Out, badge_pixels, xml_escape};
    use std::collections::VecDeque;
    use std::sync::mpsc;
    use tsumugi_mux::SessionId;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Foundation::TypedEventHandler;
    use windows::UI::Notifications::{ToastActivatedEventArgs, ToastNotification, ToastNotificationManager};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HGDIOBJ};
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
    use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegSetValueExW};
    use windows::Win32::UI::Shell::{ITaskbarList3, SetCurrentProcessExplicitAppUserModelID, TaskbarList};
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, HICON, ICONINFO};
    use windows::core::{HSTRING, IInspectable, Interface, w};

    /// The app's id to the notification service. It is registered under the
    /// user's own keys (no installer, no shortcut needed), with the name the
    /// notifications show.
    const APP_ID: &str = "uchmk.tsumugi";

    /// Before the window shows: the taskbar groups the window under the id.
    pub fn name_process() {
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID));
        }
    }

    /// The taskbar is an apartment object and lives on this thread (single
    /// threaded); the toasts live on one of their own (multithreaded), where
    /// their `Activated` events can arrive while this one waits for work.
    pub struct System {
        window: Option<HWND>,
        taskbar: Option<ITaskbarList3>,
        toasts: mpsc::Sender<(SessionId, String, String, Option<super::State>)>,
    }

    impl System {
        pub fn new(window: Option<isize>, click: Click) -> Self {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            let taskbar = unsafe { CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_INPROC_SERVER) }.ok();
            let taskbar = taskbar.filter(|t| unsafe { t.HrInit() }.is_ok());
            let (toasts, rx) = mpsc::channel::<(SessionId, String, String, Option<super::State>)>();
            let _ = std::thread::Builder::new().name("toasts".into()).spawn(move || {
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                }
                // Kept, so a click on one still reaches its handler.
                let mut shown = VecDeque::new();
                let mut registered = false;
                let mut logo = None;
                for (session, title, body, state) in rx {
                    // The name and the logo, written at the first notification only.
                    if !registered {
                        logo = write_logo();
                        register(logo.as_deref());
                        registered = true;
                    }
                    if let Ok(t) = toast(&title, &body, session, state, logo.as_deref(), click.clone()) {
                        shown.push_back(t);
                        if shown.len() > 50 {
                            shown.pop_front();
                        }
                    }
                }
            });
            Self { window: window.map(|h| HWND(h as *mut _)), taskbar, toasts }
        }

        pub fn tell(&mut self, out: Out) {
            match out {
                Out::Notify { session, title, body, state } => {
                    let _ = self.toasts.send((session, super::mark(state, &title), body, state));
                }
                // The window does these itself (`RequestUserAttention`, `sound`).
                Out::Flash | Out::Sound(_) => {}
                Out::Badge(n, error) => self.badge(n, error),
            }
        }

        fn badge(&self, n: usize, error: bool) {
            let (Some(taskbar), Some(hwnd)) = (&self.taskbar, self.window) else { return };
            unsafe {
                if n == 0 {
                    let _ = taskbar.SetOverlayIcon(hwnd, HICON::default(), w!(""));
                    return;
                }
                let Some(icon) = icon(&badge_pixels(n, error)) else { return };
                let words = HSTRING::from(format!("{n} waiting"));
                let _ = taskbar.SetOverlayIcon(hwnd, icon, &words);
                // The taskbar keeps its own copy.
                let _ = DestroyIcon(icon);
            }
        }
    }

    /// The logo (`icon::pixels`) as a PNG in the user's local app data, for the
    /// notifications to show: its path, or None if it could not be written.
    fn write_logo() -> Option<std::path::PathBuf> {
        let dir = std::env::var_os("LOCALAPPDATA").map_or_else(std::env::temp_dir, std::path::PathBuf::from).join("tsumugi");
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("logo.png");
        image::save_buffer(&path, &crate::icon::pixels(128), 128, 128, image::ColorType::Rgba8).ok()?;
        Some(path)
    }

    fn register(logo: Option<&std::path::Path>) {
        let key = HSTRING::from(format!("Software\\Classes\\AppUserModelId\\{APP_ID}"));
        unsafe {
            let mut k = HKEY::default();
            if RegCreateKeyExW(HKEY_CURRENT_USER, &key, None, None, REG_OPTION_NON_VOLATILE, KEY_WRITE, None, &mut k, None).is_ok() {
                let name: Vec<u8> = "tsumugi\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
                let _ = RegSetValueExW(k, w!("DisplayName"), None, REG_SZ, Some(&name));
                // The small mark beside the name in the notification's header.
                if let Some(path) = logo {
                    let uri: Vec<u8> = format!("{}\0", path.display()).encode_utf16().flat_map(u16::to_le_bytes).collect();
                    let _ = RegSetValueExW(k, w!("IconUri"), None, REG_SZ, Some(&uri));
                }
                let _ = RegCloseKey(k);
            }
        }
    }

    fn toast(title: &str, body: &str, session: SessionId, state: Option<super::State>, logo: Option<&std::path::Path>, click: Click) -> windows::core::Result<ToastNotification> {
        // The logo beside the words, if it was written.
        let image = logo.map_or_else(String::new, |p| {
            format!("<image placement=\"appLogoOverride\" src=\"file:///{}\"/>", xml_escape(&p.display().to_string().replace('\\', "/")))
        });
        // Open goes to the session, as a click on the toast does; Later is
        // the system's own dismiss (the design's Notify).
        // One that wants a person, or failed, stays on screen until it is
        // dealt with; a finished one goes away by itself.
        let waiting = super::ANSWER_BUTTONS.load(std::sync::atomic::Ordering::Relaxed) && matches!(state, Some(super::State::Waiting | super::State::MaybeWaiting));
        // A waiting session can be answered from the toast: Allow presses
        // Enter, Deny presses Esc (checked again when the click arrives).
        let answers = if waiting { "<action content=\"Allow\" arguments=\"allow\"/><action content=\"Deny\" arguments=\"deny\"/>" } else { "" };
        let scenario = if matches!(state, Some(super::State::Waiting | super::State::MaybeWaiting | super::State::Error)) { " scenario=\"reminder\"" } else { "" };
        let xml = format!(
            "<toast launch=\"open\"{scenario}><visual><binding template=\"ToastGeneric\">{image}<text>{}</text><text>{}</text></binding></visual>\
             <actions>{answers}<action content=\"Open\" arguments=\"open\"/><action content=\"Later\" arguments=\"dismiss\" activationType=\"system\"/></actions></toast>",
            xml_escape(title),
            xml_escape(body)
        );
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;
        // One per session: a newer one replaces the last in the Action
        // Center rather than piling up.
        toast.SetTag(&HSTRING::from(session.to_string()))?;
        toast.SetGroup(&HSTRING::from("sessions"))?;
        // A click on it or on Open, while tsumugi runs, goes to the session.
        toast.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(move |_, args| {
            let said = args.as_ref().and_then(|a| a.cast::<ToastActivatedEventArgs>().ok()).and_then(|a| a.Arguments().ok());
            match said.as_ref().map(|a| a.to_string()).as_deref() {
                None | Some("" | "open") => click(session, super::Reply::Open),
                Some("allow") => click(session, super::Reply::Allow),
                Some("deny") => click(session, super::Reply::Deny),
                _ => {}
            }
            Ok(())
        }))?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?.Show(&toast)?;
        Ok(toast)
    }

    /// A 16 × 16 icon from BGRA pixels.
    unsafe fn icon(px: &[u32]) -> Option<HICON> {
        unsafe {
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: 16,
                    // Negative: top-down rows.
                    biHeight: -16,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let color = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            std::ptr::copy_nonoverlapping(px.as_ptr(), bits.cast::<u32>(), px.len());
            // With an alpha channel the mask is not used, but must be there.
            let mask = CreateBitmap(16, 16, 1, 1, None);
            let ii = ICONINFO { fIcon: true.into(), hbmMask: mask, hbmColor: color, ..Default::default() };
            let icon = CreateIconIndirect(&ii).ok();
            let _ = DeleteObject(HGDIOBJ(color.0));
            let _ = DeleteObject(HGDIOBJ(mask.0));
            icon
        }
    }
}

#[cfg(not(windows))]
mod os {
    //! The notification through the desktop's own tool: `notify-send` on
    //! Linux and the BSDs, AppleScript on macOS. Not there, nothing shows.

    use super::{Click, Out};
    use std::process::{Command, Stdio};

    pub fn name_process() {}

    pub struct System {
        click: Click,
    }

    impl System {
        pub fn new(_window: Option<isize>, click: Click) -> Self {
            Self { click }
        }

        pub fn tell(&mut self, out: Out) {
            if let Out::Notify { session, title, body, state } = out {
                let title = super::mark(state, &title);
                let click = self.click.clone();
                // Each on a thread of its own: one that can say it was
                // clicked waits until it is closed.
                let _ = std::thread::Builder::new().name("notify".into()).spawn(move || {
                    if show(&title, &body) {
                        click(session, super::Reply::Open);
                    }
                });
            }
        }
    }

    /// Show it; whether it was clicked.
    #[cfg(target_os = "macos")]
    fn show(title: &str, body: &str) -> bool {
        // AppleScript's notifications cannot say they were clicked; a click
        // opens Script Editor. A signed app bundle would have its own.
        use super::applescript_string;
        let script = format!("display notification {} with title {}", applescript_string(body), applescript_string(title));
        let _ = Command::new("osascript").arg("-e").arg(script).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        false
    }

    /// Show it; whether it was clicked. libnotify 0.7.12 and later can wait
    /// for a click (`--action --wait`) and print its name; an older
    /// `notify-send` does not know the options, and shows it plainly.
    #[cfg(not(target_os = "macos"))]
    fn show(title: &str, body: &str) -> bool {
        use std::sync::atomic::{AtomicBool, Ordering};
        static PLAIN: AtomicBool = AtomicBool::new(false);
        let run = |actions: bool| {
            let mut cmd = Command::new("notify-send");
            cmd.arg("--app-name=tsumugi");
            if actions {
                cmd.args(["--action=default=Open", "--wait"]);
            }
            cmd.args(["--", title, body]).stdin(Stdio::null()).stderr(Stdio::null());
            cmd.output()
        };
        if !PLAIN.load(Ordering::Relaxed) {
            match run(true) {
                Ok(out) if out.status.success() => return String::from_utf8_lossy(&out.stdout).trim() == "default",
                // Not there at all: nothing more to try.
                Err(_) => return false,
                Ok(_) => PLAIN.store(true, Ordering::Relaxed),
            }
        }
        let _ = run(false);
        false
    }
}

pub use os::name_process;

#[cfg(test)]
mod tests {
    use super::*;

    fn seen<'a>(looking: bool, teller: bool, muted: &'a HashSet<SessionId>) -> Seen<'a> {
        static NONE: std::sync::OnceLock<HashMap<SessionId, String>> = std::sync::OnceLock::new();
        Seen { looking, teller, muted, places: NONE.get_or_init(HashMap::new), spent: NONE.get_or_init(HashMap::new) }
    }

    #[test]
    fn the_folder_names_the_notification() {
        let mut n = notice(0, State::Waiting, false);
        n.title = "Fix the split".into();
        n.note = "Approve the edit".into();
        assert_eq!(wording(&n, Some("filer")), ("filer is waiting for you".into(), "Fix the split · Approve the edit".into()));
        n.state = State::Error;
        n.note.clear();
        assert_eq!(wording(&n, Some("filer")), ("filer failed".into(), "Fix the split".into()));
        assert_eq!(wording(&n, None), ("Fix the split failed".into(), String::new()), "no folder: the work's name");
    }

    fn notice(id: u64, state: State, read: bool) -> Notice {
        Notice { id, session: 1, state, title: "claude".into(), note: String::new(), at_ms: 0, read, trigger: false }
    }

    #[test]
    fn notices_from_before_the_window_are_not_told() {
        let mut a = Alerts::default();
        let none = HashSet::new();
        let old = [notice(0, State::Waiting, false)];
        // Only the number: the notice is unread.
        assert_eq!(a.decide(&old, seen(false, true, &none)), vec![Out::Badge(1, false)]);
        let new = [notice(0, State::Waiting, false), notice(1, State::Error, false)];
        let out = a.decide(&new, seen(false, true, &none));
        assert_eq!(out, vec![Out::Notify { session: 1, title: "claude failed".into(), body: String::new(), state: Some(State::Error) }, Out::Badge(2, true)]);
        // Told once.
        assert_eq!(a.decide(&new, seen(false, true, &none)), vec![]);
    }

    #[test]
    fn nothing_is_told_while_looking() {
        let mut a = Alerts::default();
        let none = HashSet::new();
        a.decide(&[], seen(false, true, &none));
        let out = a.decide(&[notice(0, State::Waiting, false)], seen(true, true, &none));
        assert_eq!(out, vec![]);
        // Looking away later does not tell it then, but counts it.
        assert_eq!(a.decide(&[notice(0, State::Waiting, false)], seen(false, true, &none)), vec![Out::Badge(1, false)]);
        // Looking back clears the number.
        assert_eq!(a.decide(&[notice(0, State::Waiting, false)], seen(true, true, &none)), vec![Out::Badge(0, false)]);
    }

    #[test]
    fn a_trigger_is_told_but_neither_flashed_nor_counted() {
        let mut a = Alerts::default();
        let none = HashSet::new();
        a.decide(&[], seen(false, true, &none));
        a.rules.flash.waiting = true;
        a.rules.sound.waiting = true;
        a.rules.notify.waiting = false;
        let mut n = notice(0, State::Waiting, false);
        n.trigger = true;
        n.note = "BUILD FAILED".into();
        let out = a.decide(&[n.clone()], seen(false, true, &none));
        assert_eq!(out, vec![Out::Notify { session: 1, title: "claude: a trigger matched".into(), body: "BUILD FAILED".into(), state: None }]);
        // Not while looking at it.
        n.id = 1;
        assert_eq!(a.decide(&[n], seen(true, true, &none)), vec![]);
    }

    #[test]
    fn a_done_run_says_what_it_used() {
        let mut a = Alerts::default();
        let none = HashSet::new();
        a.decide(&[], seen(false, true, &none));
        let spent: HashMap<SessionId, String> = HashMap::from([(1, "12.3k tokens · $0.35".to_owned())]);
        let places = HashMap::new();
        let out = a.decide(&[notice(0, State::Done, false)], Seen { looking: false, teller: true, muted: &none, places: &places, spent: &spent });
        assert_eq!(out, vec![Out::Notify { session: 1, title: "claude is done".into(), body: "12.3k tokens · $0.35".into(), state: Some(State::Done) }]);
    }

    #[test]
    fn the_rules_choose() {
        let mut a = Alerts::default();
        let none = HashSet::new();
        a.decide(&[], seen(false, true, &none));
        // A long run done: told, not counted, not flashed.
        let out = a.decide(&[notice(0, State::Done, false)], seen(false, true, &none));
        assert_eq!(out, vec![Out::Notify { session: 1, title: "claude is done".into(), body: String::new(), state: Some(State::Done) }]);
        a.rules.flash.waiting = true;
        a.rules.notify.waiting = false;
        let out = a.decide(&[notice(0, State::Done, false), notice(1, State::Waiting, false)], seen(false, true, &none));
        assert_eq!(out, vec![Out::Flash, Out::Badge(1, false)]);
        // Read ones are neither told nor counted.
        assert_eq!(a.decide(&[notice(2, State::Error, true)], seen(false, true, &none)), vec![Out::Badge(0, false)]);
        // The sound once for all that came together, the error's when one failed.
        a.rules = Rules::default();
        a.rules.notify = Kinds { waiting: false, error: false, done: false };
        a.rules.sound = Kinds { waiting: true, error: true, done: false };
        let out = a.decide(&[notice(3, State::Waiting, false), notice(4, State::Error, false)], seen(false, true, &none));
        assert_eq!(out, vec![Out::Sound(true), Out::Badge(2, true)]);
        assert_eq!(a.decide(&[notice(5, State::Waiting, false)], seen(true, true, &none)), vec![Out::Badge(0, false)], "no sound while looking");
    }

    #[test]
    fn only_the_teller_tells_and_muted_ones_stay_in_the_bell() {
        let mut a = Alerts::default();
        let none = HashSet::new();
        a.decide(&[], seen(false, false, &none));
        // Another window tells; this one only counts.
        assert_eq!(a.decide(&[notice(0, State::Waiting, false)], seen(false, false, &none)), vec![Out::Badge(1, false)]);
        let mut b = Alerts::default();
        let muted = HashSet::from([1]);
        b.decide(&[], seen(false, true, &muted));
        // Muted: neither told nor counted.
        assert_eq!(b.decide(&[notice(0, State::Error, false)], seen(false, true, &muted)), vec![]);
    }

    #[test]
    fn the_badge_draws_its_digits() {
        let px = badge_pixels(7, true);
        assert_eq!(px.len(), 256);
        let white = px.iter().filter(|&&p| p == 0xffff_ffff).count();
        // The 7 is a top row and a stroke down: 3 + 4 pixels.
        assert_eq!(white, 7);
        // The corners are clear, the middle is the disc.
        assert_eq!(px[0] >> 24, 0);
        assert_eq!(px[16 * 2 + 8] >> 24, 255);
        // Two digits fit; more is 99.
        assert_eq!(badge_pixels(123, false), badge_pixels(99, false));
    }

    #[test]
    fn text_is_quoted_for_each_system() {
        assert_eq!(xml_escape("a<b & \"c\""), "a&lt;b &amp; &quot;c&quot;");
        assert_eq!(applescript_string(r#"say "hi" \ bye"#), r#""say \"hi\" \\ bye""#);
    }
}
