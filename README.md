# wasmachine-rs

The `wasmachine` Rust crate — the guest-side core for the
[WASMachine](https://github.com/jhuanglululu/WASMachine) WASM runtime:
safe task wrappers (`spawn`, `scope`, `sleep`), sync primitives, random
streams, the read-only `environ`, panic hook + host allocator, math types
and the SDK-internal `sdk_main` entry macro that plugin SDKs instantiate
as their own `main` attribute. Everything user-facing is safe Rust; all
unsafe/extern lives in the ABI layer with host-target stubs so
`cargo test` runs natively.

All tasks of an animation share **one linear memory** (engine ABI 2 —
ABI 1 fork-copied it per task), so moving an owning handle into a task is
ordinary Rust and `scope` can lend one a borrow.

## Building a guest module

Animation crates must export the shadow stack pointer, so the host can
give each spawned task its own stack region:

```toml
# .cargo/config.toml
[target.wasm32-unknown-unknown]
rustflags = ["-C", "link-arg=--export=__stack_pointer"]
```

Missing that export is a construction-time error from the host naming the
flag. Guests also build `panic = "abort"` (the workspace release profile
here already does), since a panic routes through the SDK hook to the
host's `fail`.

This repo ships no wasm example crate of its own — the crate builds as an
rlib and its tests run on the host — so there is no `.cargo/config.toml`
here to carry the flag; it belongs in each animation crate.

Plugin SDKs (e.g. [`billboard-rs`](https://github.com/jhuanglululu/billboard-rs))
depend on this crate and layer their own import modules beside the
engine's.

Personal-use library: versioned by git, no publishing pipeline; consumed
as a cargo git dependency on this repo (commit pinned by `Cargo.lock`).
