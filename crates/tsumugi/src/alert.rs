//! Telling someone who is not looking at the window (the design's 1h): the
//! system's notification, a number on the taskbar button, and flashing it.
//!
//! What to do is decided here from the server's notices, the same list the
//! bell shows; doing it is the system's part (`os`), on a thread of its own
//! so a slow notification service never holds up a frame.

use std::sync::mpsc;

use tsumugi_mux::{Notice, State};

/// Which ways to tell, per state. The settings screen (1m) will set these;
/// until then they are the design's defaults.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    /// The system's notification.
    pub notify: Kinds,
    /// Counted in the number on the taskbar button.
    pub badge: Kinds,
    /// Flash the taskbar button.
    pub flash: Kinds,
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

impl Default for Rules {
    /// The design's 1h: notifications for all three, the number for the two
    /// that want a person, no flashing.
    fn default() -> Self {
        let all = Kinds { waiting: true, error: true, done: true };
        let none = Kinds { waiting: false, error: false, done: false };
        Self { notify: all, badge: Kinds { done: false, ..all }, flash: none }
    }
}

/// One thing to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    Notify { title: String, body: String },
    Flash,
    Badge(usize),
}

/// What has been told so far.
#[derive(Default)]
pub struct Alerts {
    pub rules: Rules,
    /// Notices below this id have been seen; `None` before the first list,
    /// whose notices are from before this window and are not told again.
    next: Option<u64>,
    /// The number last put on the taskbar.
    badge: usize,
}

impl Alerts {
    /// What the notices that arrived since the last call ask for. `looking`:
    /// the window has the keyboard, and nothing needs telling.
    pub fn decide(&mut self, notices: &[Notice], looking: bool) -> Vec<Out> {
        let mut out = Vec::new();
        let next = notices.iter().map(|n| n.id + 1).max().unwrap_or(0);
        let fresh: Vec<&Notice> = match self.next {
            Some(from) => notices.iter().filter(|n| n.id >= from && !n.read).collect(),
            None => Vec::new(),
        };
        self.next = Some(next.max(self.next.unwrap_or(0)));
        if !looking {
            for n in fresh.iter().filter(|n| self.rules.notify.has(n.state)) {
                out.push(Out::Notify { title: n.title.clone(), body: body(n) });
            }
            if fresh.iter().any(|n| self.rules.flash.has(n.state)) {
                out.push(Out::Flash);
            }
        }
        let badge = if looking { 0 } else { notices.iter().filter(|n| !n.read && self.rules.badge.has(n.state)).count() };
        if badge != self.badge {
            self.badge = badge;
            out.push(Out::Badge(badge));
        }
        out
    }

    /// The number on the taskbar now.
    pub fn badge(&self) -> usize {
        self.badge
    }
}

fn body(n: &Notice) -> String {
    let what = match n.state {
        State::Waiting | State::MaybeWaiting => "Waiting for you",
        State::Error => "Error",
        State::Done => "Done",
        State::Running => "Running",
    };
    if n.note.is_empty() { what.to_owned() } else { format!("{what} · {}", n.note) }
}

/// Where the system's part runs.
pub struct Teller(mpsc::Sender<Out>);

impl Teller {
    /// `window`: the window's own handle, for the taskbar (Windows).
    pub fn start(window: Option<isize>) -> Self {
        let (tx, rx) = mpsc::channel::<Out>();
        let _ = std::thread::Builder::new().name("alerts".into()).spawn(move || {
            let mut os = os::System::new(window);
            for out in rx {
                os.tell(out);
            }
        });
        Self(tx)
    }

    pub fn send(&self, out: Out) {
        let _ = self.0.send(out);
    }
}

/// Whether the taskbar can carry the number itself; elsewhere the window's
/// title starts with it, which the taskbar and the window switcher show.
pub const TASKBAR_NUMBER: bool = cfg!(windows);

/// The number as a 16 × 16 picture, BGRA top-down with alpha: a red disc
/// with white digits (`99` at most), for the taskbar button's overlay.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn badge_pixels(n: usize) -> Vec<u32> {
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
                // Premultiplied, as the icon wants: #e5484d.
                let (r, g, b) = (0xe5 * a / 255, 0x48 * a / 255, 0x4d * a / 255);
                px[y * S + x] = a << 24 | r << 16 | g << 8 | b;
            }
        }
    }
    let text = n.min(99).to_string();
    let width = text.len() * 4 - 1;
    let (x0, y0) = ((S - width) / 2, (S - 5) / 2);
    for (i, ch) in text.bytes().enumerate() {
        let glyph = DIGITS[(ch - b'0') as usize];
        for (dy, row) in glyph.iter().enumerate() {
            for dx in 0..3 {
                if row >> (2 - dx) & 1 == 1 {
                    px[(y0 + dy) * S + x0 + i * 4 + dx] = 0xffff_ffff;
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

    use super::{Out, badge_pixels, xml_escape};
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HGDIOBJ};
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
    use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegSetValueExW};
    use windows::Win32::UI::Shell::{ITaskbarList3, SetCurrentProcessExplicitAppUserModelID, TaskbarList};
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, HICON, ICONINFO};
    use windows::core::{HSTRING, w};

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

    pub struct System {
        window: Option<HWND>,
        taskbar: Option<ITaskbarList3>,
        registered: bool,
    }

    impl System {
        pub fn new(window: Option<isize>) -> Self {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            let taskbar = unsafe { CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_INPROC_SERVER) }.ok();
            let taskbar = taskbar.filter(|t| unsafe { t.HrInit() }.is_ok());
            Self { window: window.map(|h| HWND(h as *mut _)), taskbar, registered: false }
        }

        pub fn tell(&mut self, out: Out) {
            match out {
                Out::Notify { title, body } => {
                    if !self.registered {
                        register();
                        self.registered = true;
                    }
                    let _ = toast(&title, &body);
                }
                // The window asks for this itself (`RequestUserAttention`).
                Out::Flash => {}
                Out::Badge(n) => self.badge(n),
            }
        }

        fn badge(&self, n: usize) {
            let (Some(taskbar), Some(hwnd)) = (&self.taskbar, self.window) else { return };
            unsafe {
                if n == 0 {
                    let _ = taskbar.SetOverlayIcon(hwnd, HICON::default(), w!(""));
                    return;
                }
                let Some(icon) = icon(&badge_pixels(n)) else { return };
                let words = HSTRING::from(format!("{n} waiting"));
                let _ = taskbar.SetOverlayIcon(hwnd, icon, &words);
                // The taskbar keeps its own copy.
                let _ = DestroyIcon(icon);
            }
        }
    }

    fn register() {
        let key = HSTRING::from(format!("Software\\Classes\\AppUserModelId\\{APP_ID}"));
        unsafe {
            let mut k = HKEY::default();
            if RegCreateKeyExW(HKEY_CURRENT_USER, &key, None, None, REG_OPTION_NON_VOLATILE, KEY_WRITE, None, &mut k, None).is_ok() {
                let name: Vec<u8> = "tsumugi\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
                let _ = RegSetValueExW(k, w!("DisplayName"), None, REG_SZ, Some(&name));
                let _ = RegCloseKey(k);
            }
        }
    }

    fn toast(title: &str, body: &str) -> windows::core::Result<()> {
        let xml = format!(
            "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
            xml_escape(title),
            xml_escape(body)
        );
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?.Show(&toast)
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

    use super::Out;
    use std::process::{Command, Stdio};

    pub fn name_process() {}

    pub struct System;

    impl System {
        pub fn new(_window: Option<isize>) -> Self {
            Self
        }

        pub fn tell(&mut self, out: Out) {
            if let Out::Notify { title, body } = out {
                let mut cmd = command(&title, &body);
                cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
                // Waited for, so no child is left a zombie; this thread
                // does nothing else.
                let _ = cmd.status();
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn command(title: &str, body: &str) -> Command {
        use super::applescript_string;
        let script = format!("display notification {} with title {}", applescript_string(body), applescript_string(title));
        let mut cmd = Command::new("osascript");
        cmd.arg("-e").arg(script);
        cmd
    }

    #[cfg(not(target_os = "macos"))]
    fn command(title: &str, body: &str) -> Command {
        let mut cmd = Command::new("notify-send");
        cmd.args(["--app-name=tsumugi", "--", title, body]);
        cmd
    }
}

pub use os::name_process;

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(id: u64, state: State, read: bool) -> Notice {
        Notice { id, session: 1, state, title: "claude".into(), note: String::new(), at_ms: 0, read }
    }

    #[test]
    fn notices_from_before_the_window_are_not_told() {
        let mut a = Alerts::default();
        let old = [notice(0, State::Waiting, false)];
        // Only the number: the notice is unread.
        assert_eq!(a.decide(&old, false), vec![Out::Badge(1)]);
        let new = [notice(0, State::Waiting, false), notice(1, State::Error, false)];
        let out = a.decide(&new, false);
        assert_eq!(out, vec![Out::Notify { title: "claude".into(), body: "Error".into() }, Out::Badge(2)]);
        // Told once.
        assert_eq!(a.decide(&new, false), vec![]);
    }

    #[test]
    fn nothing_is_told_while_looking() {
        let mut a = Alerts::default();
        a.decide(&[], false);
        let out = a.decide(&[notice(0, State::Waiting, false)], true);
        assert_eq!(out, vec![]);
        // Looking away later does not tell it then, but counts it.
        assert_eq!(a.decide(&[notice(0, State::Waiting, false)], false), vec![Out::Badge(1)]);
        // Looking back clears the number.
        assert_eq!(a.decide(&[notice(0, State::Waiting, false)], true), vec![Out::Badge(0)]);
    }

    #[test]
    fn the_rules_choose() {
        let mut a = Alerts::default();
        a.decide(&[], false);
        // A long run done: told, not counted, not flashed.
        let out = a.decide(&[notice(0, State::Done, false)], false);
        assert_eq!(out, vec![Out::Notify { title: "claude".into(), body: "Done".into() }]);
        a.rules.flash.waiting = true;
        a.rules.notify.waiting = false;
        let out = a.decide(&[notice(0, State::Done, false), notice(1, State::Waiting, false)], false);
        assert_eq!(out, vec![Out::Flash, Out::Badge(1)]);
        // Read ones are neither told nor counted.
        assert_eq!(a.decide(&[notice(2, State::Error, true)], false), vec![Out::Badge(0)]);
    }

    #[test]
    fn the_badge_draws_its_digits() {
        let px = badge_pixels(7);
        assert_eq!(px.len(), 256);
        let white = px.iter().filter(|&&p| p == 0xffff_ffff).count();
        // The 7 is a top row and a stroke down: 3 + 4 pixels.
        assert_eq!(white, 7);
        // The corners are clear, the middle is the disc.
        assert_eq!(px[0] >> 24, 0);
        assert_eq!(px[16 * 2 + 8] >> 24, 255);
        // Two digits fit; more is 99.
        assert_eq!(badge_pixels(123), badge_pixels(99));
    }

    #[test]
    fn text_is_quoted_for_each_system() {
        assert_eq!(xml_escape("a<b & \"c\""), "a&lt;b &amp; &quot;c&quot;");
        assert_eq!(applescript_string(r#"say "hi" \ bye"#), r#""say \"hi\" \\ bye""#);
    }
}
