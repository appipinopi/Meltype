// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Detection;
internal static class PredictionKeys {
    internal static void Write(string path) {
        foreach(var row in JsonDocument.Parse(File.ReadAllText(path)).RootElement.EnumerateArray()) {
            var directory=Path.Combine(Path.GetTempPath(),"meltype-predict-ref-"+Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(directory);
            var historyPath=Path.Combine(directory,"phrases.txt");
            File.WriteAllLines(historyPath,row.GetProperty("phrases").EnumerateArray().Select(p=>$"{p.GetProperty("reading").GetString()}\t{p.GetProperty("text").GetString()}\t{p.GetProperty("count").GetInt32()}\t{p.GetProperty("used").GetInt64()}"));
            try {
            var history=new PhraseHistory(historyPath);
            var converter=new Converter();
            var level=Enum.Parse<DetectionLevel>(row.GetProperty("level").GetString()!);
            var learning=row.GetProperty("learning").GetBoolean();
            var detector=CompositionDetector.CreateDefault();detector.SpellChecker=BuiltInWordChecker.Shared;
            var settings=new Settings { LiveConversion=false,PredictiveCandidates=true,LearningEnabled=learning,DetectionLevel=level,TabConversion=false };
            var session=new MeltypeSession(detector,converter,new CompositionOptions {
                LiveConversion=()=>false,Learning=()=>learning,Level=()=>level,Predictions=()=>true,Predictor=new Predictor(history,null,null),RomajiTypos=RomajiTypoCorrector.Load(detector.Romaji),Candidates=CandidateDictionary.Load(null),TabConversion=()=>settings.TabConversion,MoreCandidates=reading=>Candidates(reading)
            },()=>settings);
            var keys=new List<int[]>();var expected=new List<JsonElement>();
            void Press(int vk,int ch=0,int mods=0) {
                keys.Add([vk,ch,mods]);
                var result=vk==-1?session.SelectPrediction(ch):session.HandleKey(vk,ch==0?null:(char)ch,(mods&1)!=0,(mods&2)!=0,(mods&4)!=0,(mods&8)!=0);
                expected.Add(JsonDocument.Parse(result.ToJson()).RootElement.Clone());
            }
            foreach(var ch in row.GetProperty("raw").GetString()!)Press(char.ToUpperInvariant(ch),ch,char.IsUpper(ch)?1:0);
            foreach(var key in row.GetProperty("keys").EnumerateArray()) {
                var values=key.EnumerateArray().Select(x=>x.GetInt32()).ToArray();
                Press(values[0],values[1],values[2]);
            }
            var phrasesAfter=File.ReadAllLines(historyPath).Select(line=>line.Split('\t')).Select(p=>new {reading=p[0],text=p[1],count=int.Parse(p[2])}).OrderBy(p=>p.reading,StringComparer.Ordinal).ThenBy(p=>p.text,StringComparer.Ordinal).ToArray();
            Console.WriteLine(JsonSerializer.Serialize(new { @case=row,keys,expected,phrasesAfter }));
            } finally { File.Delete(historyPath); Directory.Delete(directory); }
        }
    }
    private static string Text(string reading,int n)=>$"【{reading}{n}】";
    private static string[] Candidates(string reading)=>Enumerable.Range(0,20).Select(n=>Text(reading,n)).ToArray();
    private sealed class Converter:IKanjiConverter {
        public string? Convert(string reading)=>Text(reading,0);
        public IReadOnlyList<ConversionClause>? ConvertClauses(string reading,string? context=null)=>
            (reading=="はしかみ"?new[]{"はし","かみ"}:new[]{reading}).Select(r=>new ConversionClause(r,Text(r,0))).ToArray();
    }
}
