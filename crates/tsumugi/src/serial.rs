//! `tsumugi serial PORT[@BAUD[,FRAME]]`: a serial port in a pane, for a
//! board's console (a router's, a microcontroller's). What is typed goes to
//! the port as it is, what the port sends comes out as it is; `Ctrl+]` then
//! `q` leaves, `Ctrl+]` twice sends one. A session runs it through the
//! place `serial:COM3@115200` (`remote.rs`, the server's `serial_bridge`).

use std::io::{Read, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

pub const USAGE: &str =
    "tsumugi serial [--hold] PORT[@BAUD[,FRAME]]  a serial port (COM3, ttyUSB0; 115200 and 8N1 unless said); Ctrl+] then q leaves";

/// The key that starts an escape: `Ctrl+]`, as telnet's.
const ESCAPE: u8 = 0x1d;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parity {
    None,
    Even,
    Odd,
}

/// The port and how to speak on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub port: String,
    pub baud: u32,
    /// Data bits, 5 to 8.
    pub data: u8,
    pub parity: Parity,
    /// Stop bits, 1 or 2.
    pub stop: u8,
}

impl Spec {
    /// The frame as it is written: `8N1`.
    pub fn frame(&self) -> String {
        let p = match self.parity {
            Parity::None => 'N',
            Parity::Even => 'E',
            Parity::Odd => 'O',
        };
        format!("{}{p}{}", self.data, self.stop)
    }
}

/// `COM3`, `COM3@9600`, `/dev/ttyUSB0@115200,7E1`.
pub fn parse(text: &str) -> Result<Spec, String> {
    let text = text.trim();
    let (port, rest) = match text.rsplit_once('@') {
        Some((p, r)) => (p.trim(), Some(r.trim())),
        None => (text, None),
    };
    if port.is_empty() {
        return Err("no port named".into());
    }
    let mut spec = Spec { port: port.into(), baud: 115_200, data: 8, parity: Parity::None, stop: 1 };
    let Some(rest) = rest else { return Ok(spec) };
    let (baud, frame) = match rest.split_once(',') {
        Some((b, f)) => (b.trim(), Some(f.trim())),
        None => (rest, None),
    };
    spec.baud = baud.parse().ok().filter(|b| *b > 0).ok_or_else(|| format!("`{baud}` is not a speed"))?;
    if let Some(frame) = frame {
        let bad = || format!("`{frame}` is not a frame (like 8N1)");
        let f: Vec<char> = frame.chars().collect();
        let [d, p, s] = f[..] else { return Err(bad()) };
        spec.data = d.to_digit(10).filter(|d| (5..=8).contains(d)).ok_or_else(bad)? as u8;
        spec.parity = match p.to_ascii_uppercase() {
            'N' => Parity::None,
            'E' => Parity::Even,
            'O' => Parity::Odd,
            _ => return Err(bad()),
        };
        spec.stop = s.to_digit(10).filter(|s| *s == 1 || *s == 2).ok_or_else(bad)? as u8;
    }
    Ok(spec)
}

/// The file to open: `\\.\COM3` on Windows (`COM10` and up are found only
/// so), `/dev/ttyUSB0` elsewhere; a path stays as it is.
pub fn device(port: &str, windows: bool) -> String {
    if windows {
        if port.starts_with(r"\\") {
            port.into()
        } else {
            format!(r"\\.\{port}")
        }
    } else if port.contains('/') {
        port.into()
    } else {
        format!("/dev/{port}")
    }
}

/// What `Ctrl+]` started.
#[derive(Default)]
pub struct Keys {
    escaped: bool,
}

impl Keys {
    /// The typed bytes: what goes to the port into `out`; true when asked to
    /// leave (`Ctrl+]` then `q` or `.`). `Ctrl+]` twice sends one; after it,
    /// any other key goes as it is and the escape is forgotten.
    pub fn feed(&mut self, input: &[u8], out: &mut Vec<u8>) -> bool {
        for &b in input {
            if self.escaped {
                self.escaped = false;
                match b {
                    b'q' | b'Q' | b'.' => return true,
                    _ => out.push(b),
                }
            } else if b == ESCAPE {
                self.escaped = true;
            } else {
                out.push(b);
            }
        }
        false
    }
}

/// The ports to offer, by the name `parse` takes: the registry's on
/// Windows, USB's on Linux, the call-out devices on macOS. Asks the system;
/// call it off the UI thread.
pub fn ports() -> Vec<String> {
    let mut found = imp::ports();
    found.sort_by_key(|p| natural(p));
    found.dedup();
    found
}

/// `COM3` before `COM10`.
fn natural(name: &str) -> (String, u64, String) {
    let digits = name.find(|c: char| c.is_ascii_digit()).unwrap_or(name.len());
    let tail = &name[digits..];
    let end = tail.find(|c: char| !c.is_ascii_digit()).unwrap_or(tail.len());
    (name[..digits].to_owned(), tail[..end].parse().unwrap_or(0), tail[end..].to_owned())
}

/// Of the names in `/dev`, the serial ports worth offering: USB adapters
/// and boards on Linux (the built-in `ttyS*` are listed whether wired or
/// not), the call-out devices on macOS less the two every Mac has.
#[cfg_attr(windows, allow(dead_code))]
fn unix_ports<'a>(names: impl Iterator<Item = &'a str>, mac: bool) -> Vec<String> {
    names
        .filter(|n| {
            if mac {
                n.starts_with("cu.") && !n.contains("Bluetooth-Incoming-Port") && !n.contains("debug-console")
            } else {
                n.starts_with("ttyUSB") || n.starts_with("ttyACM")
            }
        })
        .map(str::to_owned)
        .collect()
}

enum Ev {
    Quit,
    /// The port went (unplugged); the pane was told.
    Gone,
}

/// A line of our own between the port's.
fn say(out: &mut Box<dyn Write + Send>, text: &str) {
    let _ = out.write_all(format!("\r\n[tsumugi serial: {text}]\r\n").as_bytes());
    let _ = out.flush();
}

/// The command: `args` is what came after `serial`.
pub fn run(args: &[String]) -> ExitCode {
    let hold = args.first().is_some_and(|a| a == "--hold");
    let rest = &args[usize::from(hold)..];
    let spec = match rest {
        [one] => parse(one),
        _ => Err(format!("usage: {USAGE}")),
    };
    let spec = match spec {
        Ok(s) => s,
        Err(e) => {
            crate::err(&format!("tsumugi serial: {e}\n"));
            return ExitCode::from(2);
        }
    };
    let (mut con_in, mut con_out, guard) = match crate::console::raw() {
        Ok(c) => c,
        Err(e) => {
            crate::err(&format!("tsumugi serial: the console: {e}\n"));
            return ExitCode::FAILURE;
        }
    };
    let port = match imp::open(&spec) {
        Ok(p) => p,
        Err(e) => {
            say(&mut con_out, &format!("{}: {e}{}", spec.port, if hold { "; any key closes" } else { "" }));
            if hold {
                let _ = con_in.read(&mut [0; 16]);
            }
            drop(guard);
            return ExitCode::FAILURE;
        }
    };
    let (mut from_port, mut to_port) = match port.try_clone() {
        Ok(r) => (r, port),
        Err(e) => {
            say(&mut con_out, &format!("{}: {e}", spec.port));
            drop(guard);
            return ExitCode::FAILURE;
        }
    };
    say(&mut con_out, &format!("{} at {} {}; Ctrl+] q leaves", spec.port, spec.baud, spec.frame()));
    let (tx, rx) = mpsc::channel();
    let gone = Arc::new(AtomicBool::new(false));
    {
        let tx = tx.clone();
        let port_name = spec.port.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let why = loop {
                match from_port.read(&mut buf) {
                    // On Windows a read gives up after a moment with
                    // nothing (so the writes can get in); elsewhere nothing
                    // means the port went.
                    Ok(0) if cfg!(windows) => continue,
                    Ok(0) => break "it closed".to_owned(),
                    Ok(n) => {
                        if con_out.write_all(&buf[..n]).and_then(|()| con_out.flush()).is_err() {
                            let _ = tx.send(Ev::Quit);
                            return;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => break e.to_string(),
                }
            };
            say(&mut con_out, &format!("{port_name} went: {why}{}", if hold { "; any key closes" } else { "" }));
            let _ = tx.send(Ev::Gone);
        });
    }
    {
        let gone = gone.clone();
        std::thread::spawn(move || {
            let mut keys = Keys::default();
            let mut buf = [0u8; 1024];
            let mut out = Vec::new();
            loop {
                let n = match con_in.read(&mut buf) {
                    Ok(0) => {
                        let _ = tx.send(Ev::Quit);
                        return;
                    }
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => {
                        let _ = tx.send(Ev::Quit);
                        return;
                    }
                };
                if gone.load(Ordering::SeqCst) {
                    let _ = tx.send(Ev::Quit);
                    return;
                }
                out.clear();
                let quit = keys.feed(&buf[..n], &mut out);
                if !out.is_empty() && to_port.write_all(&out).is_err() {
                    // The reading side says why.
                    continue;
                }
                if quit {
                    let _ = tx.send(Ev::Quit);
                    return;
                }
            }
        });
    }
    let code = match rx.recv() {
        Ok(Ev::Gone) => {
            gone.store(true, Ordering::SeqCst);
            if hold {
                // Wait for a key, so the pane says why before it goes.
                let _ = rx.recv();
            }
            ExitCode::FAILURE
        }
        Ok(Ev::Quit) | Err(_) => ExitCode::SUCCESS,
    };
    drop(guard);
    code
}

#[cfg(windows)]
mod imp {
    use super::{Parity, Spec};
    use std::fs::{File, OpenOptions};
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Devices::Communication::{
        GetCommState, SetCommState, SetCommTimeouts, COMMTIMEOUTS, DCB, DCB_PARITY, DCB_STOP_BITS,
    };
    use windows::Win32::Foundation::HANDLE;

    fn handle(f: &File) -> HANDLE {
        HANDLE(f.as_raw_handle())
    }

    pub fn open(spec: &Spec) -> std::io::Result<File> {
        // A COM port is opened by one at a time.
        let port = OpenOptions::new().read(true).write(true).share_mode(0).open(super::device(&spec.port, true))?;
        let h = handle(&port);
        let mut dcb = DCB { DCBlength: std::mem::size_of::<DCB>() as u32, ..Default::default() };
        // SAFETY: the handle is the open port's; the structures are ours.
        unsafe {
            GetCommState(h, &mut dcb)?;
            dcb.BaudRate = spec.baud;
            // fBinary, fParity when there is one, DTR and RTS on (many
            // boards wait for them), no flow control.
            dcb._bitfield = 1 | (u32::from(spec.parity != Parity::None) << 1) | (1 << 4) | (1 << 12);
            dcb.ByteSize = spec.data;
            dcb.Parity = DCB_PARITY(match spec.parity {
                Parity::None => 0,
                Parity::Odd => 1,
                Parity::Even => 2,
            });
            dcb.StopBits = DCB_STOP_BITS(if spec.stop == 2 { 2 } else { 0 });
            SetCommState(h, &dcb)?;
            // What has come, at once; else wait a little and give up, so
            // the writes (which wait for the read on the same handle) get
            // through.
            let t = COMMTIMEOUTS {
                ReadIntervalTimeout: u32::MAX,
                ReadTotalTimeoutMultiplier: u32::MAX,
                ReadTotalTimeoutConstant: 50,
                WriteTotalTimeoutMultiplier: 0,
                WriteTotalTimeoutConstant: 0,
            };
            SetCommTimeouts(h, &t)?;
        }
        Ok(port)
    }

    /// `HKLM\HARDWARE\DEVICEMAP\SERIALCOMM`: the ports there are now, each
    /// value's data a name (`COM3`).
    pub fn ports() -> Vec<String> {
        use windows::core::{w, PWSTR};
        use windows::Win32::Foundation::ERROR_SUCCESS;
        use windows::Win32::System::Registry::{RegCloseKey, RegEnumValueW, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ};
        let mut out = Vec::new();
        let mut key = HKEY::default();
        // SAFETY: the key is closed below; the buffers are ours, their
        // lengths passed with them.
        unsafe {
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, w!(r"HARDWARE\DEVICEMAP\SERIALCOMM"), None, KEY_READ, &mut key) != ERROR_SUCCESS {
                return out;
            }
            for i in 0.. {
                let mut name = [0u16; 256];
                let mut name_len = name.len() as u32;
                let mut data = [0u16; 128];
                let mut data_len = std::mem::size_of_val(&data) as u32;
                let mut kind = 0u32;
                let r = RegEnumValueW(
                    key,
                    i,
                    Some(PWSTR(name.as_mut_ptr())),
                    &mut name_len,
                    None,
                    Some(&mut kind),
                    Some(data.as_mut_ptr().cast()),
                    Some(&mut data_len),
                );
                if r != ERROR_SUCCESS {
                    break;
                }
                if kind == REG_SZ.0 {
                    let units = &data[..(data_len as usize / 2).min(data.len())];
                    let text = String::from_utf16_lossy(units);
                    let text = text.trim_end_matches('\0').trim();
                    if !text.is_empty() {
                        out.push(text.to_owned());
                    }
                }
            }
            let _ = RegCloseKey(key);
        }
        out
    }
}

#[cfg(unix)]
mod imp {
    use super::{Parity, Spec};
    use std::fs::{File, OpenOptions};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;

    fn check(r: libc::c_int) -> std::io::Result<()> {
        if r == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn speed(baud: u32) -> Option<libc::speed_t> {
        use libc::*;
        Some(match baud {
            50 => B50,
            75 => B75,
            110 => B110,
            134 => B134,
            150 => B150,
            200 => B200,
            300 => B300,
            600 => B600,
            1200 => B1200,
            1800 => B1800,
            2400 => B2400,
            4800 => B4800,
            9600 => B9600,
            19200 => B19200,
            38400 => B38400,
            57600 => B57600,
            115200 => B115200,
            230400 => B230400,
            460800 => B460800,
            500000 => B500000,
            576000 => B576000,
            921600 => B921600,
            1000000 => B1000000,
            1152000 => B1152000,
            1500000 => B1500000,
            2000000 => B2000000,
            2500000 => B2500000,
            3000000 => B3000000,
            3500000 => B3500000,
            4000000 => B4000000,
            _ => return None,
        })
    }

    /// The BSDs and macOS take the speed as a number.
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    fn speed(baud: u32) -> Option<libc::speed_t> {
        Some(baud as libc::speed_t)
    }

    pub fn open(spec: &Spec) -> std::io::Result<File> {
        let speed = speed(spec.baud).ok_or_else(|| std::io::Error::other(format!("{} is not a speed this system knows", spec.baud)))?;
        // Not as our terminal; and not waiting for the modem's carrier.
        let port = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
            .open(super::device(&spec.port, false))?;
        let fd = port.as_raw_fd();
        // SAFETY: the descriptor is the open port's; the termios is ours.
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            check(libc::tcgetattr(fd, &mut t))?;
            libc::cfmakeraw(&mut t);
            t.c_cflag &= !(libc::CSIZE | libc::PARENB | libc::PARODD | libc::CSTOPB | libc::CRTSCTS);
            t.c_cflag |= libc::CLOCAL | libc::CREAD;
            t.c_cflag |= match spec.data {
                5 => libc::CS5,
                6 => libc::CS6,
                7 => libc::CS7,
                _ => libc::CS8,
            };
            match spec.parity {
                Parity::None => {}
                Parity::Even => t.c_cflag |= libc::PARENB,
                Parity::Odd => t.c_cflag |= libc::PARENB | libc::PARODD,
            }
            if spec.stop == 2 {
                t.c_cflag |= libc::CSTOPB;
            }
            t.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);
            t.c_cc[libc::VMIN] = 1;
            t.c_cc[libc::VTIME] = 0;
            check(libc::cfsetspeed(&mut t, speed))?;
            check(libc::tcsetattr(fd, libc::TCSANOW, &t))?;
            // From here the reads wait.
            let flags = libc::fcntl(fd, libc::F_GETFL);
            check(flags)?;
            check(libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK))?;
        }
        Ok(port)
    }

    pub fn ports() -> Vec<String> {
        let Ok(dir) = std::fs::read_dir("/dev") else { return Vec::new() };
        let names: Vec<String> = dir.flatten().filter_map(|e| e.file_name().into_string().ok()).collect();
        super::unix_ports(names.iter().map(String::as_str), cfg!(target_os = "macos"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_port_with_or_without_its_speed() {
        assert_eq!(parse("COM3").unwrap(), Spec { port: "COM3".into(), baud: 115_200, data: 8, parity: Parity::None, stop: 1 });
        assert_eq!(parse(" COM3@9600 ").unwrap().baud, 9600);
        let s = parse("/dev/ttyUSB0@57600,7e2").unwrap();
        assert_eq!((s.port.as_str(), s.baud, s.data, s.parity, s.stop), ("/dev/ttyUSB0", 57600, 7, Parity::Even, 2));
        assert_eq!(s.frame(), "7E2");
        assert_eq!(parse("ttyACM0@115200,8O1").unwrap().parity, Parity::Odd);
        for bad in ["", "@9600", "COM3@fast", "COM3@0", "COM3@9600,9N1", "COM3@9600,8X1", "COM3@9600,8N3", "COM3@9600,8N"] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_device_to_open() {
        assert_eq!(device("COM3", true), r"\\.\COM3");
        assert_eq!(device(r"\\.\COM12", true), r"\\.\COM12");
        assert_eq!(device("ttyUSB0", false), "/dev/ttyUSB0");
        assert_eq!(device("/dev/cu.usbserial-10", false), "/dev/cu.usbserial-10");
    }

    #[test]
    fn ctrl_bracket_then_q_leaves() {
        let mut k = Keys::default();
        let mut out = Vec::new();
        assert!(!k.feed(b"ls\r", &mut out));
        assert_eq!(out, b"ls\r");
        out.clear();
        // Twice sends one; another key after it goes as it is.
        assert!(!k.feed(&[ESCAPE, ESCAPE, b'a', ESCAPE, b'x'], &mut out));
        assert_eq!(out, [ESCAPE, b'a', b'x']);
        out.clear();
        // Split across reads, and what came before it still goes.
        assert!(!k.feed(b"y\x1d", &mut out));
        assert!(k.feed(b"q", &mut out));
        assert_eq!(out, b"y");
        let mut k = Keys::default();
        assert!(k.feed(b"\x1d.", &mut Vec::new()));
        assert!(!Keys::default().feed(b"q", &mut Vec::new()));
    }

    #[test]
    fn the_ports_worth_offering() {
        let linux = ["tty0", "ttyS0", "ttyS31", "ttyUSB0", "ttyACM1", "null"];
        assert_eq!(unix_ports(linux.into_iter(), false), ["ttyUSB0", "ttyACM1"]);
        let mac = ["cu.Bluetooth-Incoming-Port", "cu.debug-console", "cu.usbserial-110", "tty.usbserial-110", "cu.usbmodem1101"];
        assert_eq!(unix_ports(mac.into_iter(), true), ["cu.usbserial-110", "cu.usbmodem1101"]);
        let mut names = vec!["COM10".to_owned(), "COM3".into(), "COM1".into()];
        names.sort_by_key(|p| natural(p));
        assert_eq!(names, ["COM1", "COM3", "COM10"]);
    }
}
