// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Detection;
internal static class CaseKeys {
    internal static void Write(string path) {
        foreach(var row in JsonDocument.Parse(File.ReadAllText(path)).RootElement.EnumerateArray()) {
            var detector=CompositionDetector.CreateDefault();detector.SpellChecker=BuiltInWordChecker.Shared;
            var level=Enum.Parse<DetectionLevel>(row.GetProperty("level").GetString()!);
            var settings=new Settings { DetectionLevel=level,LiveConversion=false,PredictiveCandidates=false,LearningEnabled=false,SigilWordsDirect=false };
            var options=new CompositionOptions { Learning=()=>false,Level=()=>level,LiveConversion=()=>false,RomajiTypos=RomajiTypoCorrector.Load(detector.Romaji) };
            var session=new MeltypeSession(detector,new Converter(),options,()=>settings);
            foreach(var ch in row.GetProperty("raw").GetString()!)session.HandleKey(char.ToUpperInvariant(ch),ch,char.IsUpper(ch),false,false,false);
            var expected=new List<object>();
            foreach(var key in row.GetProperty("keys").EnumerateArray()) {
                var result=session.HandleKey(key.GetInt32(),null,false,false,false,false);
                expected.Add(new { consumed=result.Consumed,text=result.View?.Text,converting=result.View?.Converting??false });
            }
            Console.WriteLine(JsonSerializer.Serialize(new { @case=row,expected }));
        }
    }
    private sealed class Converter:IKanjiConverter {
        public string? Convert(string reading)=>reading;
        public IReadOnlyList<ConversionClause>? ConvertClauses(string reading,string? context=null)=>[new(reading,reading)];
    }
}
