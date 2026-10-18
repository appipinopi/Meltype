// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 hrmcngs

import Carbon
import Foundation

enum LegacyInputSourceRegistration {
    static let identifier = "io.github.yksr-melt.inputmethod.Meltype"

    static func normalized(_ sources: [[String: Any]]) -> [[String: Any]] {
        var result = sources.filter {
            ($0["Bundle ID"] as? String) != identifier &&
            !($0["Input Mode"] as? String ?? "").hasPrefix(identifier)
        }
        result.append(["Bundle ID": identifier, "InputSourceKind": "Keyboard Input Method"])
        result.append(["Bundle ID": identifier, "Input Mode": identifier + ".Japanese", "InputSourceKind": "Input Mode"])
        return result
    }

    static func repair(_ defaults: UserDefaults) -> Int32 {
        let sources = defaults.array(forKey: "AppleEnabledInputSources") as? [[String: Any]] ?? []
        let repaired = normalized(sources)
        if !NSArray(array: sources).isEqual(to: repaired) {
            defaults.set(sources, forKey: "MeltypeInputSourcesBeforeRepair")
            defaults.set(repaired, forKey: "AppleEnabledInputSources")
            defaults.synchronize()
        }
        return 0
    }
}
