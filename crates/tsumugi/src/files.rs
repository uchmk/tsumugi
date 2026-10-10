//! Reading and writing a person's files without losing them (the source
//! review, 2026-10-07): a file that is there but cannot be read is an
//! error, never an empty file to write over; and a file is replaced at once,
//! never left half written.

// The same two are what every uchmk app writes its settings with, so they
// live in `tsumugi-common`.
pub use tsumugi_common::{read_or_empty, write_atomic};

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
        assert!(!dir.join("a").join("f.txt.uchmk-new").exists(), "nothing left beside it");
        // UTF-16 (with its mark), as Windows PowerShell 5.1 saves a profile.
        std::fs::write(&p, [0xFF, 0xFE, b'a', 0, b'b', 0]).unwrap();
        assert!(read_or_empty(&p).is_err(), "not text: refused, not taken as empty");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
