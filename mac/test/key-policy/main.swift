// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import AppKit
import Carbon.HIToolbox
import InputMethodKit

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

var candidatePlans=0
for candidates in [[], ["一"], ["一","二","😀"], ["a\u{0}b","e\u{301}"]] {
 for previous in [[],candidates,["古","候補"],["a\u{0}b","é"]] {
  for visible in [false,true] {
   for current in [0,1,2] {
    for selected in [-1,0,1,2,9] {
     for supported in [false,true] {
      let view=CompositionView(text:"かな",converting:true,selectedIndex:selected,selectedClause:0,hint:"",candidates:candidates,clauses:[],meaning:nil,suggestion:nil)
      var expected:[String]=[],actual:[String]=[]
      if candidates.count>1 {
       expected.append("0:0")
       var index=current
       if previous != candidates || !visible {expected.append("1:0");index=0}
       if selected>=0 && selected<candidates.count {
        let direction=selected>index ? 1 : -1
        expected.append("2:\(direction)")
        if supported {
         for _ in 0..<abs(selected-index) {expected.append("3:\(direction)")}
         expected.append("4:\(selected)")
        }
       }
       expected.append("5:0")
      } else {expected.append("6:0")}
      precondition(editor.candidates(view,previous:previous,visible:visible,windowIndex:current) {op,arg in actual.append("\(op):\(arg)");return op==2 ? (supported ? 1 : 0) : 1})
      precondition(actual==expected,"candidate update differs from former SDK policy")
      candidatePlans+=1
     }
    }
   }
  }
 }
}
print("macOS Rust candidates: \(candidatePlans) legacy update/selection plans, unchanged/hidden/rebuilt lists, unsupported selectors and invalid selection passed")

precondition(editor.meaningDelayMilliseconds==1500,"meaning delay differs from original")
guard let meaningSession=NativeCore.shared.createSession() else {fatalError("private meaning session creation")}
var previousMeaning:String?=nil
var timers:[(Int,String)]=[]
var meaningChecks=0
for (index,meaning) in [(0,nil),(0,""),(0,""),(1,"意味😀"),(1,"é"),(1,"e\u{301}"),(2,"e\u{301}"),(2,"a\u{0}b"),(0,nil),(1,"意味😀")] as [(Int,String?)] {
 let key=meaning.map {"\(index):\($0)"}
 let expected=key != previousMeaning && key != nil
 precondition(editor.reserveMeaning(meaningSession,index:index,text:meaning)==expected,"meaning schedule differs from Swift key comparison")
 previousMeaning=key
 if let meaning {timers.append((index,meaning))}
 for (priorIndex,priorMeaning) in timers {
  precondition(editor.matchesMeaning(meaningSession,index:priorIndex,text:priorMeaning)==(previousMeaning=="\(priorIndex):\(priorMeaning)"),"stale meaning callback match differs")
  meaningChecks+=1
 }
}
NativeCore.shared.destroySession(meaningSession)
precondition(!editor.matchesMeaning(meaningSession,index:1,text:"意味😀"),"destroyed meaning handle matched")
precondition(!editor.reserveMeaning(meaningSession,index:1,text:"意味😀"),"destroyed meaning handle resurrected")
print("macOS Rust meaning SDK: \(meaningChecks) Swift reservation/timer-key comparisons, canonical equality, NUL, clear and destroyed handle passed")

// Exercise the real Rust -> libdispatch main queue -> Swift callback path.
final class MeaningLifetimeProbe {}
var probe: MeaningLifetimeProbe? = MeaningLifetimeProbe()
weak var weakProbe = probe
var deliveries = 0
let timedWindow = CandidateWindowProbe()
precondition(editor.candidateWindow(timedWindow,operation:1,argument:1)==1)
let timerStarted = ProcessInfo.processInfo.systemUptime
guard let liveTimerSession = NativeCore.shared.createSession(),
      let deadTimerSession = NativeCore.shared.createSession() else { fatalError("timer sessions") }
precondition(editor.reserveMeaning(liveTimerSession,index:0,text:"意味😀"))
precondition(editor.reserveMeaning(deadTimerSession,index:0,text:"古い"))
precondition(editor.scheduleMeaning { [held = probe!] in
    _ = held
    precondition(Thread.isMainThread,"Rust timer left main thread")
    precondition(ProcessInfo.processInfo.systemUptime-timerStarted>=1.5,"meaning fired early")
    precondition(editor.matchesMeaning(liveTimerSession,index:0,text:"意味😀"))
    precondition(editor.candidateWindow(timedWindow,operation:5,text:"意味😀")==1)
    deliveries+=1
})
precondition(editor.scheduleMeaning {
    precondition(Thread.isMainThread)
    precondition(!editor.matchesMeaning(deadTimerSession,index:0,text:"古い"),"destroyed timer revived")
    deliveries+=1
})
probe=nil
precondition(weakProbe != nil,"callback capture released before delivery")
NativeCore.shared.destroySession(deadTimerSession)
let timerTimeout=Date(timeIntervalSinceNow:10)
while deliveries<2 && Date()<timerTimeout {
    RunLoop.main.run(until:Date(timeIntervalSinceNow:0.02))
}
precondition(deliveries==2,"Rust main queue timer missing")
precondition(timedWindow.receivedUnits==Array("意味😀".utf16),"timer annotation lost")
precondition(weakProbe == nil,"callback capture leaked after delivery")
RunLoop.main.run(until:Date(timeIntervalSinceNow:0.1))
precondition(deliveries==2,"timer delivered more than once")
NativeCore.shared.destroySession(liveTimerSession)
print("macOS Rust dispatch: real 1500ms main-queue delivery, two callbacks exactly once, capture lifetime and destroyed-session rejection passed")

// A real Objective-C SDK object records messages from the Rust production bridge.
// It does not create/register an input method or send input to another app.
@objc final class CandidateWindowProbe: NSObject {
    var visible = false
    var trace: [String] = []
    var supportsUp = true
    var receivedUnits: [UInt16] = []
    @objc func isVisible() -> Bool { trace.append("visible"); return visible }
    @objc(updateCandidates) func update() { trace.append("update") }
    @objc(show:) func show(_ hint: UInt) { trace.append("show:\(hint)"); visible = true }
    @objc func hide() { trace.append("hide"); visible = false }
    @objc(moveDown:) func down(_ sender: Any?) { trace.append("down") }
    @objc(moveUp:) func up(_ sender: Any?) { trace.append("up") }
    @objc(showAnnotation:) func annotate(_ value: NSAttributedString) {
        trace.append("annotation")
        receivedUnits = Array(value.string.utf16)
        if value.length > 0 { precondition(value.attributes(at:0,effectiveRange:nil).isEmpty) }
    }
    override func responds(to selector: Selector!) -> Bool {
        if !supportsUp && selector == NSSelectorFromString("moveUp:") { return false }
        return super.responds(to: selector)
    }
}
var windowChecks = 0
for initiallyVisible in [false,true] {
 for supportsUp in [false,true] {
  for text in ["", "意味😀", "e\u{301}", "a\u{0}b", "👩‍💻"] {
   let original = CandidateWindowProbe(), native = CandidateWindowProbe()
   original.visible=initiallyVisible;native.visible=initiallyVisible
   original.supportsUp=supportsUp;native.supportsUp=supportsUp
   precondition(editor.candidateWindow(native,operation:0)==(original.isVisible() ? 1 : 0))
   // Original annotation is only sent while visible.
   if original.isVisible() { original.annotate(NSAttributedString(string:text)) }
   precondition(editor.candidateWindow(native,operation:5,text:text)==(initiallyVisible ? 1 : 0))
   original.update();original.show(1)
   precondition(editor.candidateWindow(native,operation:1,argument:1)==1)
   for direction in [-1,1] {
    let step=NSSelectorFromString(direction>0 ? "moveDown:" : "moveUp:")
    let supported=original.responds(to:step)
    precondition(editor.candidateWindow(native,operation:2,argument:direction)==(supported ? 1 : 0))
    if supported { original.perform(step,with:nil) }
    precondition(editor.candidateWindow(native,operation:3,argument:direction)==(supported ? 1 : 0))
   }
   if original.isVisible() { original.annotate(NSAttributedString(string:text)) }
   precondition(editor.candidateWindow(native,operation:5,text:text)==1)
   original.hide();precondition(editor.candidateWindow(native,operation:4)==1)
   precondition(editor.candidateWindow(native,operation:5,text:text)==0)
   // Last hidden query is shared by the original annotation guard.
   _=original.isVisible()
   precondition(original.trace==native.trace,"direct Objective-C candidate sequence differs: original=\(original.trace), native=\(native.trace)")
   precondition(original.receivedUnits==native.receivedUnits,"annotation UTF16 changed")
   let before=native.trace
   precondition(editor.candidateWindow(native,operation:1,argument:-1)==0)
   precondition(editor.candidateWindow(native,operation:2,argument:0)==0)
   precondition(editor.candidateWindow(native,operation:3,argument:2)==0)
   precondition(editor.candidateWindow(native,operation:99)==0)
   precondition(native.trace==before,"invalid request changed window")
   windowChecks+=1
  }
 }
}
precondition(editor.candidateWindow(NSObject(),operation:0)==0,"missing SDK selector accepted")
print("macOS Rust window: \(windowChecks) original Objective-C message traces, visible/hidden annotation, direction support, Unicode/NUL/empty payloads and invalid request rejection passed")

let backgroundWindow = CandidateWindowProbe()
let backgroundDone = DispatchSemaphore(value:0)
DispatchQueue.global().async {
    precondition(editor.candidateWindow(backgroundWindow,operation:1,argument:1)==0,"background window call accepted")
    backgroundDone.signal()
}
precondition(backgroundDone.wait(timeout:.now()+5) == .success)
precondition(backgroundWindow.trace.isEmpty,"background SDK call changed window")
print("macOS Rust window: off-main-thread operation rejected before Objective-C UI call")

precondition(#selector(IMKCandidates.update)==NSSelectorFromString("updateCandidates"),"SDK update selector changed")
for selector in [#selector(IMKCandidates.update), #selector(IMKCandidates.show(_:)), #selector(IMKCandidates.hide), #selector(IMKCandidates.isVisible), #selector(IMKCandidates.showAnnotation(_:))] {
    precondition(IMKCandidates.instancesRespond(to:selector),"actual IMKCandidates lacks \(selector)")
}
print("macOS Rust window: actual IMKCandidates SDK selectors checked, including Swift update -> Objective-C updateCandidates")

// A private SDK server/panel in this temporary test bundle. No input-source
// registration or activation, no connection to another application's editor.
@objc(MeltypePolicyController) final class PolicyInputController: IMKInputController {}
_ = NSApplication.shared
precondition(editor.createServer(name:"",bundleIdentifier:"local.empty")==nil)
precondition(editor.createCandidates(server:NSObject(),panelType:1)==nil,"non-IMK server accepted")
guard let policyBundle=Bundle.main.bundleIdentifier,
      let policyConnection=Bundle.main.object(forInfoDictionaryKey:"InputMethodConnectionName") as? String,
      let policyServer=editor.createServer(name:policyConnection,bundleIdentifier:policyBundle) as? IMKServer else {
    fatalError("private Rust IMKServer creation failed")
}
var ownedPanel: IMKCandidates?
autoreleasepool {
    ownedPanel=editor.createCandidates(server:policyServer,panelType:UInt64(kIMKSingleColumnScrollingCandidatePanel)) as? IMKCandidates
}
guard let activePanel=ownedPanel else {fatalError("private Rust IMKCandidates creation failed")}
guard let swiftPanel=IMKCandidates(server:policyServer,panelType:kIMKSingleColumnScrollingCandidatePanel) else {fatalError("reference SDK panel creation failed")}
precondition(activePanel.panelType()==swiftPanel.panelType(),"panel type differs from SDK")
precondition(activePanel.selectionKeys() as? [NSNumber] == swiftPanel.selectionKeys() as? [NSNumber],"default selection keys differ")
precondition(activePanel.dismissesAutomatically()==swiftPanel.dismissesAutomatically(),"default dismiss policy differs")
precondition(editor.candidateWindow(activePanel,operation:0)==(swiftPanel.isVisible() ? 1 : 0))
precondition(editor.candidateWindow(activePanel,operation:4)==1)
swiftPanel.hide()
precondition(!activePanel.isVisible() && !swiftPanel.isVisible())
let creationDone=DispatchSemaphore(value:0)
DispatchQueue.global().async {
    precondition(editor.createServer(name:policyConnection+"_background",bundleIdentifier:policyBundle)==nil)
    precondition(editor.createCandidates(server:policyServer,panelType:1)==nil)
    creationDone.signal()
}
precondition(creationDone.wait(timeout:.now()+5) == .success)
print("macOS Rust startup: private actual IMKServer and IMKCandidates created, ARC transfer through autorelease pool, SDK default panel/keys/dismiss/visibility and background/non-server rejection passed (no input-source registration)")

// Run the real AppKit loop through Rust; the private fixture stops its own app.
final class PolicyServicesProvider: NSObject {}
let policyProvider=PolicyServicesProvider()
let policyServicePort=policyConnection+"_Services"
precondition(!editor.registerServices(provider:policyProvider,portName:""))
precondition(!editor.registerServices(provider:policyProvider,portName:"bad\u{0}name"))
precondition(editor.registerServices(provider:policyProvider,portName:policyServicePort))
let applicationThreadDone=DispatchSemaphore(value:0)
DispatchQueue.global().async {
    precondition(!editor.runApplication(),"background AppKit loop accepted")
    precondition(!editor.registerServices(provider:policyProvider,portName:policyServicePort+"_background"))
    applicationThreadDone.signal()
}
precondition(applicationThreadDone.wait(timeout:.now()+5) == .success)
var loopCallbacks=0
let loopWatchdog=DispatchWorkItem { fatalError("Rust AppKit event loop did not stop") }
DispatchQueue.global().asyncAfter(deadline:.now()+10,execute:loopWatchdog)
precondition(editor.scheduleMeaning {
    precondition(Thread.isMainThread && NSApplication.shared.isRunning)
    precondition(editor.candidateWindow(activePanel,operation:4)==1)
    loopCallbacks+=1
    NSApplication.shared.stop(nil)
    // stop: from a runloop/GCD callback needs an event to wake nextEvent's wait.
    guard let wake=NSEvent.otherEvent(with:.applicationDefined,location:.zero,modifierFlags:[],timestamp:0,windowNumber:0,context:nil,subtype:0,data1:0,data2:0) else {fatalError("fixture wake event")}
    NSApplication.shared.postEvent(wake,atStart:true)
})
precondition(editor.runApplication(),"Rust AppKit loop failed")
loopWatchdog.cancel()
precondition(loopCallbacks==1 && !NSApplication.shared.isRunning)
NSUnregisterServicesProvider(policyServicePort)
print("macOS Rust application: actual AppKit run/stop/return with Rust timer and private SDK panel, named Services registration call, invalid-name/background rejection passed (no installed service shortcut delivery tested)")

// Exercise the real Rust Objective-C receiver through its registered SDK selector.
final class ServicesTargetProbe: ModeServiceTarget {
    var actions: [ModeShortcutAction]=[]
    var available=true
    var duringApply: (() -> Void)?
    func selectModeFromService(_ action: ModeShortcutAction) -> Bool {
        guard available else { return false }
        actions.append(action);duringApply?();return true
    }
}
func invokeNativeService(_ provider: NSObject, _ identifier: String?) -> NSString? {
    typealias Receiver = @convention(c) (AnyObject, Selector, NSPasteboard, NSString?, AutoreleasingUnsafeMutablePointer<NSString?>) -> Void
    let selector=NSSelectorFromString("changeInputMode:userData:error:")
    precondition(provider.responds(to:selector))
    guard let implementation=provider.method(for:selector) else {fatalError("native Services selector implementation missing")}
    let method=unsafeBitCast(implementation,to:Receiver.self)
    var error: NSString?
    method(provider,selector,serviceBoard,identifier as NSString?,&error)
    return error
}
let serviceBoard=NSPasteboard.withUniqueName()
serviceBoard.setString("既存の文字😀",forType:.string)
var selectedSource=true
var idleServiceActions: [ModeShortcutAction]=[]
var servicesOwner: ModeServices?=ModeServices(isMeltypeSelected:{selectedSource},selectWithoutInput:{idleServiceActions.append($0)})
weak var weakServicesOwner=servicesOwner
guard let realServiceProvider=servicesOwner?.serviceProvider as? NSObject else {fatalError("native Services provider missing")}
let oldServicesTarget=ServicesTargetProbe(), currentServicesTarget=ServicesTargetProbe()
servicesOwner!.activate(oldServicesTarget)
servicesOwner!.activate(currentServicesTarget)
servicesOwner!.deactivate(oldServicesTarget)
for action in ModeShortcutAction.allCases {
    precondition(invokeNativeService(realServiceProvider,action.serviceIdentifier)==nil)
    precondition(currentServicesTarget.actions.last==action)
}
precondition(oldServicesTarget.actions.isEmpty)
for identifier in [nil,"","DIRECT","0","unknown","direct\u{0}"] as [String?] {
    precondition(invokeNativeService(realServiceProvider,identifier)=="Meltypeの入力モードの指定が不正です。")
}
selectedSource=false
precondition(invokeNativeService(realServiceProvider,"direct")=="入力ソースでMeltypeを選択してください。")
selectedSource=true;currentServicesTarget.available=false
precondition(invokeNativeService(realServiceProvider,"code")=="Meltypeの入力欄を取得できませんでした。")
currentServicesTarget.available=true
// Reentrant activation/deactivation must not hold a native RefCell borrow.
currentServicesTarget.duringApply={servicesOwner!.deactivate(currentServicesTarget)}
precondition(invokeNativeService(realServiceProvider,"cycle")==nil)
precondition(invokeNativeService(realServiceProvider,"direct")==nil)
precondition(idleServiceActions==[.direct])
var temporaryServicesTarget: ServicesTargetProbe?=ServicesTargetProbe()
weak var weakServicesTarget=temporaryServicesTarget
servicesOwner!.activate(temporaryServicesTarget!)
temporaryServicesTarget=nil
precondition(weakServicesTarget==nil,"Rust retained inactive Swift target")
precondition(invokeNativeService(realServiceProvider,"cycle")==nil)
precondition(idleServiceActions==[.direct,.cycle])
let servicesBackgroundDone=DispatchSemaphore(value:0)
DispatchQueue.global().async {
    autoreleasepool {
        precondition(invokeNativeService(realServiceProvider,"direct")=="Meltypeの入力モードはメインスレッドで変更してください。")
    }
    servicesBackgroundDone.signal()
}
precondition(servicesBackgroundDone.wait(timeout:.now()+5) == .success)
precondition(serviceBoard.string(forType:.string)=="既存の文字😀","Services modified pasteboard")
servicesOwner=nil
precondition(weakServicesOwner==nil,"native service provider retained Swift owner")
precondition(invokeNativeService(realServiceProvider,"direct")=="Meltypeの入力欄を取得できませんでした。")
// A source-query callback may dispose its own Swift owner; native delivery pins
// it for this invocation and rejects subsequent calls through the retained root.
var disposeOwner: ModeServices?
var disposeActions: [ModeShortcutAction]=[]
disposeOwner=ModeServices(isMeltypeSelected:{disposeOwner=nil;return true},selectWithoutInput:{disposeActions.append($0)})
weak var weakDisposeOwner=disposeOwner
guard let disposeProvider=disposeOwner?.serviceProvider as? NSObject else {fatalError("dispose provider")}
precondition(invokeNativeService(disposeProvider,"code")==nil)
precondition(disposeActions==[.code] && weakDisposeOwner==nil)
precondition(invokeNativeService(disposeProvider,"direct")=="Meltypeの入力欄を取得できませんでした。")
let backgroundFirstOwner=ModeServices(isMeltypeSelected:{true},selectWithoutInput:{_ in})
let backgroundFirstDone=DispatchSemaphore(value:0)
DispatchQueue.global().async {
    autoreleasepool {
        var error: NSString?
        backgroundFirstOwner.changeInputMode(serviceBoard,userData:"direct",error:&error)
        precondition(error=="Meltypeの入力モードはメインスレッドで変更してください。")
    }
    backgroundFirstDone.signal()
}
precondition(backgroundFirstDone.wait(timeout:.now()+5) == .success)
var backgroundFirstError: NSString?
backgroundFirstOwner.changeInputMode(serviceBoard,userData:"direct",error:&backgroundFirstError)
precondition(backgroundFirstError==nil,"first background request poisoned provider creation")
// Compare the native read-only Carbon query with the SDK without changing source.
guard let currentSource=TISCopyCurrentKeyboardInputSource()?.takeRetainedValue(),
      let sourceProperty=TISGetInputSourceProperty(currentSource,kTISPropertyInputSourceID),
      let sourceIdentifier=unsafeBitCast(sourceProperty,to:CFTypeRef.self) as? String else {fatalError("SDK input source query")}
precondition(editor.inputSourceMatches(expected:sourceIdentifier))
precondition(!editor.inputSourceMatches(expected:sourceIdentifier+"_different"))
serviceBoard.releaseGlobally()
print("macOS Rust Services receiver: actual Objective-C selector, four actions, exact errors, inactive/dead targets, old deactivation, pure Swift weak references, reentry, background rejection, owner cleanup, unchanged private pasteboard and read-only Carbon identity passed (no OS shortcut delivery)")

func originalSuggestion(_ text: String, caret: NSRect, panel existing: NSPanel?) -> NSPanel {
    let label=NSTextField(labelWithString:text)
    label.font = .systemFont(ofSize:13);label.sizeToFit()
    let size=NSSize(width:label.frame.width+24,height:label.frame.height+16)
    let panel=existing ?? NSPanel(contentRect:NSRect(origin:.zero,size:size),styleMask:[.borderless,.nonactivatingPanel],backing:.buffered,defer:false)
    panel.level = .floating;panel.hasShadow=true;panel.backgroundColor = .windowBackgroundColor;panel.ignoresMouseEvents=true
    panel.setContentSize(size)
    panel.contentView?.subviews.forEach {$0.removeFromSuperview()}
    label.frame.origin=NSPoint(x:12,y:8);panel.contentView?.addSubview(label)
    if let screen=NSScreen.screens.first(where:{$0.frame.intersects(caret)}) ?? NSScreen.main {
        let visible=screen.visibleFrame
        panel.setFrameOrigin(NSPoint(x:max(visible.minX,min(caret.minX,visible.maxX-size.width)),y:max(visible.minY,min(caret.minY-size.height-6,visible.maxY-size.height))))
    }
    panel.orderFrontRegardless();return panel
}
var nativeSuggestion: NSPanel?, referenceSuggestion: NSPanel?
let suggestionScreen=NSScreen.main?.visibleFrame ?? NSRect(x:0,y:0,width:800,height:600)
var suggestionCases=0
for text in ["", "提案😀", "e\u{301}", "a\u{0}b", "長い候補"+String(repeating:"あ",count:120)] {
 for caret in [NSRect(x:suggestionScreen.midX,y:suggestionScreen.midY,width:1,height:16),NSRect(x:suggestionScreen.minX-20,y:suggestionScreen.minY,width:1,height:16),NSRect(x:suggestionScreen.maxX,y:suggestionScreen.maxY,width:1,height:16),NSRect(x:suggestionScreen.midX,y:suggestionScreen.midY,width:-2,height:16)] {
    let previous=nativeSuggestion
    referenceSuggestion=originalSuggestion(text,caret:caret,panel:referenceSuggestion)
    nativeSuggestion=editor.suggestion(panel:nativeSuggestion,text:text,caret:caret,selected:true) as? NSPanel
    guard let actual=nativeSuggestion,let expected=referenceSuggestion else {fatalError("suggestion panel missing")}
    if let previous {precondition(actual===previous,"suggestion panel recreated")}
    precondition(actual.styleMask==expected.styleMask && actual.level==expected.level && actual.hasShadow==expected.hasShadow && actual.ignoresMouseEvents==expected.ignoresMouseEvents)
    precondition(actual.backgroundColor==expected.backgroundColor)
    precondition(actual.frame.equalTo(expected.frame),"suggestion frame differs: \(actual.frame), \(expected.frame)")
    guard let actualLabel=actual.contentView?.subviews.first as? NSTextField,let expectedLabel=expected.contentView?.subviews.first as? NSTextField else {fatalError("suggestion label missing")}
    precondition(actual.contentView?.subviews.count==1)
    precondition(Array(actualLabel.stringValue.utf16)==Array(expectedLabel.stringValue.utf16))
    precondition(actualLabel.font==expectedLabel.font && actualLabel.frame.equalTo(expectedLabel.frame))
    precondition(actual.isVisible && !actual.isKeyWindow)
    actual.orderOut(nil);expected.orderOut(nil)
    suggestionCases+=1
 }
}
guard let retainedSuggestion=nativeSuggestion else {fatalError("suggestion lifetime")}
for (text,caret,selected) in [(nil,NSRect(x:1,y:1,width:1,height:16),true),("候補",.zero,true),("候補",NSRect(x:Double.nan,y:1,width:1,height:16),true),("候補",NSRect(x:1,y:1,width:1,height:16),false)] as [(String?,NSRect,Bool)] {
    retainedSuggestion.orderFrontRegardless()
    precondition(editor.suggestion(panel:retainedSuggestion,text:text,caret:caret,selected:selected) === retainedSuggestion)
    precondition(!retainedSuggestion.isVisible,"invalid suggestion not hidden")
}
let suggestionBackgroundDone=DispatchSemaphore(value:0)
DispatchQueue.global().async {
    precondition(editor.suggestion(panel:retainedSuggestion,text:"背景",caret:NSRect(x:1,y:1,width:1,height:16),selected:true)==nil)
    suggestionBackgroundDone.signal()
}
precondition(suggestionBackgroundDone.wait(timeout:.now()+5) == .success)
precondition(!retainedSuggestion.isVisible)
print("macOS Rust suggestion: \(suggestionCases) actual AppKit panels compared with former Swift, font/UTF16/geometry/style/reuse, invalid caret/selection hide and background rejection passed (screen appearance not asserted)")
