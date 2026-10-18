// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import AppKit
import Carbon.HIToolbox

precondition(NativeCore.shared.keyPolicy != nil, "real NativeCore did not resolve Rust key ABI")
precondition(NativeKeyPolicy(lookup: { _ in nil }) == nil, "missing ABI accepted")
func event(_ code: UInt16, _ text: String) -> NSEvent {
    guard let value = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
        windowNumber: 0, context: nil, characters: text, charactersIgnoringModifiers: text, isARepeat: false, keyCode: code) else {
        fatalError("could not create controlled key event")
    }
    return value
}
var count = 0
func compare(_ code: UInt16, _ text: String) {
    let value = event(code, text)
    let expected = LegacyKeyMapping.virtualKey(for: value)
    let actual = KeyMapping.virtualKey(for: value)
    precondition(actual == expected, "key mapping mismatch: code=\(code), text=\(text.debugDescription), actual=\(String(describing: actual)), expected=\(String(describing: expected))")
    count += 1
}
let texts = ["", "a", "Z", "0", "9", ",", ".", "-", "/", "[", "]", ";", " ", "\n", "\t", "\u{1f}", "かな", "😀", "İ", "Σ", "𐐀", "e\u{301}", "👩‍💻", "Ａ", "\u{200d}", "\u{0}"]
for code in UInt16.min...UInt16.max {
    autoreleasepool { compare(code, "q") }
}
for code in UInt16(0)...UInt16(255) {
    for text in texts { autoreleasepool { compare(code, text) } }
}
var modifiersCount = 0
for bits in UInt(0)..<512 {
    for deviceFlags in [UInt(0), UInt(0xffff)] {
        let flags = NSEvent.ModifierFlags(rawValue: (bits << 16) | deviceFlags)
        var expected: Int32 = 0
        if flags.contains(.shift) { expected |= 1 }
        if flags.contains(.control) { expected |= 2 }
        if flags.contains(.option) { expected |= 4 }
        if flags.contains(.command) { expected |= 8 }
        precondition(KeyMapping.modifiers(for: flags) == expected, "SDK modifier mismatch")
        modifiersCount += 1
    }
}
print("macOS Rust key policy: \(count) SDK/legacy events, \(modifiersCount) modifier states, Unicode normalization, special-key priority and missing-ABI rejection passed")

// Reference the previous InputController arithmetic and Swift String equality.
guard let editor = NativeCore.shared.editorPolicy else { fatalError("missing Rust editor policy") }
precondition(NativeEditorPolicy(lookup: { _ in nil }) == nil)
func legacyReplacement(_ marked: NSRange, _ selected: NSRange, _ remove: Int, _ expected: String?) -> NSRange? {
    if remove <= 0 { return NSRange(location: NSNotFound, length: 0) }
    let caret = marked.location != NSNotFound && marked.length > 0 ? marked.location : selected.location
    guard caret != NSNotFound, caret >= remove, let expected, (expected as NSString).length == remove else { return nil }
    return NSRange(location: caret - remove, length: remove)
}
var rangeCount = 0
for location in [0, 1, 19, 20, 21, 3999, 4000, 4001, Int.max - 40, NSNotFound] {
    for length in [0, 1, 20] {
        for code in [false, true] {
            let selection = NSRange(location: location, length: length)
            let result = editor.contextRanges(selection: selection, code: code)
            if location == NSNotFound { precondition(result == nil) }
            else {
                let start = max(0, location - (code ? 4000 : 20))
                precondition(result?.0 == NSRange(location: start, length: location - start))
                precondition(result?.1 == NSRange(location: location + length, length: 20))
            }
            rangeCount += 1
        }
    }
}
let expectedTexts: [String?] = [nil, "", "a", "😀", "e\u{301}", "é", "かな"]
for location in [0, 5, 99, NSNotFound] {
    for length in [0, 1, 3] {
        for selected in Array(0...11) + [NSNotFound] {
            for remove in -1...6 {
                for expected in expectedTexts {
                    let marked = NSRange(location: location, length: length)
                    let selection = NSRange(location: selected, length: 0)
                    precondition(editor.replacementRange(marked: marked, selected: selection, remove: remove, expected: expected) == legacyReplacement(marked, selection, remove, expected))
                    rangeCount += 1
                }
            }
        }
    }
}
precondition(editor.contextRanges(selection: NSRange(location: Int.max - 1, length: 2), code: false) == nil)
let comparisons = ["", "a", "a\u{0}b", "e\u{301}", "é", "Å", "A\u{30a}", "가", "\u{1100}\u{1161}", "ﬁ", "fi", "😀"]
var equalityCount = 0
for actual in comparisons {
    for expected in comparisons {
        precondition(editor.expectedEqual(actual, expected) == (actual == expected), "Swift canonical equality mismatch")
        equalityCount += 1
    }
}
print("macOS Rust editor policy: \(rangeCount) legacy NSRange cases, \(equalityCount) Swift canonical comparisons, UTF-16 lengths, marked/selected caret and overflow rejection passed")

// Independently replay the old InputController loop using Foundation String ==.
func response(_ edits: [(Int, String, String?)]) -> SessionResult {
    SessionResult(consumed: true, commits: edits.map { TextEdit(deleteBefore: $0.0, text: $0.1, expect: $0.2) }, view: nil)
}
let editCases = [response([]),response([(0,"😀",nil)]),response([(3,"仮名","前😀")]),
    response([(2,"NG","前😀"),(0,"later",nil)]),response([(1,"NG",nil)]),
    response([(0,"a",nil),(1,"😀","a"),(2,"終","😀")]),
    response([(0,"a\u{0}b",nil),(3,"終","a\u{0}b")]),response([(-1,"追記",nil)])]
var applyCount=0
for initial in ["", "前😀", "e\u{301}", "a\u{0}b"] {
 for initialMarked in [false,true] {
  for result in editCases {
   var document=initial, marked=initialMarked, trace:[String]=[], published=false
   var reference=initial, referenceMarked=initialMarked, referenceTrace:[String]=[], referencePublished=true
   for edit in result.commits {
    var range=NSRange(location:NSNotFound,length:0)
    if edit.deleteBefore>0 {
     referenceTrace.append("marked")
     if !referenceMarked {referenceTrace.append("selected")}
     let caret=(reference as NSString).length
     guard caret>=edit.deleteBefore,let expected=edit.expect,(expected as NSString).length==edit.deleteBefore else {referencePublished=false;break}
     range=NSRange(location:caret-edit.deleteBefore,length:edit.deleteBefore)
     referenceTrace.append("substring:\(range.location):\(range.length)")
     guard (reference as NSString).substring(with:range)==expected else {referencePublished=false;break}
    }
    referenceTrace.append("insert:\(range.location):\(range.length):\(edit.text)")
    if range.location==NSNotFound {reference+=edit.text}
    else {reference=(reference as NSString).replacingCharacters(in:range,with:edit.text)}
    referenceMarked=false
   }
   if referencePublished {referenceTrace.append("render")}
   let completed=editor.apply(result,marked:{trace.append("marked");return NSRange(location:marked ? (document as NSString).length : NSNotFound,length:marked ? 1 : 0)},
     selected:{trace.append("selected");return NSRange(location:(document as NSString).length,length:0)},
     substring:{range in trace.append("substring:\(range.location):\(range.length)");return (document as NSString).substring(with:range)},
     insert:{text,range in trace.append("insert:\(range.location):\(range.length):\(text)");if range.location==NSNotFound {document+=text}else{document=(document as NSString).replacingCharacters(in:range,with:text)};marked=false},
     render:{published=true;trace.append("render")})
   precondition(completed==referencePublished && published==referencePublished && document==reference && trace==referenceTrace,"old/new edit loop mismatch")
   applyCount+=1
  }
 }
}
var canonicalDocument="é", canonicalRendered=false
precondition(editor.apply(response([(1,"x","e\u{301}")]),marked:{NSRange(location:NSNotFound,length:0)},selected:{NSRange(location:1,length:0)},substring:{_ in "é"},insert:{_,_ in fatalError("UTF16 mismatch must reject before mutation")},render:{fatalError("invalid length rendered")} )==false)
// A host may return canonically equivalent text with a different UTF16 length.
precondition(editor.apply(response([(2,"x","e\u{301}")]),marked:{NSRange(location:2,length:1)},selected:{fatalError("marked caret must suppress selectedRange")},substring:{_ in canonicalDocument},insert:{text,_ in canonicalDocument=text},render:{canonicalRendered=true}))
precondition(canonicalDocument=="x" && canonicalRendered)
print("macOS Rust apply: \(applyCount) independent legacy edit traces/final documents, current ranges, Unicode/NUL, ordered edits, mismatch stop and canonical comparison passed")

var compositionCount=0
for text in ["", "前😀後", "e\u{301}", "a\u{0}b"] {
 for clauses in [[], ["前", "😀", "後"], ["",text], [text+"外"], ["a", "b"]] {
  for converting in [false,true] {
   for selected in [-1,0,1,8] {
    let view=CompositionView(text:text,converting:converting,selectedIndex:0,selectedClause:selected,hint:"",candidates:[],clauses:clauses,meaning:nil,suggestion:nil)
    let length=(text as NSString).length
    var expected:[String]=[],actual:[String]=[]
    if converting && !clauses.isEmpty {
     var location=0
     for (index,clause) in clauses.enumerated() {
      let size=(clause as NSString).length
      if size>0 && location+size<=length {expected.append("\(index==selected ? 2 : 1):\(location):\(size)")}
      location+=size
     }
    } else if length>0 {expected.append("0:0:\(length)")}
    expected.append("publish:\(length)")
    precondition(editor.composition(view,mark:{style,range in actual.append("\(style):\(range.location):\(range.length)")},publish:{actual.append("publish:\($0)")}))
    precondition(actual==expected,"composition decoration trace differs from former Swift")
    compositionCount+=1
   }
  }
 }
}
print("macOS Rust composition: \(compositionCount) legacy SDK decoration plans, UTF16 clauses, selected/raw styles, empty/outside ranges and publication order passed")
