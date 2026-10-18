// SPDX-License-Identifier: GPL-3.0-or-later
using System.Reflection;
using System.Text.Json;
using Meltype.Composition;
using Meltype.Input;
using Meltype.Detection;
var cases=JsonSerializer.Deserialize<Case[]>(File.ReadAllText(args[0]))!;
foreach(var c in cases) {
 var detector=CompositionDetector.CreateDefault(); detector.SpellChecker=BuiltInWordChecker.Shared;
 var host=new Host { Selection=new(c.text,c.reading??c.text), Document=c.text };
 var gate=new CaptureGate(()=>{});
 var controller=new CompositionController(gate,detector,new Converter(c.engine),host,new CompositionOptions {
  DirectMode=()=>c.direct, LiveConversion=()=>false, Learning=()=>false,
  RomajiTypos=RomajiTypoCorrector.Load(detector.Romaji), Candidates=CandidateDictionary.Load(null),
  ContextRules=ContextRules.Load(null), History=new ConversionHistory(null),
 });
 var converted=(string)typeof(CompositionController).GetMethod("ConvertRomajiText",BindingFlags.Instance|BindingFlags.NonPublic)!.Invoke(controller,[c.text])!;
 void Press(int vk) { gate.OnKey(new(vk,0,false,false,false,0),_=>true); gate.OnKey(new(vk,0,false,true,false,1),_=>true); controller.Pump(); }
 Press(28);
 var start=host.View;
 if(c.action=="original") controller.SelectCandidate(1);
 if(c.action=="stale") host.Selection=new("別の選択","べつのせんたく");
 Press(c.action=="escape"?27:13);
 Console.WriteLine(JsonSerializer.Serialize(new { @case=c,converted,start,after=host.View,document=host.Document,replacements=host.Replacements }));
}
sealed record Case(string name,string text,string? reading=null,bool direct=false,string engine="identity",string action="enter");
sealed class Converter(string engine):IKanjiConverter {
 public string? Convert(string reading)=>engine=="bracket"?"【"+reading+"】":reading;
 public IReadOnlyList<ConversionClause>? ConvertClauses(string reading,string? context=null)=>[new(reading,Convert(reading)!)];
}
sealed class Host:ICompositionHost {
 public ReconversionSelection? Selection {get;set;}
 public string Document {get;set;}="";
 public CompositionView? View {get;private set;}
 public List<string> Replacements {get;}=[];
 public ReconversionSelection? GetReconversionSelection()=>Selection;
 public bool TryReplaceSelection(ReconversionSelection selection,string text) {
  if(!ReferenceEquals(Selection,selection))return false;
  Replacements.Add(text);Document=text;Selection=null;return true;
 }
 public void CommitText(string text)=>Document+=text;
 public void DeleteBackward(int count)=>Document=Document[..Math.Max(0,Document.Length-count)];
 public void Replay(KeyEvent e) {}
 public void Replay(MouseButtonEvent e) {}
 public char? CharFromKey(KeyEvent e,bool shift)=>null;
 public void RequestSurroundingText(Action<string?,string?> callback)=>callback(null,null);
 public void Show(CompositionView view)=>View=view;
 public void Hide()=>View=null;
}
