//! Reading and writing a person's files without losing them (the source
//! review, 2026-10-07): a file that is there but cannot be read is an
//! error, never an empty file to write over; and a file is replaced at once,
//! never left half written.

use std::path::Path;

/// The file's text; empty when there is no such file. Any other failure --
/// no permission, another program holding it, not UTF-8 (a PowerShell 5.1
/// profile saved as UTF-16) -- is an error, so nothing gets written over it.
pub fn read_or_empty(path: &Path) -> Result<String, String> {
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| format!("{} is not UTF-8 text (saved as UTF-16?), so it is left as it is", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Write `bytes` to `path` all at once: beside it first, to the disk, then
/// over it. A crash or a full disk leaves the old file whole.
pub fn write_atomic(path: &Path, bytes: impl AsRef<[u8]>) -> Result<(), String> {
    use std::io::Write;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let mut aside = path.as_os_str().to_owned();
    aside.push(".tsumugi-new");
    let aside = std::path::PathBuf::from(aside);
    let written = (|| {
        let mut f = std::fs::File::create(&aside)?;
        f.write_all(bytes.as_ref())?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&aside, path)
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&aside);
        return Err(format!("{}: {e}", path.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_read_or_refused_and_replaced_whole() {
        let dir = std::env::temp_dir().join(format!("tsumugi-files-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("a").join("f.txt");
        assert_eq!(read_or_empty(&p), Ok(String::new()), "none: empty");
        write_atomic(&p, "one").unwrap();
        assert_eq!(read_or_empty(&p).as_deref(), Ok("one"));
        write_atomic(&p, "two").unwrap();
        assert_eq!(read_or_empty(&p).as_deref(), Ok("two"));
        assert!(!dir.join("a").join("f.txt.tsumugi-new").exists(), "nothing left beside it");
        // UTF-16 (with its mark), as Windows PowerShell 5.1 saves a profile.
        std::fs::write(&p, [0xFF, 0xFE, b'a', 0, b'b', 0]).unwrap();
        assert!(read_or_empty(&p).is_err(), "not text: refused, not taken as empty");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
