//! Library-mode UniFFI bindings generator.
//!
//! Invoked as `cargo run --bin uniffi-bindgen -- generate --library <cdylib>
//! --language <lang> --out-dir <dir>`. Keeping the generator in-crate locks the
//! bindgen version to the same `uniffi` the cdylib was built against.

fn main() {
    uniffi::uniffi_bindgen_main()
}
