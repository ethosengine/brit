//! Match with the system POSIX regex implementation, as stock Git does on these platforms.
#![expect(unsafe_code, reason = "POSIX regular expressions require libc FFI on these targets")]

use std::{ffi::CString, mem::MaybeUninit};

use gix::{
    Result,
    error::{ResultExt, bail, message},
};

pub(super) struct Regex(Box<libc::regex_t>);

impl Regex {
    pub(super) fn new(pattern: &[u8], extended: bool, ignore_case: bool) -> Result<Self> {
        let pattern = CString::new(pattern).or_raise(|| message("NUL in grep pattern").validation())?;
        let mut compiled = Box::new(MaybeUninit::<libc::regex_t>::zeroed());
        let flags = libc::REG_NOSUB
            | if extended { libc::REG_EXTENDED } else { 0 }
            | if ignore_case { libc::REG_ICASE } else { 0 };
        // SAFETY: Both pointers are valid for regcomp, and its input is NUL terminated.
        let status = unsafe { libc::regcomp(compiled.as_mut_ptr(), pattern.as_ptr(), flags) };
        if status != 0 {
            let mut diagnostic = [0u8; 256];
            // SAFETY: A null regex is permitted by regerror. The output buffer has the supplied length.
            unsafe {
                libc::regerror(
                    status,
                    std::ptr::null(),
                    diagnostic.as_mut_ptr().cast(),
                    diagnostic.len(),
                );
            }
            let end = diagnostic
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(diagnostic.len());
            bail!(message!("Invalid grep pattern: {}", String::from_utf8_lossy(&diagnostic[..end])).validation());
        }
        // SAFETY: regcomp initialized the expression on success.
        Ok(Self(unsafe { compiled.assume_init() }))
    }

    pub(super) fn is_match(&self, line: &[u8]) -> Result<bool> {
        let end = libc::regoff_t::try_from(line.len()).or_raise(|| message("Grep line is too long"))?;
        let mut range = libc::regmatch_t { rm_so: 0, rm_eo: end };
        let data = if line.is_empty() { b"\0" } else { line };
        // SAFETY: The expression has been compiled, and REG_STARTEND bounds the input read
        // to the range provided by the single valid match slot.
        match unsafe { libc::regexec(&*self.0, data.as_ptr().cast(), 1, &mut range, libc::REG_STARTEND) } {
            0 => Ok(true),
            libc::REG_NOMATCH => Ok(false),
            code => bail!(message!("Grep regex matching failed (POSIX error {code})")),
        }
    }
}

impl Drop for Regex {
    fn drop(&mut self) {
        // SAFETY: The expression was successfully compiled and owns its allocation until drop.
        unsafe { libc::regfree(&mut *self.0) };
    }
}

// Keep the POSIX locale scoped to the CLI thread doing the search.
#[cfg(not(target_os = "netbsd"))]
pub(super) fn use_environment_locale() -> Option<Locale> {
    // SAFETY: The empty C string requests the process environment's locale. The null base
    // asks newlocale for a new object owned by this call.
    let active = unsafe {
        libc::newlocale(
            libc::LC_CTYPE_MASK | libc::LC_COLLATE_MASK,
            c"".as_ptr(),
            std::ptr::null_mut(),
        )
    };
    if active.is_null() {
        return None;
    }
    // SAFETY: active is a live locale. The returned previous locale stays live throughout this scope.
    let previous = unsafe { libc::uselocale(active) };
    if previous.is_null() {
        // SAFETY: The locale could not be installed and is owned only by this call.
        unsafe { libc::freelocale(active) };
        return None;
    }
    Some(Locale { active, previous })
}

#[cfg(not(target_os = "netbsd"))]
pub(super) struct Locale {
    active: libc::locale_t,
    previous: libc::locale_t,
}

#[cfg(not(target_os = "netbsd"))]
impl Drop for Locale {
    fn drop(&mut self) {
        // SAFETY: Restore the previous live locale before freeing the locale owned by this guard.
        unsafe {
            libc::uselocale(self.previous);
            libc::freelocale(self.active);
        }
    }
}

#[cfg(target_os = "netbsd")]
pub(super) struct Locale;

#[cfg(target_os = "netbsd")]
pub(super) fn use_environment_locale() -> Locale {
    Locale
}
