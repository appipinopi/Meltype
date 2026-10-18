// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import Foundation

func testInputSourceRegistration(_ editor:NativeEditorPolicy) throws {
    let identifier=LegacyInputSourceRegistration.identifier
    let other:[String:Any]=["InputSourceKind":"Keyboard Layout","KeyboardLayout Name":"ABC","unknown":Data([0,1,255])]
    let base:[String:Any]=["Bundle ID":identifier,"InputSourceKind":"Keyboard Input Method"]
    let variants:[[String:Any]]=[
        other,base,["Input Mode":identifier+".Japanese"],["Input Mode":identifier+".Other"],
        ["Input Mode":identifier+"-suffix"],["Bundle ID":identifier+"X"],
        ["Bundle ID":25,"Input Mode":false],["Bundle ID":NSNull(),"Input Mode":NSNull()],
        ["Input Mode":"other."+identifier],["unknown":["a",1,Data([0])]],
        ["Bundle ID":"other","date":Date(timeIntervalSince1970:17)],[:],
    ]
    var comparisons=0
    for first in variants.indices {
        for last in variants.indices {
            let sources=[variants[first]]+Array(repeating:base,count:first)+[variants[last],other]
            let expected=LegacyInputSourceRegistration.normalized(sources)
            let actual=InputSourceRegistration.normalized(sources)
            precondition(NSArray(array:actual).isEqual(to:expected),"input-source normalization differs")
            precondition(NSArray(array:actual).isEqual(to:InputSourceRegistration.normalized(actual)),"normalization not idempotent")
            comparisons+=1
        }
    }
    let inputs:[Any?]=[
        nil,[Any](),[other]+Array(repeating:base,count:15),
        [["Bundle ID":identifier],["Input Mode":identifier+".Japanese.extra"],other],
        LegacyInputSourceRegistration.normalized([other]),
        "invalid stored array",25,Data([4,5]),[1,2],["a","b"],
        [["Bundle ID":25,"Input Mode":false]],
    ]
    for (index,input) in inputs.enumerated() {
        let prefix="MeltypeRustRegistration."+UUID().uuidString
        let oldName=prefix+".old",nativeName=prefix+".native"
        let old=UserDefaults(suiteName:oldName)!,native=UserDefaults(suiteName:nativeName)!
        defer {old.removePersistentDomain(forName:oldName);native.removePersistentDomain(forName:nativeName)}
        for defaults in [old,native] {
            defaults.set(["preserve backup marker"],forKey:"MeltypeInputSourcesBeforeRepair")
            defaults.set("untouched",forKey:"other preference")
            if let input {defaults.set(input,forKey:"AppleEnabledInputSources")}
        }
        precondition(LegacyInputSourceRegistration.repair(old)==0)
        precondition(editor.repairInputSources(native))
        let expected=old.persistentDomain(forName:oldName)!,actual=native.persistentDomain(forName:nativeName)!
        precondition(NSDictionary(dictionary:actual).isEqual(to:expected),"preference repair/backup differs for fixture \(index)")
        precondition(editor.repairInputSources(native))
        precondition(NSDictionary(dictionary:native.persistentDomain(forName:nativeName)!).isEqual(to:actual),"repeat repair replaced original backup")
    }
    // Production registration must reject before accessing HIToolbox/Carbon on
    // a background thread. Never invoke this mutating entry on the main thread.
    let done=DispatchSemaphore(value:0)
    DispatchQueue.global().async {
        precondition(editor.registerInputSource()==1)
        done.signal()
    }
    precondition(done.wait(timeout:.now()+5) == .success)
    print("macOS Rust input-source registration: \(comparisons) legacy NSDictionary normalization/idempotence cases and \(inputs.count) isolated NSUserDefaults repair/backup/type cases, background registration rejection passed (real Carbon register/enable NOT_RUN; status/order/release covered by Rust fake API)")
}
