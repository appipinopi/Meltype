// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Test-only converter dependency. NativeCore initialization is real; callbacks
// are not invoked by key-policy tests. This fixture is never packaged.
final class MeltypeConverter {
    static let shared = MeltypeConverter()
    func clauses(for reading: String, context: String?) -> [(reading: String, text: String)] { [] }
    func candidates(for reading: String) -> [String] { [] }
}
