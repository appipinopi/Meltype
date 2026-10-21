// SPDX-License-Identifier: GPL-3.0-or-later
using System.Collections;
using System.Reflection;
using System.Text.Json;
using Meltype.Composition;
using Meltype.Detection;
using Meltype.Input;
internal static class Filter {
    internal static void Write(string path) {
        // LearnConversion enqueues runs. A single worker and a final sentinel
        // give a deterministic observation after every queued run has executed.
        if (!ThreadPool.SetMinThreads(1, 1) || !ThreadPool.SetMaxThreads(1, 1)) throw new Exception("ThreadPool setup failed");
        foreach (var row in JsonDocument.Parse(File.ReadAllText(path)).RootElement.EnumerateArray()) {
            var learner = new Learner();
            var controller = new CompositionController(new CaptureGate(()=>{}), CompositionDetector.CreateDefault(), learner, new Host(), new CompositionOptions { Learning=()=>row.GetProperty("learning").GetBoolean() && !row.GetProperty("private").GetBoolean() });
            var type=typeof(CompositionController);
            var clauseType=type.GetNestedType("Clause",BindingFlags.NonPublic)!;
            var list=(IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(clauseType))!;
            foreach(var c in row.GetProperty("clauses").EnumerateArray()) {
                var clause=Activator.CreateInstance(clauseType, [c.GetProperty("reading").GetString()!, c.GetProperty("english").GetBoolean(), new List<string>{c.GetProperty("text").GetString()!}])!;
                clauseType.GetProperty("Raw")!.SetValue(clause,c.GetProperty("raw").GetString());
                var translations=(HashSet<string>)clauseType.GetProperty("Translations")!.GetValue(clause)!;
                foreach(var t in c.GetProperty("translations").EnumerateArray())translations.Add(t.GetString()!);
                list.Add(clause);
            }
            type.GetField("_clauses",BindingFlags.Instance|BindingFlags.NonPublic)!.SetValue(controller,list);
            type.GetField("_precedingText",BindingFlags.Instance|BindingFlags.NonPublic)!.SetValue(controller,row.GetProperty("before").GetString());
            type.GetMethod("Learn",BindingFlags.Instance|BindingFlags.NonPublic)!.Invoke(controller,null);
            using var done=new ManualResetEventSlim();ThreadPool.QueueUserWorkItem(_=>done.Set());
            if(!done.Wait(5000))throw new Exception("Learning did not finish");
            Console.WriteLine(JsonSerializer.Serialize(new { @case=row, expected=learner.batches }));
        }
    }
    private sealed class Learner:IKanjiConverter,ILearningConverter {
        public readonly List<object> batches=[];
        public string? Convert(string reading)=>reading;
        public IReadOnlyList<ConversionClause>? ConvertClauses(string reading,string? context=null)=>[new(reading,reading)];
        public void Learn(string? context,IReadOnlyList<ConversionClause> clauses)=>batches.Add(new {context=context??"",clauses=clauses.Select(c=>new {reading=c.Reading,text=c.Text}).ToArray()});
    }
    private sealed class Host:ICompositionHost {
        public void CommitText(string text){} public void DeleteBackward(int count){} public void Replay(KeyEvent e){} public void Replay(MouseButtonEvent e){}
        public char? CharFromKey(KeyEvent e,bool shift)=>null;
        public void RequestSurroundingText(Action<string?,string?> callback)=>callback(null,null);
        public void Show(CompositionView view){} public void Hide(){}
    }
}
