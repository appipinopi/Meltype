// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 hrmcngs

import Carbon
import Foundation

enum LegacyInputSourceRegistration {
    static let identifier = "io.github.yksr-melt.inputmethod.Meltype"

    // 参照: upstream ecb2ba1c の登録順・付随情報を保持する正規化。
    static let japaneseIdentifier = identifier + ".Japanese"

    static func isMeltype(_ source: [String: Any]) -> Bool {
        (source["Bundle ID"] as? String) == identifier ||
        (source["Input Mode"] as? String) == japaneseIdentifier
    }

    static func normalized(_ sources: [[String: Any]]) -> [[String: Any]] {
        var seen = Set<String>()
        var result: [[String: Any]] = []
        for source in sources {
            guard isMeltype(source) else {
                result.append(source)
                continue
            }
            let kind = source["InputSourceKind"] as? String
            if kind == "Input Mode", (source["Input Mode"] as? String) == japaneseIdentifier {
                if seen.insert("Input Mode").inserted { result.append(source) }
            } else if kind == "Keyboard Input Method", (source["Bundle ID"] as? String) == identifier {
                if seen.insert("Keyboard Input Method").inserted { result.append(source) }
            } else {
                result.append(source)
            }
        }
        if !seen.contains("Input Mode") {
            result.append(["Bundle ID": identifier, "Input Mode": japaneseIdentifier, "InputSourceKind": "Input Mode"])
        }
        if !seen.contains("Keyboard Input Method") {
            result.append(["Bundle ID": identifier, "InputSourceKind": "Keyboard Input Method"])
        }
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
