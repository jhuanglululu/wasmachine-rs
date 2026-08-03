//! The animation's environment: an immutable, ordered set of key/value strings
//! the host hands the guest at startup.
//!
//! Where they come from is the plugin's business (a `/bb env` command, host
//! built-ins, …); what this module guarantees is the shape. The environment is
//! **read-only and fixed for the run** — changing it restarts the animation, so
//! a guest never has to wonder whether a value it read a minute ago is still
//! current. It is also loaded *once*, during [`__rt::init`](crate::__rt::init),
//! before seeding and before the user's `main`, so the first lookup is already
//! a plain memory read rather than an ABI crossing.
//!
//! ```ignore
//! let env = wasmachine::environ();
//! let speed: f64 = env.get("speed").unwrap_or("1.0").parse().expect("speed");
//! for (key, value) in env.iter() {
//!     wasmachine::log(&format!("{key} = {value}"));
//! }
//! ```
//!
//! # Wire format
//!
//! The host serves the whole thing as one blob (`environ_len` / `environ_read`,
//! the same two-call read the rest of the ABI uses). Every integer is a
//! little-endian `u32`: an entry count, then per entry a key length, the key's
//! UTF-8 bytes, a value length, the value's UTF-8 bytes. Entries arrive sorted
//! by key bytes.
//!
//! The host is trusted, so the parser's job is not defence — it is *diagnosis*.
//! A truncated, mis-counted, non-UTF-8 or duplicate-keyed blob means the two
//! sides disagree about the format, and that fails loudly through the panic
//! hook (host `fail`, which kills the animation with the message) rather than
//! silently serving a half-read environment.

use std::sync::OnceLock;

/// The environment, parsed once and kept for the life of the animation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Environ {
    /// Sorted by key, no duplicates — both checked at parse time.
    entries: Vec<(String, String)>,
}

impl Environ {
    /// The value for `key`, or `None` if it is not set.
    ///
    /// The borrow is the environment's own, so
    /// `environ().get("k")` yields an `Option<&'static str>`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .binary_search_by(|(k, _)| k.as_str().cmp(key))
            .ok()
            .map(|i| self.entries[i].1.as_str())
    }

    /// Every entry, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// How many entries there are.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the environment is empty — which it is whenever nothing was set
    /// for this animation.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Parse the host's blob. `Err` carries a message describing *how* the two
    /// sides disagree; see the module docs for the format.
    fn parse(blob: &[u8]) -> Result<Environ, String> {
        let mut reader = Reader { blob, at: 0 };
        let count = reader.u32("entry count")? as usize;
        let mut entries: Vec<(String, String)> = Vec::with_capacity(count.min(1024));
        for i in 0..count {
            let key = reader.string(&format!("key of entry {i}"))?;
            let value = reader.string(&format!("value of entry {i}"))?;
            if let Some((previous, _)) = entries.last() {
                match previous.as_str().cmp(&key) {
                    core::cmp::Ordering::Less => {}
                    core::cmp::Ordering::Equal => {
                        return Err(format!("environ has duplicate key {key:?}"));
                    }
                    core::cmp::Ordering::Greater => {
                        return Err(format!(
                            "environ is not sorted by key: {previous:?} precedes {key:?}"
                        ));
                    }
                }
            }
            entries.push((key, value));
        }
        if reader.at != blob.len() {
            return Err(format!(
                "environ blob has {} trailing bytes after {count} entries",
                blob.len() - reader.at
            ));
        }
        Ok(Environ { entries })
    }
}

/// A cursor over the blob. Every read either advances or reports exactly what
/// it was short of.
struct Reader<'a> {
    blob: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u32(&mut self, what: &str) -> Result<u32, String> {
        let end = self.at.wrapping_add(4);
        let bytes = self.blob.get(self.at..end).ok_or_else(|| {
            format!(
                "environ blob ends mid-{what}: need 4 bytes at offset {}, blob is {} bytes",
                self.at,
                self.blob.len()
            )
        })?;
        self.at = end;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn string(&mut self, what: &str) -> Result<String, String> {
        let len = self.u32(what)? as usize;
        // `wrapping_add` (here and above) rather than `+`: a bogus length from a
        // disagreeing host must come back as "the blob is too short", not as an
        // arithmetic overflow — and on wasm32 a `u32` length can wrap `usize`.
        let end = self.at.wrapping_add(len);
        let bytes = self.blob.get(self.at..end).ok_or_else(|| {
            format!(
                "environ blob ends mid-{what}: need {len} bytes at offset {}, blob is {} bytes",
                self.at,
                self.blob.len()
            )
        })?;
        let text = core::str::from_utf8(bytes)
            .map_err(|e| format!("environ {what} is not UTF-8: {e}"))?
            .to_owned();
        self.at = end;
        Ok(text)
    }
}

static ENVIRON: OnceLock<Environ> = OnceLock::new();

/// The animation's environment. Cheap: the blob is read and parsed once, during
/// [`__rt::init`](crate::__rt::init), and every call after that hands back the
/// same `&'static Environ`.
pub fn environ() -> &'static Environ {
    ENVIRON.get_or_init(load)
}

/// Read and parse the blob, or die telling the host what it sent.
fn load() -> Environ {
    #[cfg(target_arch = "wasm32")]
    let blob = crate::abi::marshal::environ();
    // Off wasm there is no host to ask, so the environment is simply empty —
    // which keeps `environ()` callable from `cargo test` without a stub panic.
    #[cfg(not(target_arch = "wasm32"))]
    let blob: Vec<u8> = Vec::new();

    if blob.is_empty() {
        return Environ::default();
    }
    Environ::parse(&blob).unwrap_or_else(|e| panic!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::Environ;

    // Every blob below is written out by hand, byte by byte, so a bug in a
    // serializer could never cancel out against a bug in the parser. `u32`
    // lengths are spelled as their four little-endian bytes.

    #[test]
    fn an_empty_environ_is_a_bare_zero_count() {
        let env = Environ::parse(&[0, 0, 0, 0]).expect("empty blob parses");
        assert!(env.is_empty());
        assert_eq!(env.len(), 0);
        assert_eq!(env.get("anything"), None);
        assert_eq!(env.iter().count(), 0);
    }

    #[test]
    fn one_entry_round_trips_key_and_value() {
        // count = 1
        // key_len = 5, "speed"
        // value_len = 3, "2.5"
        let env = Environ::parse(&[
            1, 0, 0, 0, //
            5, 0, 0, 0, b's', b'p', b'e', b'e', b'd', //
            3, 0, 0, 0, b'2', b'.', b'5',
        ])
        .expect("one entry parses");
        assert_eq!(env.len(), 1);
        assert_eq!(env.get("speed"), Some("2.5"));
        assert_eq!(env.get("spee"), None);
        assert_eq!(env.get("speedy"), None);
    }

    #[test]
    fn several_entries_iterate_in_key_order() {
        // "a" = "1", "bb" = "", "c" = "xyz"
        let env = Environ::parse(&[
            3, 0, 0, 0, //
            1, 0, 0, 0, b'a', 1, 0, 0, 0, b'1', //
            2, 0, 0, 0, b'b', b'b', 0, 0, 0, 0, //
            1, 0, 0, 0, b'c', 3, 0, 0, 0, b'x', b'y', b'z',
        ])
        .expect("three entries parse");
        assert_eq!(env.len(), 3);
        assert_eq!(
            env.iter().collect::<Vec<_>>(),
            vec![("a", "1"), ("bb", ""), ("c", "xyz")]
        );
        // And lookup agrees with iteration, including the empty value.
        assert_eq!(env.get("a"), Some("1"));
        assert_eq!(env.get("bb"), Some(""));
        assert_eq!(env.get("c"), Some("xyz"));
        assert_eq!(env.get("b"), None);
    }

    /// A key may be any UTF-8, and the lengths are *bytes*, not characters —
    /// "héllo" is six bytes for five characters.
    #[test]
    fn keys_and_values_are_utf8_measured_in_bytes() {
        let env = Environ::parse(&[
            1, 0, 0, 0, //
            6, 0, 0, 0, b'h', 0xC3, 0xA9, b'l', b'l', b'o', //
            4, 0, 0, 0, 0xF0, 0x9F, 0x8E, 0xA8, // U+1F3A8, one character
        ])
        .expect("utf-8 blob parses");
        assert_eq!(env.get("héllo"), Some("🎨"));
        assert_eq!(env.iter().next(), Some(("héllo", "🎨")));
    }

    #[test]
    fn a_blob_cut_short_of_its_last_value_is_reported() {
        // Announces a 3-byte value but supplies one byte.
        let err = Environ::parse(&[
            1, 0, 0, 0, //
            1, 0, 0, 0, b'k', //
            3, 0, 0, 0, b'v',
        ])
        .expect_err("a truncated value must not parse");
        assert!(err.contains("ends mid-value of entry 0"), "{err}");
        assert!(err.contains("need 3 bytes"), "{err}");
    }

    #[test]
    fn a_blob_cut_short_of_a_length_is_reported() {
        let err = Environ::parse(&[1, 0, 0, 0, 1, 0])
            .expect_err("a truncated length must not parse");
        assert!(err.contains("ends mid-key of entry 0"), "{err}");
    }

    #[test]
    fn an_empty_blob_has_not_even_a_count() {
        let err = Environ::parse(&[]).expect_err("a zero-byte blob must not parse");
        assert!(err.contains("ends mid-entry count"), "{err}");
    }

    #[test]
    fn a_count_that_does_not_match_the_entries_is_reported() {
        // Says one entry, supplies two.
        let err = Environ::parse(&[
            1, 0, 0, 0, //
            1, 0, 0, 0, b'a', 0, 0, 0, 0, //
            1, 0, 0, 0, b'b', 0, 0, 0, 0,
        ])
        .expect_err("trailing entries must not be ignored");
        assert!(err.contains("trailing bytes"), "{err}");
    }

    #[test]
    fn a_non_utf8_key_is_reported() {
        let err = Environ::parse(&[
            1, 0, 0, 0, //
            1, 0, 0, 0, 0xFF, //
            0, 0, 0, 0,
        ])
        .expect_err("invalid UTF-8 must not parse");
        assert!(err.contains("not UTF-8"), "{err}");
    }

    /// `get` binary-searches, so out-of-order entries would make lookups lie.
    /// The host promises sorted order; a violation is a format disagreement and
    /// is reported rather than quietly re-sorted.
    #[test]
    fn out_of_order_entries_are_reported() {
        let err = Environ::parse(&[
            2, 0, 0, 0, //
            1, 0, 0, 0, b'b', 0, 0, 0, 0, //
            1, 0, 0, 0, b'a', 0, 0, 0, 0,
        ])
        .expect_err("unsorted entries must not parse");
        assert!(err.contains("not sorted"), "{err}");
    }

    #[test]
    fn duplicate_keys_are_reported() {
        let err = Environ::parse(&[
            2, 0, 0, 0, //
            1, 0, 0, 0, b'a', 1, 0, 0, 0, b'1', //
            1, 0, 0, 0, b'a', 1, 0, 0, 0, b'2',
        ])
        .expect_err("duplicate keys must not parse");
        assert!(err.contains("duplicate key"), "{err}");
    }

    /// A wildly large count must not pre-allocate itself to death before the
    /// blob runs out — the reservation is capped, and the truncation is what
    /// gets reported.
    #[test]
    fn an_absurd_count_fails_on_the_missing_bytes_not_on_allocation() {
        let err = Environ::parse(&[0xFF, 0xFF, 0xFF, 0xFF])
            .expect_err("a count with no entries behind it must not parse");
        assert!(err.contains("ends mid-key of entry 0"), "{err}");
    }
}
