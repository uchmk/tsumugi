//! macOS's Secure Keyboard Entry (Terminal's and iTerm2's): while it is on,
//! other programs cannot read the keys typed, a key logger nor an event tap.
//! `[advanced] secure_input` asks for it; it is held only while the window
//! is in front, since the system turns it on for everyone while any program
//! holds it. Elsewhere there is nothing to do.

/// Whether we hold it now: the calls must pair, one off for each on.
#[derive(Default)]
pub struct Secure {
    on: bool,
}

impl Secure {
    /// Once a frame: hold it while `want` (asked for and the window in
    /// front), let go otherwise.
    pub fn follow(&mut self, want: bool) {
        if want != self.on {
            self.on = want;
            imp::set(want);
        }
    }
}

impl Drop for Secure {
    fn drop(&mut self) {
        self.follow(false);
    }
}

/// Whether this system has it (the palette and the settings show it only
/// there).
pub const HERE: bool = cfg!(target_os = "macos");

#[cfg(target_os = "macos")]
mod imp {
    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn EnableSecureEventInput() -> i32;
        fn DisableSecureEventInput() -> i32;
    }

    pub fn set(on: bool) {
        // SAFETY: no arguments; the system counts the calls, which `Secure`
        // keeps paired.
        unsafe {
            if on {
                EnableSecureEventInput();
            } else {
                DisableSecureEventInput();
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn set(_on: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_without_calling_twice() {
        let mut s = Secure::default();
        s.follow(true);
        s.follow(true);
        assert!(s.on);
        s.follow(false);
        assert!(!s.on);
    }
}
