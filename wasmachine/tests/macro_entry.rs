//! What `sdk_main` generates, checked by compiling it and calling the exports
//! it emits — the same three functions the host looks up in a real module.
//!
//! This test file *is* the plugin SDK: `extern crate self as sdk` lets it play
//! the role `billboard` plays for an animation, supplying the items the macro's
//! generated code reaches for through the `sdk` path. That is the whole contract
//! a plugin SDK has to satisfy, so it is worth having one written down where it
//! can fail loudly.

extern crate self as sdk;

// The SDK-side contract: the engine's runtime glue and handshake constant come
// straight from `wasmachine`; the exit type and the plugin's own ABI version
// are the SDK's to define.
pub use wasmachine::{__rt, ENGINE_ABI_VERSION};

/// The plugin handshake value — deliberately not 1, so a test asserting on it
/// cannot pass by accidentally reading the engine's.
pub const ABI_VERSION: i32 = 7;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitCode {
    End = 0,
    Keep = 1,
}

impl ExitCode {
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

/// No `random_seed` here on purpose: seeding would reach the host import, which
/// panics on this target, and then `_engine_main` could not be called at all.
/// The seed path has its own coverage in `wasmachine-macros`' unit tests.
#[wasmachine_macros::sdk_main(
    config(
        sdk = ::sdk,
        attribute = "#[sdk::main]",
        abi_export = _plugin_abi,
    )
)]
fn animation() -> ExitCode {
    ExitCode::Keep
}

#[test]
fn the_engine_handshake_is_fixed_not_configurable() {
    // Every guest exports this name, whatever plugin it belongs to — that is
    // what lets the host check the engine ABI without knowing the plugin.
    assert_eq!(_engine_abi(), 2);
    // One source of truth: the macro exports the crate's constant.
    assert_eq!(_engine_abi(), ENGINE_ABI_VERSION);
}

#[test]
fn the_plugin_handshake_is_the_one_the_config_named() {
    assert_eq!(_plugin_abi(), 7);
    assert_eq!(_plugin_abi(), ABI_VERSION);
}

/// The entry export runs the author's function and hands back its exit code
/// raw — the engine attaches no meaning to that `i32`, the plugin does.
#[test]
fn the_entry_export_returns_the_authors_exit_code() {
    assert_eq!(_engine_main(), ExitCode::Keep.as_i32());
    assert_eq!(_engine_main(), 1);
    // And the author's own function is still callable under its own name.
    assert_eq!(animation(), ExitCode::Keep);
}
