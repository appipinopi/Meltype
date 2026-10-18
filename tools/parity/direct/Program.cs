// SPDX-License-Identifier: GPL-3.0-or-later
// Reference-only timed replay. No IME registration or real application input.
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Detection;

var cases = JsonSerializer.Deserialize<Case[]>(File.ReadAllText(args[0]))!;
foreach (var item in cases)
{
    var settings = new Settings { LiveConversion = false, PredictiveCandidates = false,
        LearningEnabled = false, DirectModeAutoDetect = true }.Normalize();
    var detector = CompositionDetector.CreateDefault();
    detector.SpellChecker = BuiltInWordChecker.Shared;
    var engine = ScoreEngine.CreateDefault(null, () => settings);
    long now = 0;
    var session = new MeltypeSession(detector, new NoConverter(), new CompositionOptions
    {
        LiveConversion = () => false, Learning = () => false,
        ClassifyDirect = (letters, final) => engine.Evaluate(new DetectionInput(letters,
            letters.Select(c => (int)char.ToUpperInvariant(c)).ToArray(), final)).Verdict,
    }, () => settings, directModeDetection: true, timeMs: () => now) { Direct = true };
    session.CanDeleteSurrounding = true;
    var document = item.before ?? "";
    var responses = new List<object>();
    foreach (var step in item.steps)
    {
        now = step.time;
        if (step.before is not null) document = step.before;
        if (step.moved) session.ForgetDirectInput();
        if (step.direct is { } direct) session.Direct = direct;
        var result = session.HandleKey(step.vk, step.ch == 0 ? null : (char)step.ch,
            (step.mods & 1) != 0, (step.mods & 2) != 0, (step.mods & 4) != 0, (step.mods & 8) != 0,
            document);
        foreach (var edit in result.Commits)
        {
            if (edit.DeleteBefore > 0)
            {
                if (edit.Expect is null || !document.EndsWith(edit.Expect) || edit.Expect.Length != edit.DeleteBefore)
                    throw new InvalidOperationException("unsafe reference edit");
                document = document[..^edit.DeleteBefore];
            }
            document += edit.Text;
        }
        if (!result.Consumed && step.ch >= 32) document += (char)step.ch;
        responses.Add(new { response = JsonDocument.Parse(result.ToJson()).RootElement,
            direct = session.Direct, english = session.DirectEnglishWord, document });
    }
    Console.WriteLine(JsonSerializer.Serialize(new { item.name, responses }));
}
record Case(string name, Step[] steps, string? before = null);
record Step(int vk, int ch, int mods, long time, string? before = null, bool moved = false, bool? direct = null);
sealed class NoConverter : IKanjiConverter
{
    public string? Convert(string reading) => null;
    public IReadOnlyList<ConversionClause>? ConvertClauses(string reading, string? context = null) => null;
}
