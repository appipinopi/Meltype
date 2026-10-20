// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        if (args.Length > 1 && args[1] == "--synthetic") { Synthetic.Write(args[0]); return; }
        var cases = JsonSerializer.Deserialize<Case[]>(File.ReadAllText(args[0]))!;
        // The real worker keeps its converters on one STA. Optional reuse
        // measures that lifecycle without changing the upstream API methods.
        using var sharedEngine = args.Contains("--reuse") ? new MsImeKanjiConverter() : null;
        using var sharedExtras = args.Contains("--reuse") ? new WinRtCandidates() : null;
        foreach (var c in cases)
        {
            using var ownedEngine = sharedEngine is null ? new MsImeKanjiConverter() : null;
            using var ownedExtras = sharedExtras is null ? new WinRtCandidates() : null;
            var engine = sharedEngine ?? ownedEngine!;
            var extras = sharedExtras ?? ownedExtras!;
            var converted = engine.Convert(c.text);
            var clauses = engine.ConvertClauses(c.text, c.context)?.Select(s => new { reading = s.Reading, text = s.Text });
            var candidates = extras.Get(c.text);
            // WinRT has a deliberate 500-ms budget. Keep a complete result if
            // the original API supplies one on a bounded repeat; never derive
            // expected candidates from the Rust implementation.
            for (var attempt = 1; attempt < 3 && candidates.Count == 0 && c.text.Length > 0; ++attempt)
                candidates = extras.Get(c.text);
            Console.WriteLine(JsonSerializer.Serialize(new { c.name, available = engine.IsAvailable, converted, clauses, candidates }));
        }
    }
    private sealed record Case(string name, string text, string? context = null);
}
