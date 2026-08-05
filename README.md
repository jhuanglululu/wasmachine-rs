# wasmachine-rs

The `wasmachine` Rust crate — the guest-side core for the
[WASMachine](https://github.com/jhuanglululu/WASMachine) WASM runtime:
safe task/spawn/sleep wrappers, sync primitives, random streams, the
read-only `env`, panic hook + host allocator, math types and the
SDK-internal `sdk_main` entry macro that plugin SDKs instantiate as their
own `main` attribute. Everything user-facing is safe Rust; all
unsafe/extern lives in the ABI layer with host-target stubs so
`cargo test` runs natively.

Engine ABI 2. A task's memory is still its own — `spawn` forks and
deep-copies it — with one exception: the host-owned **shared static
region** (`engine.shared_alloc`), which a fork references instead of
copying. `env` is what lives there today, parsed once from the host's
environ blob before the animation's `main`, which is why a value is a
`&'static str` every task shares.

Plugin SDKs (e.g. [`billboard-rs`](https://github.com/jhuanglululu/billboard-rs))
depend on this crate and layer their own import modules beside the
engine's.

Personal-use library: versioned by git, no publishing pipeline; consumed
as a cargo git dependency on this repo (commit pinned by `Cargo.lock`).
