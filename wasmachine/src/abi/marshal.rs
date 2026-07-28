//! The pointer boundary: every `(ptr, len)` argument and every out-pointer the
//! engine's half of the guest ABI takes, wrapped so that **no module outside
//! `abi` ever writes a pointer expression**.
//!
//! Callers hand over `&str`, `&[u8]`, or take back a `String`; the raw addresses
//! are formed and consumed here. The imports themselves live in `abi::sys`
//! (`wasm.rs` on wasm, `stubs.rs` on the host target); this layer sits directly
//! on top of them and nowhere else.
//!
//! Two-call reads (`*_len`, then fill a buffer) are race-free because nothing
//! here parks, so no other task can run between the calls. A **zero length
//! returns early** without a second call: an empty `Vec`'s `as_mut_ptr()` is a
//! dangling (though aligned) address, and there is no reason to hand the host a
//! pointer it must not dereference.
//!
//! [`read_string`] is `pub` for the same reason the module is: a plugin SDK's
//! own marshalling layer runs the identical two-call protocol against its own
//! imports, and one implementation of it is better than two.

use super::sys;

// --- Diagnostics. ---

pub fn log(msg: &str) {
    unsafe { sys::log(msg.as_ptr(), msg.len()) }
}

/// Kill the animation with a message. Never returns.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn fail(msg: &str) -> ! {
    unsafe { sys::fail(msg.as_ptr(), msg.len()) }
}

// --- Two-call string reads. ---

/// How many bytes a two-call read needs, or `None` when there is nothing to
/// read and the second host call must be skipped entirely.
///
/// A negative length is the host contradicting the ABI, which is a kill.
fn read_len(len: i32, what: &str) -> Option<usize> {
    match len {
        0 => None,
        n if n > 0 => Some(n as usize),
        n => panic!("host returned a negative {what} length: {n}"),
    }
}

/// Run the two-call protocol: ask for the length, then let `fill` write exactly
/// that many bytes into a fresh buffer.
///
/// **SDK-internal**: `len` is what a `get_*_len` import just returned, and
/// `fill` must call the matching `get_*` import with the buffer it is handed.
pub fn read_string(len: i32, what: &str, fill: impl FnOnce(*mut u8)) -> String {
    let Some(len) = read_len(len, what) else {
        return String::new();
    };
    let mut buf = vec![0u8; len];
    fill(buf.as_mut_ptr());
    String::from_utf8(buf).unwrap_or_else(|_| panic!("host returned a non-UTF-8 {what}"))
}

// --- Channels: payload bytes in and out. ---

pub fn channel_send(id: i32, bytes: &[u8]) {
    unsafe { sys::channel_send(id, bytes.as_ptr(), bytes.len()) }
}

pub fn channel_recv(id: i32, buf: &mut [u8]) {
    unsafe { sys::channel_recv(id, buf.as_mut_ptr()) }
}

pub fn channel_peek(id: i32, buf: &mut [u8]) {
    unsafe { sys::channel_peek(id, buf.as_mut_ptr()) }
}

// --- The math kernel's one pointer-taking entry. ---

/// Format `x` host-side and take the text back as a `String`.
///
/// `precision` is `-1` for the shortest round-trip form or `0..=17` fixed
/// decimals; anything else is API misuse and kills. The host reports the exact
/// byte count and writes as much of it as fits, so a first buffer that turns out
/// short costs one extra call and never a wrong answer — and since nothing parks
/// between the two calls, the retry cannot race another task.
pub fn format_f64(x: f64, precision: i32) -> String {
    // One stack buffer covers every ordinary number; only the extremes
    // (`1e300` needs 302 bytes, `{:.17}` of a large value more) retry.
    const INLINE: usize = 32;
    let mut inline = [0u8; INLINE];
    let needed =
        count(unsafe { sys::format_f64(x, precision, inline.as_mut_ptr(), INLINE as i32) });
    if needed <= INLINE {
        return decode(&inline[..needed]);
    }
    let mut heap = vec![0u8; needed];
    let again = count(unsafe {
        sys::format_f64(
            x,
            precision,
            heap.as_mut_ptr(),
            i32::try_from(needed).expect("formatted f64 longer than i32::MAX bytes"),
        )
    });
    assert!(
        again == needed,
        "host asked for {needed} bytes to format a f64, then wanted {again}"
    );
    decode(&heap)
}

/// A byte count the host just reported. Negative is the host contradicting the
/// ABI, which is a kill.
fn count(n: i32) -> usize {
    usize::try_from(n).unwrap_or_else(|_| panic!("host returned a negative format length: {n}"))
}

fn decode(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("host returned a non-UTF-8 formatted number")
}

// --- The allocator's one import. Only the global allocator calls this, and it
// is inherently pointer-shaped, so it stays raw — but it stays *here*. ---

#[cfg(target_arch = "wasm32")]
pub unsafe fn realloc(ptr: *mut u8, old_size: usize, align: usize, new_size: usize) -> *mut u8 {
    unsafe { sys::realloc(ptr, old_size, align, new_size) }
}

#[cfg(test)]
mod tests {
    use super::read_len;

    /// A zero-length read must report "nothing to read" so the caller skips the
    /// second host call: handing the host an empty `Vec`'s dangling pointer is
    /// pointless at best.
    #[test]
    fn a_zero_length_read_skips_the_second_call() {
        assert_eq!(read_len(0, "text"), None);
    }

    #[test]
    fn a_positive_length_asks_for_that_many_bytes() {
        assert_eq!(read_len(5, "text"), Some(5));
        assert_eq!(read_len(i32::MAX, "text"), Some(i32::MAX as usize));
    }

    #[test]
    #[should_panic(expected = "negative text length: -1")]
    fn a_negative_length_is_the_host_breaking_the_abi() {
        let _ = read_len(-1, "text");
    }
}
