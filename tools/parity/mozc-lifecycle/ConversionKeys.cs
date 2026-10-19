// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Detection;
internal static class ConversionKeys {
    internal static void Write(string path) {
        foreach(var row in JsonDocument.Parse(File.ReadAllText(path)).RootElement.EnumerateArray()) {
            var converter=new Converter();
            var settings=new Settings { LiveConversion=false,PredictiveCandidates=false,LearningEnabled=false,DetectionLevel=DetectionLevel.Manual,TabConversion=row.GetProperty("tab").GetBoolean() };
            var session=new MeltypeSession(CompositionDetector.CreateDefault(),converter,new CompositionOptions {
                LiveConversion=()=>false,Learning=()=>false,Level=()=>DetectionLevel.Manual,Candidates=CandidateDictionary.Load(null),TabConversion=()=>settings.TabConversion,MoreCandidates=reading=>Candidates(reading)
            },()=>settings);
            var keys=new List<int[]>();var expected=new List<JsonElement>();
            void Press(int vk,int ch=0,int mods=0) {
                keys.Add([vk,ch,mods]);
                var result=session.HandleKey(vk,ch==0?null:(char)ch,(mods&1)!=0,(mods&2)!=0,(mods&4)!=0,(mods&8)!=0);
                expected.Add(JsonDocument.Parse(result.ToJson()).RootElement.Clone());
            }
            foreach(var ch in row.GetProperty("raw").GetString()!)Press(char.ToUpperInvariant(ch),ch);
            foreach(var key in row.GetProperty("keys").EnumerateArray()) {
                var values=key.EnumerateArray().Select(x=>x.GetInt32()).ToArray();
                Press(values[0],values[1],values[2]);
            }
            Console.WriteLine(JsonSerializer.Serialize(new { @case=row,keys,expected }));
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
