// SPDX-License-Identifier: GPL-3.0-or-later
using System.Collections;
using System.Reflection;
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Detection;
internal static class NumberKeys {
    internal static void Write(string path) {
        foreach(var row in JsonDocument.Parse(File.ReadAllText(path)).RootElement.EnumerateArray()) {
            var kana=row.GetProperty("kana").GetBoolean();
            var settings=new Settings { LiveConversion=false,PredictiveCandidates=false,LearningEnabled=false,DetectionLevel=DetectionLevel.Manual };
            var session=new MeltypeSession(CompositionDetector.CreateDefault(),new Converter(),new CompositionOptions { LiveConversion=()=>false,Learning=()=>false,Level=()=>DetectionLevel.Manual,KanaInput=()=>kana },()=>settings);
            foreach(var ch in (row.GetProperty("clauses").GetInt32()==1?"hashi":"hashikami"))session.HandleKey(char.ToUpperInvariant(ch),ch,false,false,false,false);
            var controller=typeof(MeltypeSession).GetField("_controller",BindingFlags.Instance|BindingFlags.NonPublic)!.GetValue(session)!;
            var type=typeof(CompositionController);var clauseType=type.GetNestedType("Clause",BindingFlags.NonPublic)!;
            var list=(IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(clauseType))!;
            var active=row.GetProperty("active").GetInt32();
            for(int i=0;i<row.GetProperty("clauses").GetInt32();i++) {
                var values=Enumerable.Range(0,row.GetProperty("count").GetInt32()).Select(n=>$"候補{i}_{n}").ToList();
                var clause=Activator.CreateInstance(clauseType,[i==0?"はし":"かみ",false,values])!;
                clauseType.GetProperty("Expanded")!.SetValue(clause,true);
                clauseType.GetProperty("Index")!.SetValue(clause,i==active?row.GetProperty("start").GetInt32():0);
                list.Add(clause);
            }
            type.GetField("_clauses",BindingFlags.Instance|BindingFlags.NonPublic)!.SetValue(controller,list);
            type.GetField("_converting",BindingFlags.Instance|BindingFlags.NonPublic)!.SetValue(controller,true);
            type.GetField("_selectedClause",BindingFlags.Instance|BindingFlags.NonPublic)!.SetValue(controller,active);
            var digit=row.GetProperty("row").GetInt32()+1;
            var result=session.HandleKey((row.GetProperty("keypad").GetBoolean()?0x60:0x30)+digit,(char)('0'+digit),false,false,false,false);
            Console.WriteLine(JsonSerializer.Serialize(new { @case=row,expected=new { consumed=result.Consumed,commits=result.Commits.Select(e=>new {deleteBefore=e.DeleteBefore,text=e.Text,expect=e.Expect}),text=result.View?.Text,converting=result.View?.Converting??false,selectedClause=result.View?.SelectedClause??0,selectedIndex=result.View?.SelectedIndex??0 } }));
        }
    }
    private sealed class Converter:IKanjiConverter { public string? Convert(string reading)=>reading;public IReadOnlyList<ConversionClause>? ConvertClauses(string reading,string? context=null)=>[new(reading,reading)]; }
}
