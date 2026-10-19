// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
var dictionary=MisspellingDictionary.Load(null);
foreach(var reading in JsonSerializer.Deserialize<string[]>(File.ReadAllText(args[0]))!) {
    try {
        var text=CompositionText.ToKatakana(reading);
        var found=dictionary.Find(reading);
        var expected=found is {} f ? text[..f.Start]+f.Right+text[(f.Start+f.Length)..] : null;
        Console.WriteLine(JsonSerializer.Serialize(new { reading, expected, error=(string?)null }));
    } catch(ArgumentException) {
        // Keep original failures as evidence; do not invent a normal result.
        Console.WriteLine(JsonSerializer.Serialize(new { reading, expected=(string?)null, error="ArgumentException" }));
    }
}
