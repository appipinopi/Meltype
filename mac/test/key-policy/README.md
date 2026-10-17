# Rust macOS key policy validation

`bash mac/test/key-policy/run.sh /absolute/path/libmeltype_native.dylib`
requires macOS and its Swift/Carbon/AppKit SDK. It builds a temporary owned app,
loads the actual Rust library through the production NativeCore initializer,
and uses production KeyMapping/NativeKeyPolicy with the former SDK implementation
as an independent reference. It compares all UInt16 key codes, a Unicode/control/
punctuation corpus, and modifier combinations including device/ignored bits.
NSEvents are constructed only in the test process; no event is posted and no
input source is registered or activated. The converter fixture is test-only and
is not called or packaged. This does not prove actual IMK input or UI parity.

Rust packaging enables MELTYPE_RUST_CORE=1 in mac/build.sh. Explicit legacy
NativeAOT builds retain the original Swift mapping. OS lowercase normalization
and Unicode extraction stay at the SDK boundary, preserving system behavior.
The Rust build rejects a missing key ABI without switching to the legacy mapper.
The focused Rust macOS SDK workflow also compiles the complete Swift adapter,
including its real converter dependency, separately from the SDK fixture.
