// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 hrmcngs

import Carbon
import Foundation

enum InputSourceRegistration {
    static let identifier = "io.github.yksr-melt.inputmethod.Meltype"

    static func normalized(_ sources: [[String: Any]]) -> [[String: Any]] {
#if MELTYPE_RUST_CORE
        guard let policy=NativeCore.shared.editorPolicy else {preconditionFailure("Rust input-source policy unavailable")}
        return policy.normalizedInputSources(sources)
#else
        var result = sources.filter {
            ($0["Bundle ID"] as? String) != identifier &&
            !($0["Input Mode"] as? String ?? "").hasPrefix(identifier)
        }
        result.append(["Bundle ID": identifier, "InputSourceKind": "Keyboard Input Method"])
        result.append(["Bundle ID": identifier, "Input Mode": identifier + ".Japanese", "InputSourceKind": "Input Mode"])
        return result
#endif
    }

    static func register() -> Int32 {
#if MELTYPE_RUST_CORE
        return NativeCore.shared.editorPolicy?.registerInputSource() ?? 1
#else
        guard let defaults = UserDefaults(suiteName: "com.apple.HIToolbox") else { return 1 }
        let sources = defaults.array(forKey: "AppleEnabledInputSources") as? [[String: Any]] ?? []
        let repaired = normalized(sources)
        if !NSArray(array: sources).isEqual(to: repaired) {
            defaults.set(sources, forKey: "MeltypeInputSourcesBeforeRepair")
            defaults.set(repaired, forKey: "AppleEnabledInputSources")
            defaults.synchronize()
        }
        let status = TISRegisterInputSource(Bundle.main.bundleURL as CFURL)
        guard status == noErr else { return status }
        let list = TISCreateInputSourceList(nil, true).takeRetainedValue() as NSArray
        for item in list {
            let source = item as! TISInputSource
            guard let pointer = TISGetInputSourceProperty(source, kTISPropertyInputSourceID) else { continue }
            let id = unsafeBitCast(pointer, to: CFString.self) as String
            if id == identifier || id == identifier + ".Japanese" {
                let enabled = TISEnableInputSource(source)
                guard enabled == noErr else { return enabled }
            }
        }
        return 0
#endif
    }
}
