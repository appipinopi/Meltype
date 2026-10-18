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

Real-converter mode registers the production callbacks before exercising the
IMK adapter. It compares Space and Tab commits with the pinned converter's actual
clause result; prediction candidates are disabled in this fixture so Tab tests
conversion rather than prediction selection. Other settings remain explicit in
the private profile. Candidate Tab/Shift+Tab navigation must not refetch the
backend. macOS external candidates retain the registered converter's order
rather than being displaced by bundled dictionary/context candidates.

The OS English spelling callback is now resolved by Rust before acquiring the
session lock. Resolved evidence has a thread-local scope with restoration on
nested calls/unwind; no host callback runs from dictionary evaluation. Rust caches
positive and negative checks for five minutes (4096 entries), invalidates the
cache on callback registration, and rejects results if the provider was replaced
or the session was destroyed while the callback ran. Private/direct/disabled/
manual/kana paths do not request OS evidence. Swift Rust builds keep only the
NSSpellChecker OS call; the cache and check counter are native. The portable real
ABI test covers classification, commit/Undo, cache behavior, callback replacement,
reentrant session access and destruction. The real-converter SDK host requires
an actual NSSpellChecker invocation through the registered product callback.
This is evidence integration, not proof of full upstream detector parity.
