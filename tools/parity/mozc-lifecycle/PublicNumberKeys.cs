// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Detection;
internal static class PublicNumberKeys {
    internal static void Write(string path) {
        foreach(var row in JsonDocument.Parse(File.ReadAllText(path)).RootElement.EnumerateArray()) {
            var converter=new Converter();
            var settings=new Settings { LiveConversion=false,PredictiveCandidates=false,LearningEnabled=false,DetectionLevel=DetectionLevel.Manual };
            var session=new MeltypeSession(CompositionDetector.CreateDefault(),converter,new CompositionOptions {
                LiveConversion=()=>false,Learning=()=>false,Level=()=>DetectionLevel.Manual,Candidates=CandidateDictionary.Load(null),MoreCandidates=reading=>Candidates(reading)
            },()=>settings);
            var keys=new List<int[]>();var expected=new List<JsonElement>();
            void Press(int vk,int ch=0,int mods=0) {
                keys.Add([vk,ch,mods]);
                var result=session.HandleKey(vk,ch==0?null:(char)ch,(mods&1)!=0,false,false,false);
                expected.Add(JsonDocument.Parse(result.ToJson()).RootElement.Clone());
            }
            foreach(var ch in row.GetProperty("raw").GetString()!)Press(char.ToUpperInvariant(ch),ch);
            Press(32,32);
            for(int i=0;i<row.GetProperty("active").GetInt32();i++)Press(39);
            for(int i=0;i<row.GetProperty("start").GetInt32();i++)Press(40);
            var digit=row.GetProperty("digit").GetInt32();
            Press((row.GetProperty("keypad").GetBoolean()?0x60:0x30)+digit,'0'+digit);
            Press(13);
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
