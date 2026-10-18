# Rust macOS key policy validation

`bash mac/test/key-policy/run.sh /absolute/path/libmeltype_native.dylib`
requires macOS and its Swift/Carbon/AppKit SDK. It builds a temporary owned app,
loads the actual Rust library through the production NativeCore initializer,
and uses production KeyMapping/NativeKeyPolicy with the former SDK implementation
as an independent reference. It compares all UInt16 key codes, a Unicode/control/
punctuation corpus, and modifier combinations including device/ignored bits.
Key-comparison NSEvents are constructed only in the test process. The AppKit
loop fixture posts an application-defined wake event to its own app after stop;
no key event is posted and no input source is registered or activated. The
converter fixture is test-only and is not shipped with the product. This does
not prove actual IMK input or UI parity.

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

The Rust response-application ABI now owns the edit loop and publishes the view
only after successful edits. Its Swift sink supplies live marked/selected ranges,
UTF-16 substrings, attributed insertion and the existing view renderer. Substring
buffers are explicitly owned through the next callback. Each dependent edit
queries the current client, not a cached initial range; mismatch stops later edits
and rendering. No core/session lock spans SDK callbacks. The old loop remains in
the explicit legacy build. Independent Foundation replay compares traces and final
documents, while controlled IMK tests exercise dependent replacement and mismatch.
Portable ABI tests cover each host-failure point and malformed-response rejection
before any callback. Objective-C exception recovery, actual external client input
and moving the remaining rendering/OS bridge to Rust still require work.

Composition decoration is now planned by Rust: UTF-16 clause offsets, raw/
converted/selected style, valid nonempty attribute ranges, and cursor-at-end
publication. The Swift sink maps styles to actual Carbon marks and sends the
attributed string through IMK; candidate and suggestion widgets still use Swift.
The independent SDK reference compares the old plan across empty/mismatched
clauses, selection boundaries, combining text, NUL and supplementary characters.
The controlled product IMK host verifies attributed UTF-16 payload/selection and
clear. A portable ABI test ensures callback failures stop before publication and
malformed replies do not start callbacks. This does not establish pixel parity.

Candidate update orchestration now runs in Rust. It requests list rebuild only
when contents differ by Swift-compatible canonical equality or the window is
hidden, asks whether the direction selector exists, performs incremental moves,
then records the selected index and requests meaning scheduling. Zero/one-item
lists clear/hide. Swift supplies the actual IMKCandidates calls and always ends
selection-notification suppression, including a failed traversal. The old update
and move loops remain only in legacy builds. The independent SDK host compares
list/selection traces across visibility, content, index and selector support;
portable ABI tests stop at each callback failure and reject malformed input.
Actual candidate-window pixels and complete IMK input still need verification,
along with other remaining direct OS integration.

Meaning reservation keys, duplicate scheduling, canonical matching, delay policy
(1500 ms) and session cleanup now live in Rust. Rust schedules the libdispatch main queue. Swift checks the original session
token/key with Rust, then Rust queries window visibility and sends the annotation
through Objective-C. Clear/hide invalidates the current key; core
session destruction removes it and stale callbacks cannot match a new session.
The A->B->A key behavior deliberately matches the former Swift equality policy.
SDK and portable ABI tests compare original key rules, empty versus absent text,
NUL, independent sessions, malformed UTF-16 and repeated destruction/recreation.
The SDK host checks actual delayed delivery on the main queue, one-shot callback
lifetime, and annotation delivery to a controlled Objective-C object. It checks
the actual IMKCandidates selectors, including Swift update() mapping to the
Objective-C updateCandidates selector. Candidate-window pixels remain unverified.

The temporary app also creates an actual private IMKServer and IMKCandidates
through the Rust initializers and compares panel type, selection keys, automatic
dismissal and visibility with the SDK initializer. No input source is registered.
The AppKit fixture calls the original named Services registration through Rust,
then runs the real NSApplication event loop, delivers a native timer, operates
the private SDK panel, and stops/wakes its own app. A watchdog bounds the loop.
Background loop/registration calls and invalid service names are rejected. The
SDK Services function returns void: the fixture proves the registration call,
not delivery of an installed service shortcut from another application.


The mode Services receiver itself is now an Objective-C class implemented in Rust.
The SDK host invokes its actual changeInputMode:userData:error: implementation
through the production Swift bridge. It exercises all four identifiers, exact
legacy errors, inactive/dead targets, identity-checked old deactivation, weak pure
Swift targets, reentrant deactivation, owner disposal during the source callback,
first-call background rejection and a retained receiver after owner destruction.
A private pasteboard remains unchanged. The Carbon source query is compared with
the SDK current source without selecting or registering any input source. The
remaining Swift callbacks perform mode selection and indicator display; direct
receiver invocation does not prove OS delivery of an installed Services shortcut.


Suggestion panels are now created/updated/hidden through Rust AppKit calls. The
SDK host compares twenty actual panels with the former Swift implementation,
including font, UTF-16 payload, label/content/window geometry, style, screen-edge
placement, long/empty/combining/NUL/emoji text, negative caret width and reuse.
Invalid caret/missing selection hides the panel; background calls are rejected.
These are actual AppKit property comparisons, not screenshot or full app input
verification. The Swift boundary still obtains the IMK selection and caret.


Mode-indicator view generation and layout now live in Rust. A test-only copy of
the former Swift implementation compares six complete AppKit view trees and ten
layout frames. The comparison includes effect material/blending/state, font and
label frames, selected accent colors, layer radii, accessibility radio roles,
labels/values, compact/central caption and caret validation. Deferred presentation, panel management and the one-second NSTimer now live in
Rust too. The lifecycle fixture compares actual panel settings with the former
Swift panel and checks queue supersession, hide-before-presentation, preference
queries at presentation time, timer replacement, panel reuse, missing screens,
and destruction while pending/visible. Both queue and timer target hold only
weak owners. Swift provides the preference read callback and bridge entry point. Full-screen visual parity
and actual editor input are not established by these property comparisons.


The mode/preference bridge uses Rust-owned NSUserDefaults calls with the existing
MacSelectedInputMode and MacShowModeIndicator schema. A private UUID-named suite
checks twenty bool/action comparisons, absent defaults, persistence and restoration
through new selection objects, per-instance cached modes, and invalid persisted
values without rewriting them. The private suite is removed after the fixture.
This restoration check creates new instances in the same process; a full app
restart and installed input-method use remain separate integration checks.
