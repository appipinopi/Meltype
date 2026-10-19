// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
if(args[0]=="--filter") { Filter.Write(args[1]); return; }
var actions = JsonDocument.Parse(File.ReadAllText(args[2])).RootElement;
using (var converter = new MozcConverter(Path.GetFullPath(args[0]), Path.GetFullPath(args[1]))) {
    converter.WarmUp();
    var deadline = Environment.TickCount64 + 10000;
    while (converter.ConvertClauses("じゅんび") is null) {
        if (Environment.TickCount64 > deadline) throw new Exception("Fixture startup failed");
        Thread.Sleep(10);
    }
    foreach (var action in actions.EnumerateArray()) {
        var clauses = action.GetProperty("clauses").EnumerateArray().Select(c => new ConversionClause(c.GetProperty("reading").GetString()!, c.GetProperty("text").GetString()!)).ToList();
        converter.Learn(action.GetProperty("context").GetString(), clauses);
    }
}
// Original Dispose may return immediately after Kill. Read the flushed log
// with writer sharing while Windows finishes closing the killed process.
using var log = new StreamReader(new FileStream(Path.Combine(args[1], "fixture-calls.txt"), FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete));
var trace = log.ReadToEnd().Split('\n', StringSplitOptions.RemoveEmptyEntries).Select(l => l.TrimEnd('\r')).Where(l => !l.StartsWith("C\t"));
Console.WriteLine(JsonSerializer.Serialize(new { trace }));
