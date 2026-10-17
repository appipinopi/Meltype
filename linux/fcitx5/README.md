# fcitx5 adapter migration

The default `MELTYPE_RUST_CORE=ON` build requires the Rust native library,
including `meltype_json_visit_utf8`, `meltype_fcitx_key` and
`meltype_fcitx_modifiers`. JSON bytes, escapes and numbers are parsed
in Rust. The C++ event adapter receives copied UTF-8 values for the existing
fcitx5 widgets; it does not parse JSON. Keycode/modifier translation also runs in Rust. The SDK supplies Unicode
key-symbol conversion and raw modifier bits. Input-context ownership, mode policy,
editor application and widget integration still contain C++ and remain migration
work. This is not a standalone Rust fcitx5 addon yet.

Build on Linux with the fcitx5 development packages installed:

```sh
cargo build -p meltype-native --release --locked
cmake -S linux/fcitx5 -B linux/build/fcitx5-rust -DMELTYPE_RUST_CORE=ON -DMELTYPE_RUST_ABI_TESTS=ON
cmake --build linux/build/fcitx5-rust --parallel 2
python3 linux/fcitx5/test/json_bridge.py target/release/libmeltype_native.so
linux/build/fcitx5-rust/testkeymapping "$PWD/target/release/libmeltype_native.so"
```

The addon loads `libMeltypeNative.so` under `MELTYPE_DIR`, retaining the existing
frontend filename. The Cargo output is `libmeltype_native.so`; packaging must
copy it under the frontend filename. These commands do not install/activate an
input method. The Rust Linux bundle currently packages IBus, not this addon.

`MELTYPE_RUST_CORE=OFF` explicitly selects the legacy NativeAOT compatibility
reader in `compat/json_reader.hpp`. The old `linux/build.sh` selects that option
for its C# library. It is not included in the default Rust adapter. Existing
upstream history and GPL notices are retained.

The UTF-8 visitor validates the complete document before callbacks, rejects
malformed JSON/UTF-8 and unpaired surrogate escapes, and uses the last duplicate
object value. Embedded escaped NUL is length-delimited. Callback text is borrowed
only for the synchronous callback. A zero callback return cancels traversal, and
C++ exceptions are caught at the callback boundary. The Windows TSF compatibility
visitor retains its existing numeric and UTF-16 behavior separately.

The standalone test compiles the production C++ event adapter, loads the real
Rust library and compares 217 valid reference/core projections. It additionally
checks malformed-input rejection, unchanged output on failure, cancellation,
Unicode/NUL and duplicate keys. Temporary profiles are private; it does not run a
fcitx daemon or type into any application. The SDK addon builds and real fcitx5
input require separate evidence. CI builds both Rust and compatibility variants
and runs the standalone ABI test against its release Rust library.

The SDK key test compares the original virtual-key policy for 131,076 key
symbols and 512 modifier combinations through the real Rust ABI. It preserves
the former fcitx policy (including ignoring lock bits and Super2), independently
of the IBus frontend’s mapping. Japanese mode/role keys are still handled by the
C++ adapter before ordinary key translation. See the [SDK modifier definitions](https://github.com/fcitx/fcitx5/blob/master/src/lib/fcitx-utils/keysym.h)
and [raw flag accessor](https://github.com/fcitx/fcitx5/blob/master/src/lib/fcitx-utils/flags.h).
