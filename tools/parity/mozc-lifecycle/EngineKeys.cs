// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Detection;
internal static class EngineKeys {
    internal static void Write(string path) {
        if(!ThreadPool.SetMinThreads(1,1)||!ThreadPool.SetMaxThreads(1,1))throw new Exception("ThreadPool setup failed");
        foreach(var row in JsonDocument.Parse(File.ReadAllText(path)).RootElement.EnumerateArray()) {
            var learner=new Learner();
            var detector=CompositionDetector.CreateDefault();detector.SpellChecker=BuiltInWordChecker.Shared;
            var separate=row.TryGetProperty("separate",out var mode)?mode.GetBoolean():row.GetProperty("action").GetString()=="convert";
            var level=row.TryGetProperty("level",out var detection)?Enum.Parse<DetectionLevel>(detection.GetString()!):DetectionLevel.Manual;
            var settings=new Settings { DetectionLevel=level, LiveConversion=false, PredictiveCandidates=false, SeparateKeyRoles=separate };
            var options=new CompositionOptions { Learning=()=>row.GetProperty("learning").GetBoolean(), Level=()=>level,
                SeparateKeyRoles=()=>separate, LiveConversion=()=>false, RomajiTypos=RomajiTypoCorrector.Load(detector.Romaji) };
            var session=new MeltypeSession(detector,learner,options,()=>settings) { Private=row.GetProperty("private").GetBoolean() };
            var committed=new List<string>();
            SessionResult Press(int vk,char? ch=null,bool shift=false) {
                var result=session.HandleKey(vk,ch,shift,false,false,false);
                committed.AddRange(result.Commits.Select(c=>c.Text));return result;
            }
            foreach(var c in row.GetProperty("raw").GetString()!)Press(char.ToUpperInvariant(c),c,char.IsUpper(c));
            var force=row.GetProperty("force").GetInt32();if(force!=0)Press(force);
            var converting=false;
            if(row.GetProperty("action").GetString()!="enter") {
                var convert=row.GetProperty("action").GetString()=="convert";
                var start=Press(convert?28:32,convert?null:' ',row.GetProperty("action").GetString()=="shift-space");converting=start.View?.Converting==true;
                if(converting && row.GetProperty("target").GetString() is {} target) {
                    var index=start.View!.Candidates.ToList().IndexOf(target);if(index<0)throw new Exception("Missing target "+target);
                    session.SelectCandidate(index);
                }
            }
            Press(13);
            using var done=new ManualResetEventSlim();ThreadPool.QueueUserWorkItem(_=>done.Set());
            if(!done.Wait(5000))throw new Exception("Learning did not finish");
            Console.WriteLine(JsonSerializer.Serialize(new { @case=row, expected=new {converting,committed=string.Concat(committed),batches=learner.batches} }));
        }
    }
    private sealed class Learner:IKanjiConverter,ILearningConverter {
        public readonly List<object> batches=[];
        public string? Convert(string reading)=>"【"+reading+"】";
        public IReadOnlyList<ConversionClause>? ConvertClauses(string reading,string? context=null)=>[new(reading,Convert(reading)!)];
        public void Learn(string? context,IReadOnlyList<ConversionClause> clauses)=>batches.Add(new {context=context??"",clauses=clauses.Select(c=>new {reading=c.Reading,text=c.Text}).ToArray()});
    }
}
