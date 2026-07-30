# wasmachine-rs

The `wasmachine` Rust crate — the guest-side core for the
[WASMachine](https://github.com/jhuanglululu/WASMachine) WASM runtime:
safe task/spawn/sleep wrappers, sync primitives, random streams, panic
hook + host allocator, math types and the SDK-internal `sdk_main` entry
macro that plugin SDKs instantiate as their own `main` attribute.
Everything user-facing is safe Rust; all unsafe/extern lives in the ABI
layer with host-target stubs so `cargo test` runs natively.

Plugin SDKs (e.g. [`billboard-rs`](https://github.com/jhuanglululu/billboard-rs))
depend on this crate by path and layer their own import modules beside the
engine's.

Personal-use library: versioned by git, consumed via cargo path dependency.
