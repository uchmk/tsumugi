//! `tsumugi tmux [--ssh HOST] [-t NAME]`: a tmux session's panes as
//! tsumugi's, through tmux's control mode (`tmux -u -C`), as iTerm2 does. Each
//! tmux window is a tab, each of its panes a pane split the same way; what
//! a pane shows comes from tmux's `%output`, what is typed goes back by
//! `send-keys`, and a pane's size in tsumugi becomes its size in tmux. The
//! programs stay in tmux, so they outlive the window, the line and this
//! command: run it again and they are back.
//!
//! This command is the gateway: it runs `tmux -u -C`, here or over ssh, and
//! starts a session for each pane that runs `tsumugi tmux-pane` (the
//! bridge), which passes one pane's bytes between its console and the
//! gateway over a socket on 127.0.0.1. Closing a pane in tsumugi lets go of
//! it (the program goes on in tmux); a pane that ends in tmux ends here.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::ExitCode;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use tsumugi_mux::{Client, Dir, Place, SessionId};

pub const USAGE: &str = "tsumugi tmux [--ssh HOST] [-t NAME]      a tmux session's windows as tabs and its panes as panes (control mode; tmux here, or on HOST)";

/// What `tsumugi tmux` was asked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Opts {
    pub ssh: Option<String>,
    /// The session: attached to, made when there is none.
    pub target: Option<String>,
}

pub fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--ssh" => o.ssh = Some(it.next().filter(|h| !h.is_empty()).ok_or("--ssh needs a host")?.clone()),
            "-t" => o.target = Some(it.next().filter(|t| !t.is_empty()).ok_or("-t needs a session's name")?.clone()),
            other => return Err(format!("`{other}` is not an option")),
        }
    }
    Ok(o)
}

/// `text` for `sh`, as one word.
fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// The shell's command that starts tmux in control mode: the named session
/// (made when it is not there), or the last one, or a new one. `-u`: names
/// and paths come as UTF-8 (and a tab as itself) whatever the locale.
pub fn command(target: Option<&str>) -> String {
    match target {
        Some(t) => format!("exec tmux -u -C new-session -A -s {}", sh_quote(t)),
        None => "tmux has-session 2>/dev/null && exec tmux -u -C attach || exec tmux -u -C new-session".into(),
    }
}

/// tmux's `%output` text: a byte below a space and `\` come as `\ooo`.
pub fn unescape(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let oct = |b: u8| (b'0'..=b'7').contains(&b);
        if text[i] == b'\\' && i + 4 <= text.len() && text[i + 1..i + 4].iter().all(|&b| oct(b)) {
            let n = text[i + 1..i + 4].iter().fold(0u32, |n, &b| n * 8 + u32::from(b - b'0'));
            out.push(n as u8);
            i += 4;
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    out
}

/// A window's layout as tmux writes it (`#{window_layout}`): a pane, or
/// panes side by side (`{…}`) or one above another (`[…]`), with sizes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cell {
    Pane(u32, (u16, u16)),
    /// Left to right.
    Row(Vec<Cell>, (u16, u16)),
    /// Top to bottom.
    Col(Vec<Cell>, (u16, u16)),
}

/// `f91d,120x40,0,0{60x40,0,0,0,59x40,61,0,1}`: the checksum, then cells
/// of `WxH,X,Y` and a pane's number or the cells inside.
pub fn layout(text: &str) -> Option<Cell> {
    let (_sum, rest) = text.split_once(',')?;
    let mut at = rest.as_bytes();
    let cell = parse_cell(&mut at)?;
    at.is_empty().then_some(cell)
}

fn number(at: &mut &[u8]) -> Option<u32> {
    let n = at.iter().take_while(|b| b.is_ascii_digit()).count();
    let v = std::str::from_utf8(&at[..n]).ok()?.parse().ok()?;
    *at = &at[n..];
    Some(v)
}

fn eat(at: &mut &[u8], b: u8) -> Option<()> {
    (at.first() == Some(&b)).then(|| *at = &at[1..])
}

fn parse_cell(at: &mut &[u8]) -> Option<Cell> {
    let w = number(at)?;
    eat(at, b'x')?;
    let h = number(at)?;
    eat(at, b',')?;
    number(at)?;
    eat(at, b',')?;
    number(at)?;
    let size = (u16::try_from(w).ok()?, u16::try_from(h).ok()?);
    match at.first()? {
        b',' => {
            eat(at, b',')?;
            Some(Cell::Pane(number(at)?, size))
        }
        &open @ (b'{' | b'[') => {
            *at = &at[1..];
            let close = if open == b'{' { b'}' } else { b']' };
            let mut cells = vec![parse_cell(at)?];
            while eat(at, b',').is_some() {
                cells.push(parse_cell(at)?);
            }
            eat(at, close)?;
            Some(if open == b'{' { Cell::Row(cells, size) } else { Cell::Col(cells, size) })
        }
        _ => None,
    }
}

impl Cell {
    /// The panes, left to right and top to bottom, each with its way from
    /// the top: at each split, whether it is side by side and which part.
    fn leaves(&self) -> Vec<(u32, Vec<(bool, usize)>)> {
        fn walk(c: &Cell, path: &mut Vec<(bool, usize)>, out: &mut Vec<(u32, Vec<(bool, usize)>)>) {
            let (cells, row) = match c {
                Cell::Pane(id, _) => return out.push((*id, path.clone())),
                Cell::Row(cells, _) => (cells, true),
                Cell::Col(cells, _) => (cells, false),
            };
            for (i, c) in cells.iter().enumerate() {
                path.push((row, i));
                walk(c, path, out);
                path.pop();
            }
        }
        let mut out = Vec::new();
        walk(self, &mut Vec::new(), &mut out);
        out
    }

    pub fn panes(&self) -> Vec<u32> {
        self.leaves().into_iter().map(|(id, _)| id).collect()
    }

    /// The window's size were each pane the size `size` says (its own
    /// when it says nothing): side by side, the widths and a column
    /// between each; one above another, the heights and a line between.
    pub fn fit(&self, size: &impl Fn(u32) -> Option<(u16, u16)>) -> (u16, u16) {
        match self {
            Cell::Pane(id, own) => size(*id).unwrap_or(*own),
            Cell::Row(cells, _) | Cell::Col(cells, _) => {
                let row = matches!(self, Cell::Row(..));
                let sizes: Vec<(u16, u16)> = cells.iter().map(|c| c.fit(size)).collect();
                let gaps = sizes.len().saturating_sub(1) as u16;
                let (along, across): (Vec<u16>, Vec<u16>) = sizes.iter().map(|&(w, h)| if row { (w, h) } else { (h, w) }).unzip();
                let total = along.iter().fold(gaps, |a, &b| a.saturating_add(b));
                let most = across.into_iter().max().unwrap_or(1);
                if row { (total, most) } else { (most, total) }
            }
        }
    }
}

/// Where the pane `id` goes in tsumugi: `None` for a tab of its own (no
/// pane of its window is there yet), else beside the pane that is, the
/// nearest before it (or after), split the way the two are in tmux.
pub fn place(cell: &Cell, id: u32, there: impl Fn(u32) -> bool) -> Option<(u32, Dir)> {
    let leaves = cell.leaves();
    let me = leaves.iter().position(|(l, _)| *l == id)?;
    let before = leaves[..me].iter().rev();
    let after = leaves[me + 1..].iter();
    let (other, path) = before.chain(after).find(|(l, _)| there(*l))?;
    let mine = &leaves[me].1;
    let split = mine.iter().zip(path).position(|(a, b)| a != b).unwrap_or(0);
    let row = mine.get(split).is_none_or(|(row, _)| *row);
    Some((*other, if row { Dir::Right } else { Dir::Down }))
}

/// One line of tmux's, out of a reply.
#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    /// A reply starts; `mine` when it answers one of our commands (the
    /// attach's own has flags 0).
    Begin { number: u64, mine: bool },
    /// A reply ends, or ends saying the command failed.
    End { number: u64, failed: bool },
    Output { pane: u32, bytes: Vec<u8> },
    /// The panes may have changed: a window came or went, a split moved,
    /// the session is another.
    Changed,
    Renamed { window: u32, name: String },
    Exit(String),
    Other,
}

pub fn line(text: &[u8]) -> Line {
    let words = |t: &[u8]| String::from_utf8_lossy(t).split(' ').map(str::to_owned).collect::<Vec<_>>();
    let tag_len = text.iter().position(|&b| b == b' ').unwrap_or(text.len());
    let rest = text.get(tag_len + 1..).unwrap_or(&[]);
    let num = |w: Option<&String>, sigil: char| w.and_then(|w| w.strip_prefix(sigil)).and_then(|n| n.parse().ok());
    match &text[..tag_len] {
        b"%begin" | b"%end" | b"%error" => {
            let w = words(rest);
            let number = w.get(1).and_then(|n| n.parse().ok());
            let flags: Option<u32> = w.get(2).and_then(|n| n.parse().ok());
            match (&text[..tag_len], number) {
                (b"%begin", Some(number)) => Line::Begin { number, mine: flags.is_some_and(|f| f & 1 == 1) },
                (tag, Some(number)) => Line::End { number, failed: tag == b"%error" },
                _ => Line::Other,
            }
        }
        b"%output" => {
            let id_len = rest.iter().position(|&b| b == b' ').unwrap_or(rest.len());
            let id = std::str::from_utf8(&rest[..id_len]).ok().and_then(|p| p.strip_prefix('%')).and_then(|n| n.parse().ok());
            match id {
                Some(pane) => Line::Output { pane, bytes: unescape(rest.get(id_len + 1..).unwrap_or(&[])) },
                None => Line::Other,
            }
        }
        b"%layout-change" | b"%window-add" | b"%window-close" | b"%unlinked-window-close" | b"%session-changed" => Line::Changed,
        b"%window-renamed" => {
            let w = words(rest);
            match num(w.first(), '@') {
                Some(window) => Line::Renamed { window, name: w[1..].join(" ") },
                None => Line::Other,
            }
        }
        b"%exit" => Line::Exit(String::from_utf8_lossy(rest).into_owned()),
        _ => Line::Other,
    }
}

/// A pane as `list-panes` says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub pane: u32,
    pub window: u32,
    pub size: (u16, u16),
    pub layout: String,
    pub path: String,
    pub name: String,
}

/// What `list-panes` is asked to write, a pane a line (the path last: it may hold a tab).
const LIST: &str = "list-panes -s -F '#{pane_id}\t#{window_id}\t#{pane_width}\t#{pane_height}\t#{window_layout}\t#{window_name}\t#{pane_current_path}'";

pub fn listed(text: &[u8]) -> Option<Listed> {
    let text = String::from_utf8_lossy(text);
    let f: Vec<&str> = text.splitn(7, '\t').collect();
    let [pane, window, w, h, layout, name, path] = f[..] else { return None };
    Some(Listed {
        pane: pane.strip_prefix('%')?.parse().ok()?,
        window: window.strip_prefix('@')?.parse().ok()?,
        size: (w.parse().ok()?, h.parse().ok()?),
        layout: layout.into(),
        path: path.into(),
        name: name.into(),
    })
}

/// Where the cursor is and the modes a snapshot puts back (`display -p`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub alternate: bool,
    pub shown: bool,
    pub keypad: bool,
}

const CURSOR: &str = "'#{cursor_x} #{cursor_y} #{alternate_on} #{cursor_flag} #{keypad_cursor_flag}'";

pub fn cursor(text: &[u8]) -> Option<Cursor> {
    let text = String::from_utf8_lossy(text);
    let n: Vec<u16> = text.split_whitespace().map(|w| w.parse().ok()).collect::<Option<_>>()?;
    let [x, y, alternate, shown, keypad] = n[..] else { return None };
    Some(Cursor { x, y, alternate: alternate == 1, shown: shown == 1, keypad: keypad == 1 })
}

/// What a bridge writes first: the pane's screen as tmux has it (`lines`
/// from `capture-pane -e`), and the cursor where it is.
pub fn snapshot(c: Cursor, lines: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    if c.alternate {
        out.extend_from_slice(b"\x1b[?1049h");
    }
    out.extend_from_slice(b"\x1b[H\x1b[2J");
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(b"\x1b[0m\r\n");
        }
        out.extend_from_slice(l);
    }
    out.extend_from_slice(format!("\x1b[0m\x1b[{};{}H", c.y + 1, c.x + 1).as_bytes());
    out.extend_from_slice(if c.shown { b"\x1b[?25h" } else { b"\x1b[?25l" });
    if c.keypad {
        out.extend_from_slice(b"\x1b[?1h");
    }
    out
}

/// The commands that type `bytes` into the pane: `send-keys -H`, a few
/// hundred bytes each.
pub fn keys(pane: u32, bytes: &[u8]) -> Vec<String> {
    bytes.chunks(256).map(|c| {
        let hex: Vec<String> = c.iter().map(|b| format!("{b:02x}")).collect();
        format!("send-keys -t %{pane} -H {}", hex.join(" "))
    }).collect()
}

// The socket between the gateway and a bridge: a kind byte, the length
// (4 bytes, big-endian) and that many bytes. A bridge says `h` (the token
// and its pane), `k` (keys) and `s` (`COLS ROWS`); the gateway says `o`
// (output), `t` (a title) and `x` (why the pane ends).

pub fn frame(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 5);
    out.push(kind);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn read_frame(r: &mut impl Read) -> std::io::Result<(u8, Vec<u8>)> {
    let mut head = [0u8; 5];
    r.read_exact(&mut head)?;
    let len = u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize;
    if len > 16 << 20 {
        return Err(std::io::Error::other("a frame too long"));
    }
    let mut payload = vec![0; len];
    r.read_exact(&mut payload)?;
    Ok((head[0], payload))
}

/// A token no other program on this machine can guess, so only our
/// bridges are let in.
fn token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

/// What the gateway's threads tell it.
enum Ev {
    /// A line of tmux's.
    Line(Vec<u8>),
    /// tmux is gone.
    Eof,
    Hello { conn: u64, pane: u32, to: TcpStream },
    Keys(u64, Vec<u8>),
    Size(u64, (u16, u16)),
    Bye(u64),
}

/// What a reply answers.
enum Want {
    Nothing,
    Panes,
    Cursor(u32),
    Capture(u32),
}

/// A tmux pane and its tsumugi session.
struct Pane {
    window: u32,
    /// Its size in tmux.
    size: (u16, u16),
    session: Option<SessionId>,
    conn: Option<u64>,
    /// Its snapshot is written: output goes now.
    ready: bool,
    /// Closed in tsumugi: not brought back.
    let_go: bool,
    /// The size its pane has in tsumugi, as its bridge said.
    want: Option<(u16, u16)>,
    cursor: Cursor,
}

struct Gateway {
    client: Client,
    to_tmux: Box<dyn Write + Send>,
    pending: VecDeque<Want>,
    /// The reply being read: whose, and its lines.
    reply: Option<(bool, Vec<Vec<u8>>)>,
    panes: HashMap<u32, Pane>,
    /// By window: its layout and name.
    windows: HashMap<u32, (Cell, String)>,
    /// Windows whose tab is named.
    named: HashMap<u32, String>,
    conns: HashMap<u64, TcpStream>,
    listing: bool,
    list_again: bool,
    /// What a new session runs: this program, with the socket's address and
    /// token.
    bridge: (String, String, String),
    /// The folder for a new session: tmux's when it is on this machine.
    local: bool,
}

impl Gateway {
    fn send(&mut self, cmd: &str, want: Want) {
        if self.to_tmux.write_all(format!("{cmd}\n").as_bytes()).and_then(|()| self.to_tmux.flush()).is_ok() {
            self.pending.push_back(want);
        }
    }

    fn list(&mut self) {
        if self.listing {
            self.list_again = true;
        } else {
            self.listing = true;
            self.send(LIST, Want::Panes);
        }
    }

    fn tell(&mut self, conn: u64, kind: u8, payload: &[u8]) {
        let ok = self.conns.get_mut(&conn).is_some_and(|s| s.write_all(&frame(kind, payload)).is_ok());
        if !ok {
            self.conns.remove(&conn);
        }
    }

    fn line(&mut self, text: Vec<u8>) -> Option<String> {
        if let Some((mine, lines)) = &mut self.reply {
            match line(&text) {
                Line::End { failed, .. } => {
                    let (mine, lines) = (*mine, std::mem::take(lines));
                    self.reply = None;
                    if mine {
                        let want = self.pending.pop_front().unwrap_or(Want::Nothing);
                        self.answer(want, !failed, lines);
                    }
                }
                _ => lines.push(text),
            }
            return None;
        }
        match line(&text) {
            Line::Begin { mine, .. } => self.reply = Some((mine, Vec::new())),
            Line::Output { pane, bytes } => {
                if let Some(conn) = self.panes.get(&pane).filter(|p| p.ready).and_then(|p| p.conn) {
                    self.tell(conn, b'o', &bytes);
                }
            }
            Line::Changed => self.list(),
            Line::Renamed { window, name } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    w.1 = name.clone();
                }
                let conns: Vec<u64> = self.panes.values().filter(|p| p.window == window).filter_map(|p| p.conn).collect();
                for c in conns {
                    self.tell(c, b't', name.as_bytes());
                }
            }
            Line::Exit(why) => return Some(if why.trim().is_empty() { "tmux let go (detached, or its server ended)".into() } else { format!("tmux: {}", why.trim()) }),
            Line::End { .. } | Line::Other => {}
        }
        None
    }

    fn answer(&mut self, want: Want, ok: bool, lines: Vec<Vec<u8>>) {
        match want {
            Want::Nothing => {}
            Want::Panes => {
                self.listing = false;
                if ok {
                    self.panes_are(lines.iter().filter_map(|l| listed(l)).collect());
                }
                if std::mem::take(&mut self.list_again) {
                    self.list();
                }
            }
            Want::Cursor(id) => {
                if let (Some(p), Some(c)) = (self.panes.get_mut(&id), lines.first().and_then(|l| cursor(l))) {
                    p.cursor = c;
                }
            }
            Want::Capture(id) => {
                let Some(p) = self.panes.get_mut(&id) else { return };
                p.ready = true;
                let (conn, shot, window) = (p.conn, snapshot(p.cursor, &lines), p.window);
                if let Some(conn) = conn {
                    let name = self.windows.get(&window).map(|w| w.1.clone()).unwrap_or_default();
                    self.tell(conn, b't', name.as_bytes());
                    self.tell(conn, b'o', &shot);
                }
            }
        }
    }

    /// The panes tmux has now: new ones get a session, gone ones end.
    fn panes_are(&mut self, list: Vec<Listed>) {
        self.windows.clear();
        for l in &list {
            if let Some(cell) = layout(&l.layout) {
                self.windows.insert(l.window, (cell, l.name.clone()));
            }
        }
        let gone: Vec<u32> = self.panes.keys().filter(|id| !list.iter().any(|l| l.pane == **id)).copied().collect();
        for id in gone {
            if let Some(conn) = self.panes.remove(&id).and_then(|p| p.conn) {
                self.tell(conn, b'x', b"the pane closed in tmux");
                self.conns.remove(&conn);
            }
        }
        // By window, in the layout's order, so a pane can split one that is
        // already there.
        let mut order: Vec<&Listed> = list.iter().collect();
        order.sort_by_key(|l| (l.window, self.windows.get(&l.window).and_then(|w| w.0.panes().iter().position(|p| *p == l.pane))));
        for l in order {
            if let Some(p) = self.panes.get_mut(&l.pane) {
                p.size = l.size;
                p.window = l.window;
                continue;
            }
            let session = self.start(l);
            let pane = Pane { window: l.window, size: l.size, session, conn: None, ready: false, let_go: false, want: None, cursor: Cursor::default() };
            self.panes.insert(l.pane, pane);
        }
    }

    /// A session for the pane `l`, running a bridge to it.
    fn start(&self, l: &Listed) -> Option<SessionId> {
        let there = |id: u32| self.panes.get(&id).is_some_and(|p| p.session.is_some() && !p.let_go);
        let place = match self.windows.get(&l.window).and_then(|w| place(&w.0, l.pane, there)) {
            Some((beside, dir)) => Place::Split { beside: self.panes[&beside].session?, dir },
            None => Place::NewWorkspace,
        };
        let (exe, addr, token) = &self.bridge;
        let id = format!("%{}", l.pane);
        let shell = tsumugi_mux::server::own_command(exe, &["tmux-pane", addr, token, &id], cfg!(windows));
        let here = std::env::current_dir().unwrap_or_default();
        let cwd = if self.local && std::path::Path::new(&l.path).is_dir() { l.path.clone().into() } else { here };
        let size = ito_pane::Size::new(usize::from(l.size.0.max(2)), usize::from(l.size.1.max(2)));
        match self.client.spawn_at(cwd, Some(shell), size, (8, 16), place) {
            Ok(p) => Some(p.id()),
            Err(e) => {
                eprintln!("tsumugi tmux: a session for {id}: {e}");
                None
            }
        }
    }

    fn hello(&mut self, conn: u64, pane: u32, to: TcpStream) {
        let Some(p) = self.panes.get_mut(&pane).filter(|p| p.conn.is_none() && !p.let_go) else { return };
        p.conn = Some(conn);
        self.conns.insert(conn, to);
        self.send(&format!("display -p -t %{pane} {CURSOR}"), Want::Cursor(pane));
        self.send(&format!("capture-pane -p -e -t %{pane}"), Want::Capture(pane));
    }

    fn pane_of(&self, conn: u64) -> Option<u32> {
        self.panes.iter().find(|(_, p)| p.conn == Some(conn)).map(|(id, _)| *id)
    }

    /// Make the window of `pane` the size its panes have in tsumugi.
    fn fit(&mut self, pane: u32) {
        let Some(window) = self.panes.get(&pane).map(|p| p.window) else { return };
        let Some((cell, _)) = self.windows.get(&window) else { return };
        let want = |id: u32| self.panes.get(&id).and_then(|p| p.want);
        let (w, h) = cell.fit(&want);
        let mut cmds = vec![format!("refresh-client -C @{window}:{w}x{h}")];
        if !matches!(cell, Cell::Pane(..)) {
            for id in cell.panes() {
                if let Some((c, r)) = want(id) {
                    cmds.push(format!("resize-pane -t %{id} -x {c} -y {r}"));
                }
            }
        }
        for c in cmds {
            self.send(&c, Want::Nothing);
        }
    }

    /// Name each window's tab after it, once the server says which tab its
    /// first pane is in.
    fn name_tabs(&mut self) {
        let mut todo = Vec::new();
        for (window, (cell, name)) in &self.windows {
            if self.named.get(window) == Some(name) {
                continue;
            }
            let first = cell.panes().into_iter().find_map(|id| self.panes.get(&id).and_then(|p| p.session));
            if let Some(s) = first {
                todo.push((*window, s, name.clone()));
            }
        }
        if todo.is_empty() {
            return;
        }
        let tabs = self.client.workspaces();
        for (window, s, name) in todo {
            if let Some(tab) = tabs.iter().find(|t| t.layout.contains(&s)) {
                self.client.rename_workspace(tab.id, format!("tmux: {name}"));
                self.named.insert(window, name);
            }
        }
    }

    /// Tell every bridge why it ends.
    fn end(&mut self, why: &str) {
        let conns: Vec<u64> = self.conns.keys().copied().collect();
        for c in conns {
            self.tell(c, b'x', why.as_bytes());
        }
    }
}

/// Run `tmux -u -C` here or over ssh, its stdin and stdout piped.
fn spawn_tmux(o: &Opts) -> Result<std::process::Child, String> {
    use std::process::{Command, Stdio};
    let line = command(o.target.as_deref());
    let mut cmd = match &o.ssh {
        Some(host) => {
            let ssh = std::env::var_os("TSUMUGI_SSH").filter(|s| !s.is_empty()).unwrap_or_else(|| "ssh".into());
            let mut c = Command::new(ssh);
            c.args(["-T", "--", host, &line]);
            c
        }
        None if cfg!(windows) => return Err("tmux runs on Unix: name a machine with --ssh HOST".into()),
        None => {
            let mut c = Command::new("sh");
            c.args(["-c", &line]);
            c
        }
    };
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
    cmd.spawn().map_err(|e| format!("could not run {}: {e}", if o.ssh.is_some() { "ssh" } else { "tmux" }))
}

/// The command: `args` is what came after `tmux`.
pub fn run(args: &[String]) -> ExitCode {
    let o = match parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("tsumugi tmux: {e}\nusage: {USAGE}");
            return ExitCode::from(2);
        }
    };
    match gateway(&o) {
        Ok(why) => {
            println!("tsumugi tmux: {why}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tsumugi tmux: {e}");
            ExitCode::FAILURE
        }
    }
}

fn gateway(o: &Opts) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("this program's path: {e}"))?;
    let client = crate::connect(|| {})?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("a socket for the panes: {e}"))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?.to_string();
    let token = token();
    let mut child = spawn_tmux(o)?;
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill();
        return Err("tmux gave no pipes".into());
    };
    let (tx, rx) = mpsc::channel();
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match r.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        while buf.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
                            buf.pop();
                        }
                        if tx.send(Ev::Line(buf.clone())).is_err() {
                            return;
                        }
                    }
                }
            }
            let _ = tx.send(Ev::Eof);
        });
    }
    {
        let tx = tx.clone();
        let token = token.clone();
        std::thread::spawn(move || {
            for (conn, s) in listener.incoming().enumerate() {
                let Ok(s) = s else { continue };
                let (tx, token) = (tx.clone(), token.clone());
                std::thread::spawn(move || bridge_reader(conn as u64, s, &token, &tx));
            }
        });
    }
    let mut g = Gateway {
        client,
        to_tmux: Box::new(stdin),
        pending: VecDeque::new(),
        reply: None,
        panes: HashMap::new(),
        windows: HashMap::new(),
        named: HashMap::new(),
        conns: HashMap::new(),
        listing: false,
        list_again: false,
        bridge: (exe.to_string_lossy().into_owned(), addr, token),
        local: o.ssh.is_none(),
    };
    g.list();
    let at = o.ssh.as_deref().map(|h| format!(" on {h}")).unwrap_or_default();
    println!("tsumugi tmux: tmux{at} in control mode; its panes open as tsumugi's. Ctrl+C here (or `tmux detach` in a pane) leaves them running in tmux.");
    let why = loop {
        let ev = match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(ev) => ev,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                g.name_tabs();
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break "tmux went".to_owned(),
        };
        match ev {
            Ev::Line(l) => {
                if let Some(why) = g.line(l) {
                    break why;
                }
            }
            Ev::Eof => break "tmux went".into(),
            Ev::Hello { conn, pane, to } => g.hello(conn, pane, to),
            Ev::Keys(conn, bytes) => {
                if let Some(pane) = g.pane_of(conn) {
                    for c in keys(pane, &bytes) {
                        g.send(&c, Want::Nothing);
                    }
                }
            }
            Ev::Size(conn, size) => {
                if let Some(pane) = g.pane_of(conn) {
                    let p = g.panes.get_mut(&pane).expect("found");
                    if p.want != Some(size) {
                        p.want = Some(size);
                        g.fit(pane);
                    }
                }
            }
            Ev::Bye(conn) => {
                g.conns.remove(&conn);
                if let Some(pane) = g.pane_of(conn) {
                    let p = g.panes.get_mut(&pane).expect("found");
                    p.conn = None;
                    p.let_go = true;
                    p.ready = false;
                }
            }
        }
    };
    g.end(&why);
    // tmux's control client detaches when its input ends.
    g.to_tmux = Box::new(std::io::sink());
    let _ = child.wait();
    Ok(why)
}

/// A bridge's socket: its hello, then its keys and sizes, until it goes.
fn bridge_reader(conn: u64, s: TcpStream, token: &str, tx: &mpsc::Sender<Ev>) {
    let Ok(to) = s.try_clone() else { return };
    let mut r = BufReader::new(s);
    let pane = match read_frame(&mut r) {
        Ok((b'h', said)) => {
            let said = String::from_utf8_lossy(&said).into_owned();
            match said.split_once(' ') {
                Some((t, p)) if t == token => p.strip_prefix('%').and_then(|n| n.parse().ok()),
                _ => None,
            }
        }
        _ => None,
    };
    let Some(pane) = pane else { return };
    if tx.send(Ev::Hello { conn, pane, to }).is_err() {
        return;
    }
    loop {
        let ev = match read_frame(&mut r) {
            Ok((b'k', keys)) => Ev::Keys(conn, keys),
            Ok((b's', size)) => {
                let size = String::from_utf8_lossy(&size).into_owned();
                let n: Vec<u16> = size.split_whitespace().filter_map(|w| w.parse().ok()).collect();
                match n[..] {
                    [c, r] if c > 0 && r > 0 => Ev::Size(conn, (c, r)),
                    _ => continue,
                }
            }
            Ok(_) => continue,
            Err(_) => break,
        };
        if tx.send(ev).is_err() {
            return;
        }
    }
    let _ = tx.send(Ev::Bye(conn));
}

/// `tsumugi tmux-pane ADDR TOKEN %N`: a pane's end of the gateway (see the
/// top), run in the session the gateway started for it.
pub fn bridge(args: &[String]) -> ExitCode {
    let [addr, token, pane] = args else {
        eprintln!("tsumugi tmux-pane: run by `tsumugi tmux`, not by hand");
        return ExitCode::from(2);
    };
    let (mut con_in, mut con_out, guard) = match crate::console::raw() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("tsumugi tmux-pane: the console: {e}");
            return ExitCode::FAILURE;
        }
    };
    let say = |out: &mut Box<dyn Write + Send>, text: &str| {
        let _ = out.write_all(format!("\r\n[tsumugi tmux: {text}]\r\n").as_bytes());
        let _ = out.flush();
    };
    let stream = match TcpStream::connect(addr.as_str()) {
        Ok(s) => s,
        Err(e) => {
            say(&mut con_out, &format!("the gateway at {addr}: {e}"));
            drop(guard);
            return ExitCode::FAILURE;
        }
    };
    let _ = stream.set_nodelay(true);
    let Ok(mut from) = stream.try_clone() else { return ExitCode::FAILURE };
    let to = Arc::new(Mutex::new(stream));
    let send = |to: &Mutex<TcpStream>, kind: u8, payload: &[u8]| to.lock().map(|mut s| s.write_all(&frame(kind, payload)).is_ok()).unwrap_or(false);
    if !send(&to, b'h', format!("{token} {pane}").as_bytes()) {
        drop(guard);
        return ExitCode::FAILURE;
    }
    let (done_tx, done_rx) = mpsc::channel::<()>();
    {
        let done = done_tx.clone();
        std::thread::spawn(move || {
            let why = loop {
                match read_frame(&mut from) {
                    Ok((b'o', bytes)) => {
                        if con_out.write_all(&bytes).and_then(|()| con_out.flush()).is_err() {
                            break None;
                        }
                    }
                    Ok((b't', title)) => {
                        let title: String = String::from_utf8_lossy(&title).chars().filter(|c| !c.is_control()).collect();
                        let _ = con_out.write_all(format!("\x1b]2;tmux: {title}\x07").as_bytes());
                        let _ = con_out.flush();
                    }
                    Ok((b'x', why)) => break Some(String::from_utf8_lossy(&why).into_owned()),
                    Ok(_) => {}
                    Err(_) => break Some("the gateway went".into()),
                }
            };
            if let Some(why) = why {
                say(&mut con_out, &why);
            }
            let _ = done.send(());
        });
    }
    {
        let (to, done) = (to.clone(), done_tx);
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match con_in.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if !send(&to, b'k', &buf[..n]) {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            let _ = done.send(());
        });
    }
    // The size, when it changes: tsumugi resizes the console, and tmux
    // follows.
    let mut last = None;
    loop {
        let now = crate::console::size();
        if let Some((c, r)) = now.filter(|_| now != last) {
            last = now;
            if !send(&to, b's', format!("{c} {r}").as_bytes()) {
                break;
            }
        }
        match done_rx.recv_timeout(Duration::from_millis(250)) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            _ => break,
        }
    }
    drop(guard);
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options() {
        assert_eq!(parse(&[]).unwrap(), Opts::default());
        let o = parse(&["--ssh".into(), "box".into(), "-t".into(), "work".into()]).unwrap();
        assert_eq!((o.ssh.as_deref(), o.target.as_deref()), (Some("box"), Some("work")));
        assert!(parse(&["--ssh".into()]).is_err());
        assert!(parse(&["-x".into()]).is_err());
    }

    #[test]
    fn the_command_tmux_runs() {
        assert_eq!(command(Some("it's")), r"exec tmux -u -C new-session -A -s 'it'\''s'");
        assert!(command(None).contains("tmux -u -C attach || exec tmux -u -C new-session"));
    }

    #[test]
    fn output_is_unescaped() {
        assert_eq!(unescape(br"hi\015\012\134x\033[0m"), b"hi\r\n\\x\x1b[0m");
        // Not three octal digits: as it is.
        assert_eq!(unescape(br"a\9b\01"), br"a\9b\01");
        assert_eq!(unescape("日本".as_bytes()), "日本".as_bytes());
    }

    #[test]
    fn a_layout_is_read() {
        assert_eq!(layout("b25d,80x24,0,0,0"), Some(Cell::Pane(0, (80, 24))));
        let l = layout("f91d,120x40,0,0{60x40,0,0,0,59x40,61,0[59x20,61,0,1,59x19,61,21,2]}").unwrap();
        assert_eq!(l.panes(), [0, 1, 2]);
        assert_eq!(l, Cell::Row(vec![Cell::Pane(0, (60, 40)), Cell::Col(vec![Cell::Pane(1, (59, 20)), Cell::Pane(2, (59, 19))], (59, 40))], (120, 40)));
        assert_eq!(layout("x,80x24,0,0{"), None);
        assert_eq!(layout("x,80x24,0,0,1junk"), None);
    }

    #[test]
    fn a_window_fits_its_panes() {
        let l = layout("f91d,120x40,0,0{60x40,0,0,0,59x40,61,0[59x20,61,0,1,59x19,61,21,2]}").unwrap();
        assert_eq!(l.fit(&|_| None), (120, 40));
        // The left pane wider and taller: the window with it.
        assert_eq!(l.fit(&|id| (id == 0).then_some((70, 50))), (130, 50));
        assert_eq!(l.fit(&|id| (id == 2).then_some((59, 30))), (120, 51));
    }

    #[test]
    fn a_pane_goes_beside_its_neighbour() {
        let l = layout("f91d,120x40,0,0{60x40,0,0,0,59x40,61,0[59x20,61,0,1,59x19,61,21,2]}").unwrap();
        assert_eq!(place(&l, 0, |_| false), None);
        assert_eq!(place(&l, 1, |id| id == 0), Some((0, Dir::Right)));
        assert_eq!(place(&l, 2, |id| id < 2), Some((1, Dir::Down)));
        // Before it there is none: the one after.
        assert_eq!(place(&l, 0, |id| id == 2), Some((2, Dir::Right)));
        assert_eq!(place(&l, 9, |_| true), None);
    }

    #[test]
    fn lines_are_read() {
        assert_eq!(line(b"%begin 1791698267 270 1"), Line::Begin { number: 270, mine: true });
        assert_eq!(line(b"%begin 1791698267 264 0"), Line::Begin { number: 264, mine: false });
        assert_eq!(line(b"%end 1791698267 270 1"), Line::End { number: 270, failed: false });
        assert_eq!(line(b"%error 1791698267 271 1"), Line::End { number: 271, failed: true });
        assert_eq!(line(br"%output %3 ls\015\012"), Line::Output { pane: 3, bytes: b"ls\r\n".to_vec() });
        assert_eq!(line(b"%output %3 "), Line::Output { pane: 3, bytes: Vec::new() });
        assert_eq!(line(b"%layout-change @1 f91d,80x24,0,0,0 f91d,80x24,0,0,0 *"), Line::Changed);
        assert_eq!(line(b"%window-renamed @2 my editor"), Line::Renamed { window: 2, name: "my editor".into() });
        assert_eq!(line(b"%exit"), Line::Exit(String::new()));
        assert_eq!(line(b"%exit server exited"), Line::Exit("server exited".into()));
        assert_eq!(line(b"%sessions-changed"), Line::Other);
        assert_eq!(line(b"anything"), Line::Other);
    }

    #[test]
    fn a_listed_pane_and_its_cursor() {
        let l = listed(b"%3\t@1\t80\t24\tb25d,80x24,0,0,3\tbash\t/home/me/a\tb").unwrap();
        assert_eq!((l.pane, l.window, l.size, l.path.as_str()), (3, 1, (80, 24), "/home/me/a\tb"));
        assert_eq!(l.name, "bash");
        assert_eq!(listed(b"%3\t@1\t80"), None);
        assert_eq!(cursor(b"12 1 0 1 0"), Some(Cursor { x: 12, y: 1, alternate: false, shown: true, keypad: false }));
        assert_eq!(cursor(b"12 1 0"), None);
    }

    #[test]
    fn a_snapshot_puts_the_screen_and_the_cursor_back() {
        let c = Cursor { x: 2, y: 1, alternate: true, shown: false, keypad: false };
        let s = snapshot(c, &[b"a".to_vec(), b"bc".to_vec()]);
        assert_eq!(s, b"\x1b[?1049h\x1b[H\x1b[2Ja\x1b[0m\r\nbc\x1b[0m\x1b[2;3H\x1b[?25l");
    }

    #[test]
    fn keys_go_as_hex() {
        assert_eq!(keys(4, b"ls\r"), ["send-keys -t %4 -H 6c 73 0d"]);
        assert_eq!(keys(4, &[b'a'; 300]).len(), 2);
    }

    #[test]
    fn frames_go_and_come_back() {
        let mut bytes = frame(b'o', b"hello");
        bytes.extend(frame(b'x', b""));
        let mut r = &bytes[..];
        assert_eq!(read_frame(&mut r).unwrap(), (b'o', b"hello".to_vec()));
        assert_eq!(read_frame(&mut r).unwrap(), (b'x', Vec::new()));
        assert!(read_frame(&mut r).is_err());
        assert_ne!(token(), token());
    }
}
