// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Detection;
internal static class Choice {
    internal static void Write(string path) {
        foreach(var row in JsonDocument.Parse(File.ReadAllText(path)).RootElement.EnumerateArray()) {
            var memory=new LanguageMemory(null);
            var detector=CompositionDetector.CreateDefault();
            detector.SpellChecker=BuiltInWordChecker.Shared;
            detector.Memory=memory;
            var options=new CompositionOptions {
                Learning=()=>row.GetProperty("learning").GetBoolean(), Languages=memory,
                RomajiTypos=RomajiTypoCorrector.Load(detector.Romaji), Level=()=>DetectionLevel.Manual,
                LiveConversion=()=>false
            };
            var settings=new Settings { DetectionLevel=DetectionLevel.Manual, LiveConversion=false, PredictiveCandidates=false };
            var session=new MeltypeSession(detector,new Converter(),options,()=>settings) { Private=row.GetProperty("private").GetBoolean() };
            var raw=row.GetProperty("raw").GetString()!;
            var commits=new List<string>();
            for(int repeat=0;repeat<row.GetProperty("repeats").GetInt32();repeat++) {
                SessionResult result=null!;
                foreach(var ch in raw)result=session.HandleKey(char.ToUpperInvariant(ch),ch,char.IsUpper(ch),false,false,false);
                if(row.GetProperty("forceKana").GetBoolean())session.HandleKey(117,null,false,false,false,false);
                if(!row.GetProperty("preview").GetBoolean())result=session.HandleKey(32,' ',false,false,false,false);
                var candidates=result.View?.Candidates??throw new Exception("No candidate view: "+row.GetProperty("name").GetString());
                var index=candidates.ToList().IndexOf(row.GetProperty("target").GetString()!);
                if(index<0)throw new Exception("Raw candidate missing: "+row.GetProperty("name").GetString());
                session.SelectCandidate(index);
                var committed=session.HandleKey(13,null,false,false,false,false);
                commits.Add(string.Concat(committed.Commits.Select(c=>c.Text)));
            }
            Console.WriteLine(JsonSerializer.Serialize(new { @case=row, commits, expected=memory.Entries().Select(e=>new {word=e.Word,english=e.English,count=e.Count,active=e.Active}) }));
        }
    }
    private sealed class Converter:IKanjiConverter {
        public string? Convert(string reading)=>"【"+reading+"】";
        public IReadOnlyList<ConversionClause>? ConvertClauses(string reading,string? context=null)=>[new(reading,Convert(reading)!)];
    }
}
