// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
internal static class DirectTypos {
    internal static void Write(string path) {
        var corrector=RomajiTypoCorrector.Load(CompositionDetector.CreateDefault().Romaji);
        foreach(var row in JsonDocument.Parse(File.ReadAllText(path)).RootElement.EnumerateArray()) {
            var raw=row.GetString()!;
            Console.WriteLine(JsonSerializer.Serialize(new { raw,expected=corrector.Fix(raw,true) }));
        }
    }
}
