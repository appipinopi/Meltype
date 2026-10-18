// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Detection;
var cases = JsonSerializer.Deserialize<Case[]>(File.ReadAllText(args[0]))!;
foreach (var c in cases) {
 var detector = CompositionDetector.CreateDefault();
 detector.SpellChecker = BuiltInWordChecker.Shared;
 var settings = new Settings { LiveConversion=false, PredictiveCandidates=false, LearningEnabled=false };
 var session = new MeltypeSession(detector, new Converter(c.failure), new CompositionOptions {
  LiveConversion=()=>false, Learning=()=>false, MoreCandidates=r=>[r+"★"]
 }, ()=>settings);
 session.CanDeleteSurrounding=true;
 var results = new List<JsonElement>();
 foreach(var k in c.keys) results.Add(JsonSerializer.Deserialize<JsonElement>(session.HandleKey(k[0], k[1]==0?null:(char)k[1], (k[2]&1)!=0, false,false,false).ToJson()));
 Console.WriteLine(JsonSerializer.Serialize(new { name=c.name, keys=c.keys, failure=c.failure, expected=results }));
}
sealed record Case(string name,int[][] keys,bool failure=false);
sealed class Converter(bool failure) : IKanjiConverter {
 public string? Convert(string reading)=>failure && reading=="あいうえおか" ? null : "【"+reading+"】";
 public IReadOnlyList<ConversionClause>? ConvertClauses(string reading,string? context=null)=>reading=="あいうえおかきくけこ"
  ? [new("あいうえお","【あいうえお】"),new("かきくけこ","【かきくけこ】")]
  : reading=="あいうえおかきくけこさしす"
  ? [new("あいうえお","【あいうえお】"),new("かきくけこ","【かきくけこ】"),new("さしす","【さしす】")]
  : [new(reading,Convert(reading)!)];
}
