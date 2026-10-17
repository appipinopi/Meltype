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

The same SDK host also compares the Rust editor-range bridge with the former
InputController NSRange arithmetic and Swift canonical String equality. It covers
UTF-16 deletion lengths, marked/selected caret choice, missing expected strings,
20/4000-unit context windows, overflow rejection, composed/decomposed characters,
Hangul, NUL and compatibility-character distinctions. The existing controlled
IMK event/boundary tools accept --rust-core to exercise product Rust paths; their
explicit source lists include the new bridge files for legacy builds as well.

Rust's macOS-specific handle-key ABI also preserves upstream external-scalar
semantics: invalid scalars leave composition unchanged; combining marks commit
without automatic spacing (Latin accents preserve unchosen ASCII raw text), and
supplementary scalars commit before the original event passes through. Other
frontends keep their full-scalar input path. Existing Swift/IMK boundary tests
check this actual production entry point. A portable ABI fixture reproduces the
eight original Swift external-text cases without registering an input source.
