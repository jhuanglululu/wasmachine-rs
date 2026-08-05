//! The animation's environment: an immutable, ordered set of key/value strings
//! the host hands the guest at startup.
//!
//! Where the pairs come from is the plugin's business (a `/bb env` command,
//! host built-ins, …); what this module guarantees is the shape. The
//! environment is **read-only and fixed for the instance's life** — changing it
//! restarts the animation, so a guest never has to wonder whether a value it
//! read a minute ago is still current. That is why every lookup hands back
//! `&'static str` rather than a borrow of anything.
//!
//! ```ignore
//! let speed: f64 = wasmachine::env::get("speed")
//!     .unwrap_or("1.0")
//!     .parse()
//!     .expect("speed must be a number");
//! for (key, value) in wasmachine::env::iter() {
//!     wasmachine::log(&format!("{key} = {value}"));
//! }
//! ```
//!
//! # Where it lives
//!
//! The blob is fetched and parsed exactly once, in
//! [`__rt::init`](crate::__rt::init) — on task 0, before the user's `main` and
//! therefore before any fork can exist. Both the bytes and the `(key, value)`
//! index go into the engine's **shared static region**, which a fork references
//! instead of copying: every task of the instance reads one copy of every byte.
//! The root pointer is a plain static, so each fork's private copy of it simply
//! points back at the same shared address — which is the point.
//!
//! # Wire format
//!
//! The host serves the whole environment as one blob (`environ_len` /
//! `environ_read`, the same two-call read the rest of the ABI uses). Every
//! integer is a little-endian `u32`: an entry count, then per entry a key
//! length, the key's UTF-8 bytes, a value length, the value's UTF-8 bytes.
//! Entries arrive sorted by raw key bytes, which is what lets [`get`] binary
//! search. No environ at all is a zero-length blob.
//!
//! The host is trusted, so the parser's job is not defence — it is *diagnosis*.
//! A truncated, mis-counted, non-UTF-8, unsorted or duplicate-keyed blob means
//! the two sides disagree about the format, and that fails loudly through the
//! panic hook (host `fail`, which kills the animation with the message) rather
//! than silently serving a half-read environment.

use std::sync::OnceLock;

/// One environment entry: key, then value. Both borrow the blob in the shared
/// region, which is never freed.
type Entry = (&'static str, &'static str);

/// The parsed index, sorted by key. Set once by [`init`]; a fork copies the
/// pointer, not the entries.
static ENTRIES: OnceLock<&'static [Entry]> = OnceLock::new();

/// Fetch, parse and index the environment. Called by
/// [`__rt::init`](crate::__rt::init) before the user's `main`, on task 0.
///
/// Calling it twice is harmless (the second call is ignored) but pointless: the
/// environment cannot change while the instance runs.
pub(crate) fn init() {
    let _ = ENTRIES.set(parse(crate::abi::marshal::environ()));
}

/// The entries, or nothing at all before [`init`] has run.
fn entries() -> &'static [Entry] {
    ENTRIES.get().copied().unwrap_or(&[])
}

/// The value set for `key`, or `None` if there is none.
///
/// A plain binary search over memory that is already parsed — no host call, no
/// allocation. The result is `&'static str` because the environment is fixed
/// for the instance's life: keep it, share it across tasks, store it in a
/// struct.
pub fn get(key: &str) -> Option<&'static str> {
    let entries = entries();
    entries
        .binary_search_by(|(k, _)| k.as_bytes().cmp(key.as_bytes()))
        .ok()
        .map(|i| entries[i].1)
}

/// Every entry, in key order (sorted by raw key bytes).
///
/// ```ignore
/// let count = wasmachine::env::iter().count();
/// ```
pub fn iter() -> impl Iterator<Item = (&'static str, &'static str)> {
    entries().iter().copied()
}

/// Parse the host's blob into a sorted index, both living in the shared region.
///
/// `blob` must already be shared-region memory (it is: `marshal::environ` reads
/// it straight in there), because the index's `&'static str`s point into it.
///
/// Panics — i.e. kills the animation with a message — on anything that means
/// the two sides disagree about the format; see the module docs.
fn parse(blob: &'static [u8]) -> &'static [Entry] {
    if blob.is_empty() {
        return &[];
    }
    let mut reader = Reader { blob, at: 0 };
    let count = reader.u32("entry count") as usize;
    // Capped: an absurd count from a disagreeing host must fail on the bytes it
    // does not have, not on a reservation.
    let mut entries: Vec<Entry> = Vec::with_capacity(count.min(1024));
    for i in 0..count {
        let key = reader.str(&format!("key of entry {i}"));
        let value = reader.str(&format!("value of entry {i}"));
        if let Some((previous, _)) = entries.last() {
            match previous.as_bytes().cmp(key.as_bytes()) {
                core::cmp::Ordering::Less => {}
                core::cmp::Ordering::Equal => panic!("environ has duplicate key {key:?}"),
                core::cmp::Ordering::Greater => {
                    panic!("environ is not sorted by key: {previous:?} precedes {key:?}")
                }
            }
        }
        entries.push((key, value));
    }
    assert!(
        reader.at == blob.len(),
        "environ blob has {} trailing bytes after {count} entries",
        blob.len() - reader.at
    );
    // The index is built on the private heap and then copied across: it is a
    // handful of fat pointers, and copying them is cheaper than teaching the
    // bump allocator to grow an array in place.
    crate::abi::marshal::shared_copy(&entries)
}

/// A cursor over the blob. Every read either advances or says exactly what it
/// was short of.
struct Reader {
    blob: &'static [u8],
    at: usize,
}

impl Reader {
    fn u32(&mut self, what: &str) -> u32 {
        let end = self.at.wrapping_add(4);
        let bytes = self.blob.get(self.at..end).unwrap_or_else(|| {
            panic!(
                "environ blob ends mid-{what}: need 4 bytes at offset {}, blob is {} bytes",
                self.at,
                self.blob.len()
            )
        });
        self.at = end;
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    fn str(&mut self, what: &str) -> &'static str {
        let len = self.u32(what) as usize;
        // `wrapping_add` (here and above) rather than `+`: a bogus length from a
        // disagreeing host must come back as "the blob is too short", not as an
        // arithmetic overflow — and on wasm32 a `u32` length can wrap `usize`.
        let end = self.at.wrapping_add(len);
        let bytes = self.blob.get(self.at..end).unwrap_or_else(|| {
            panic!(
                "environ blob ends mid-{what}: need {len} bytes at offset {}, blob is {} bytes",
                self.at,
                self.blob.len()
            )
        });
        let text = core::str::from_utf8(bytes)
            .unwrap_or_else(|e| panic!("environ {what} is not UTF-8: {e}"));
        self.at = end;
        text
    }
}

#[cfg(test)]
mod tests {
    use super::{Entry, get, init, iter, parse};

    // Every blob below is written out by hand, byte by byte, so a bug in a
    // serializer could never cancel out against a bug in the parser. `u32`
    // lengths are spelled as their four little-endian bytes. The literals are
    // constants, so they promote to `'static` — which is exactly the lifetime
    // `parse` demands of the real shared-region blob.

    fn lookup(entries: &'static [Entry], key: &str) -> Option<&'static str> {
        entries
            .binary_search_by(|(k, _)| k.as_bytes().cmp(key.as_bytes()))
            .ok()
            .map(|i| entries[i].1)
    }

    #[test]
    fn no_environ_at_all_is_a_zero_length_blob() {
        assert_eq!(parse(&[]), &[] as &[Entry]);
    }

    #[test]
    fn an_empty_environ_is_a_bare_zero_count() {
        let entries = parse(&[0, 0, 0, 0]);
        assert!(entries.is_empty());
        assert_eq!(lookup(entries, "anything"), None);
    }

    #[test]
    fn one_entry_round_trips_key_and_value() {
        // count = 1; key_len = 5, "speed"; value_len = 3, "2.5"
        let entries = parse(&[
            1, 0, 0, 0, //
            5, 0, 0, 0, b's', b'p', b'e', b'e', b'd', //
            3, 0, 0, 0, b'2', b'.', b'5',
        ]);
        assert_eq!(entries, &[("speed", "2.5")]);
        assert_eq!(lookup(entries, "speed"), Some("2.5"));
        // Neighbours of a hit, so the binary search cannot be answering "close
        // enough".
        assert_eq!(lookup(entries, "spee"), None);
        assert_eq!(lookup(entries, "speedy"), None);
    }

    #[test]
    fn several_entries_keep_key_order_and_look_up_one_by_one() {
        // "a" = "1", "bb" = "", "c" = "xyz", "d" = "4"
        let entries = parse(&[
            4, 0, 0, 0, //
            1, 0, 0, 0, b'a', 1, 0, 0, 0, b'1', //
            2, 0, 0, 0, b'b', b'b', 0, 0, 0, 0, //
            1, 0, 0, 0, b'c', 3, 0, 0, 0, b'x', b'y', b'z', //
            1, 0, 0, 0, b'd', 1, 0, 0, 0, b'4',
        ]);
        assert_eq!(entries, &[("a", "1"), ("bb", ""), ("c", "xyz"), ("d", "4")]);
        for (key, value) in entries {
            assert_eq!(lookup(entries, key), Some(*value));
        }
        // Including the empty value, which is set — not absent.
        assert_eq!(lookup(entries, "bb"), Some(""));
        assert_eq!(lookup(entries, "b"), None);
        assert_eq!(lookup(entries, ""), None);
    }

    /// A key may be any UTF-8, and the lengths are *bytes*, not characters —
    /// "héllo" is six bytes for five characters.
    #[test]
    fn keys_and_values_are_utf8_measured_in_bytes() {
        let entries = parse(&[
            2, 0, 0, 0, //
            6, 0, 0, 0, b'h', 0xC3, 0xA9, b'l', b'l', b'o', //
            4, 0, 0, 0, 0xF0, 0x9F, 0x8E, 0xA8, // U+1F3A8, one character
            5, 0, 0, 0, 0xE6, 0xBC, 0xA2, b'x', b'y', //
            0, 0, 0, 0,
        ]);
        assert_eq!(entries, &[("héllo", "🎨"), ("漢xy", "")]);
        assert_eq!(lookup(entries, "héllo"), Some("🎨"));
        // Sorted by *bytes*: the multi-byte key really does come last here.
        assert_eq!(lookup(entries, "漢xy"), Some(""));
    }

    /// `get` binary-searches, so out-of-order entries would make lookups lie.
    /// The host promises sorted order; a violation is a format disagreement and
    /// is reported rather than quietly re-sorted.
    #[test]
    #[should_panic(expected = "not sorted")]
    fn out_of_order_entries_are_reported() {
        parse(&[
            2, 0, 0, 0, //
            1, 0, 0, 0, b'b', 0, 0, 0, 0, //
            1, 0, 0, 0, b'a', 0, 0, 0, 0,
        ]);
    }

    #[test]
    #[should_panic(expected = "duplicate key")]
    fn duplicate_keys_are_reported() {
        parse(&[
            2, 0, 0, 0, //
            1, 0, 0, 0, b'a', 1, 0, 0, 0, b'1', //
            1, 0, 0, 0, b'a', 1, 0, 0, 0, b'2',
        ]);
    }

    #[test]
    #[should_panic(expected = "ends mid-value of entry 0")]
    fn a_blob_cut_short_of_its_last_value_is_reported() {
        // Announces a 3-byte value but supplies one byte.
        parse(&[
            1, 0, 0, 0, //
            1, 0, 0, 0, b'k', //
            3, 0, 0, 0, b'v',
        ]);
    }

    #[test]
    #[should_panic(expected = "ends mid-key of entry 0")]
    fn a_blob_cut_short_of_a_length_is_reported() {
        parse(&[1, 0, 0, 0, 1, 0]);
    }

    /// A wildly large count must not pre-allocate itself to death before the
    /// blob runs out — the reservation is capped, and the truncation is what
    /// gets reported.
    #[test]
    #[should_panic(expected = "ends mid-key of entry 0")]
    fn an_absurd_count_fails_on_the_missing_bytes_not_on_allocation() {
        parse(&[0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    #[should_panic(expected = "trailing bytes")]
    fn a_count_that_does_not_match_the_entries_is_reported() {
        // Says one entry, supplies two.
        parse(&[
            1, 0, 0, 0, //
            1, 0, 0, 0, b'a', 0, 0, 0, 0, //
            1, 0, 0, 0, b'b', 0, 0, 0, 0,
        ]);
    }

    #[test]
    #[should_panic(expected = "not UTF-8")]
    fn a_non_utf8_key_is_reported() {
        parse(&[
            1, 0, 0, 0, //
            1, 0, 0, 0, 0xFF, //
            0, 0, 0, 0,
        ]);
    }

    /// The whole prologue path, through the host stubs: a blob served by
    /// `environ_len`/`environ_read`, copied into the (natively leaked) shared
    /// region and indexed, then read back through the public API. There is one
    /// global environment per process, so this is the only test that touches
    /// [`init`] and the module-level [`get`]/[`iter`].
    #[test]
    fn init_reads_the_hosts_blob_and_the_public_api_serves_it() {
        crate::abi::set_environ_blob(vec![
            2, 0, 0, 0, //
            4, 0, 0, 0, b'b', b'b', b'.', b'x', 2, 0, 0, 0, b'1', b'2', //
            5, 0, 0, 0, b's', b'p', b'e', b'e', b'd', 3, 0, 0, 0, b'2', b'.', b'5',
        ]);
        init();
        assert_eq!(get("bb.x"), Some("12"));
        assert_eq!(get("speed"), Some("2.5"));
        assert_eq!(get("missing"), None);
        assert_eq!(
            iter().collect::<Vec<_>>(),
            vec![("bb.x", "12"), ("speed", "2.5")]
        );
        // Re-initialising is a no-op rather than a second parse: the
        // environment is fixed for the instance's life.
        crate::abi::set_environ_blob(Vec::new());
        init();
        assert_eq!(get("speed"), Some("2.5"));
    }
}
