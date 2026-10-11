//! Telling someone who is not looking at the window (the design's 1h): the
//! system's notification, a number on the taskbar button, and flashing it.
//!
//! What to do is decided here from the server's notices, the same list the
//! bell shows; doing it is the system's part (`os`), on a thread of its own
//! so a slow notification service never holds up a frame.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, mpsc};

use tsumugi_mux::{Notice, SessionId, State};
use ito_notify::notifier::{Action, Notifier, Toast};
use ito_notify::toast::App;

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
        // A server that began again numbers its notices from 0: the mark
        // from the old one would hide every new notice until they passed it.
        if self.next.is_some_and(|from| !notices.is_empty() && next < from) {
            self.next = None;
        }
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

    /// The mark below which a notice is not new (for the log).
    pub fn mark_next(&self) -> Option<u64> {
        self.next
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

/// Whether a notification that wants a person stays on screen
/// (`[notify] stay`), read when the toast is made.
static STAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_stay(on: bool) {
    STAY.store(on, std::sync::atomic::Ordering::Relaxed);
}

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

/// The app's id to the notification service: the taskbar groups the window
/// under it too.
const APP_ID: &str = "uchmk.tsumugi";

/// The mark beside the name, as a picture file (Windows).
fn logo() -> Option<fn() -> Option<std::path::PathBuf>> {
    #[cfg(windows)]
    return Some(os::write_logo);
    #[cfg(not(windows))]
    None
}

/// One session's notification. Open goes to the session, as a click on the
/// notification does; Later is the system's own dismiss. One that wants a
/// person, or failed, stays on screen until it is dealt with. A waiting
/// session can be answered from it (`answer_buttons`): Allow presses Enter,
/// Deny presses Esc, checked again when the click arrives.
fn toast(session: SessionId, title: &str, body: &str, state: Option<State>) -> Toast {
    let act = |label: &str, arg: &str, dismiss| Action { label: label.into(), arg: arg.into(), dismiss };
    let waiting = matches!(state, Some(State::Waiting | State::MaybeWaiting));
    let mut actions = Vec::new();
    if waiting && ANSWER_BUTTONS.load(std::sync::atomic::Ordering::Relaxed) {
        actions.extend([act("Allow", "allow", false), act("Deny", "deny", false)]);
    }
    actions.extend([act("Open", "open", false), act("Later", "dismiss", true)]);
    Toast {
        title: title.into(),
        body: body.into(),
        // One per session: a newer one replaces the last in the Action Center.
        tag: session.to_string(),
        urgent: (waiting || state == Some(State::Error)) && STAY.load(std::sync::atomic::Ordering::Relaxed),
        launch: "open".into(),
        actions,
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
        let notifier = Notifier::new(
            App { id: APP_ID, name: "tsumugi" },
            logo(),
            Arc::new(move |tag, arg| {
                let Ok(session) = tag.parse::<SessionId>() else { return };
                match arg {
                    "open" => click(session, Reply::Open),
                    "allow" => click(session, Reply::Allow),
                    "deny" => click(session, Reply::Deny),
                    _ => {}
                }
            }),
        );
        let _ = std::thread::Builder::new().name("alerts".into()).spawn(move || {
            let mut os = os::System::new(window);
            for out in rx {
                if let Out::Notify { session, title, body, state } = &out {
                    notifier.show(toast(*session, &mark(*state, title), body, *state));
                }
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

#[cfg(windows)]
mod os {
    //! A toast through the notification service, and the number as an
    //! overlay on the taskbar button (`ITaskbarList3`).

    use super::{Out, badge_pixels};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HGDIOBJ};
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
    use windows::Win32::UI::Shell::{ITaskbarList3, SetCurrentProcessExplicitAppUserModelID, TaskbarList};
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, HICON, ICONINFO};
    use windows::core::{HSTRING, w};

    use super::APP_ID;

    /// Before the window shows: the taskbar groups the window under the id.
    pub fn name_process() {
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID));
        }
    }

    /// The taskbar is an apartment object and lives on this thread (single
    /// threaded). The toasts are `ito-notify`'s, on a thread of its own.
    pub struct System {
        window: Option<HWND>,
        taskbar: Option<ITaskbarList3>,
    }

    impl System {
        pub fn new(window: Option<isize>) -> Self {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            let taskbar = unsafe { CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_INPROC_SERVER) }.ok();
            let taskbar = taskbar.filter(|t| unsafe { t.HrInit() }.is_ok());
            Self { window: window.map(|h| HWND(h as *mut _)), taskbar }
        }

        pub fn tell(&mut self, out: Out) {
            match out {
                // The notification is the notifier's; the window does the
                // rest itself (`RequestUserAttention`, `sound`).
                Out::Notify { .. } | Out::Flash | Out::Sound(_) => {}
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
    pub fn write_logo() -> Option<std::path::PathBuf> {
        let dir = std::env::var_os("LOCALAPPDATA").map_or_else(std::env::temp_dir, std::path::PathBuf::from).join("tsumugi");
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("logo.png");
        image::save_buffer(&path, &crate::icon::pixels(128), 128, 128, image::ColorType::Rgba8).ok()?;
        Some(path)
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
    //! Nothing of its own: the notifications are `ito-notify`'s, and the
    //! number is in the window's title.

    use super::Out;

    pub fn name_process() {}

    pub struct System;

    impl System {
        pub fn new(_window: Option<isize>) -> Self {
            Self
        }

        pub fn tell(&mut self, _out: Out) {}
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
    fn a_server_that_began_again_is_told_from_its_first_new_notice() {
        let mut a = Alerts::default();
        let none = HashSet::new();
        a.decide(&[notice(40, State::Waiting, true), notice(41, State::Waiting, true)], seen(false, true, &none));
        // The new server's list starts again: what it already holds is not told.
        assert_eq!(a.decide(&[notice(0, State::Waiting, false)], seen(false, true, &none)), vec![Out::Badge(1, false)]);
        let out = a.decide(&[notice(0, State::Waiting, false), notice(1, State::Error, false)], seen(false, true, &none));
        assert!(matches!(out.first(), Some(Out::Notify { state: Some(State::Error), .. })));
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
    fn a_toast_has_allow_and_deny_only_for_a_wait() {
        let args = |t: &Toast| t.actions.iter().map(|a| a.arg.clone()).collect::<Vec<_>>();
        set_answer_buttons(true);
        let wait = toast(3, "t", "b", Some(State::Waiting));
        assert_eq!((args(&wait), wait.urgent, wait.tag.as_str(), wait.launch.as_str()), (["allow", "deny", "open", "dismiss"].map(String::from).to_vec(), true, "3", "open"));
        let done = toast(3, "t", "b", Some(State::Done));
        assert_eq!((args(&done), done.urgent), (["open", "dismiss"].map(String::from).to_vec(), false));
        assert!(toast(3, "t", "b", Some(State::Error)).urgent);
        set_stay(false);
        assert!(!toast(3, "t", "b", Some(State::Waiting)).urgent);
        set_stay(true);
    }
}
