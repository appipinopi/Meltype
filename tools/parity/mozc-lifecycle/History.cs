// SPDX-License-Identifier: GPL-3.0-or-later
using System.Collections;
using System.Reflection;
using System.Text.Json;
using Meltype.Composition;
using Meltype.Detection;
using Meltype.Input;
internal static class History {
    internal static void Write(string path) {
        foreach(var row in JsonDocument.Parse(File.ReadAllText(path)).RootElement.EnumerateArray()) {
            var history=new ConversionHistory(null);
            var translations=new TranslationHistory(null);
            var detector=CompositionDetector.CreateDefault();
            var controller=new CompositionController(new CaptureGate(()=>{}),detector,new Converter(),new Host(),new CompositionOptions {
                Learning=()=>row.GetProperty("learning").GetBoolean()&&!row.GetProperty("private").GetBoolean(), History=history, TranslationHistory=translations
            });
            var type=typeof(CompositionController);
            var clauseType=type.GetNestedType("Clause",BindingFlags.NonPublic)!;
            var list=(IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(clauseType))!;
            var clause=Activator.CreateInstance(clauseType,[row.GetProperty("reading").GetString()!,row.GetProperty("english").GetBoolean(),new List<string>{row.GetProperty("text").GetString()!}])!;
            clauseType.GetProperty("Raw")!.SetValue(clause,row.GetProperty("raw").GetString());
            clauseType.GetProperty("Changed")!.SetValue(clause,row.GetProperty("changed").GetBoolean());
            foreach(var t in row.GetProperty("translations").EnumerateArray()) ((HashSet<string>)clauseType.GetProperty("Translations")!.GetValue(clause)!).Add(t.GetString()!);
            list.Add(clause);
            type.GetField("_clauses",BindingFlags.Instance|BindingFlags.NonPublic)!.SetValue(controller,list);
            for(int i=0;i<row.GetProperty("repeats").GetInt32();i++) type.GetMethod("Learn",BindingFlags.Instance|BindingFlags.NonPublic)!.Invoke(controller,null);
            Console.WriteLine(JsonSerializer.Serialize(new { @case=row, expected=new { selected=history.Entries().ToDictionary(e=>e.Reading,e=>e.Text), translations=translations.Get(row.GetProperty("reading").GetString()!).ToDictionary(e=>e.Word,e=>e.Count) } }));
        }
    }
    private sealed class Converter:IKanjiConverter {
        public string? Convert(string reading)=>reading;
        public IReadOnlyList<ConversionClause>? ConvertClauses(string reading,string? context=null)=>[new(reading,reading)];
    }
    private sealed class Host:ICompositionHost {
        public void CommitText(string text){} public void DeleteBackward(int count){} public void Replay(KeyEvent e){} public void Replay(MouseButtonEvent e){}
        public char? CharFromKey(KeyEvent e,bool shift)=>null;
        public void RequestSurroundingText(Action<string?,string?> callback)=>callback(null,null);
        public void Show(CompositionView view){} public void Hide(){}
    }
}
