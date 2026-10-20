// SPDX-License-Identifier: GPL-3.0-or-later
// Read the unmodified 1.3.0 Core in an isolated source checkout.
using System.Reflection;
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Input;
Console.OutputEncoding=System.Text.Encoding.UTF8;

if(args[0]=="--settings") {
    Directory.CreateDirectory(args[2]);
    var index=0;
    foreach(var row in JsonDocument.Parse(File.ReadAllText(args[1])).RootElement.EnumerateArray()) {
        var path=Path.Combine(args[2],"settings-"+(index++)+".json");File.WriteAllText(path,row.GetProperty("config").GetRawText());
        var settings=Settings.Load(path);var trace=new List<object>();
        void Record(bool accepted)=>trace.Add(new { accepted,settings=JsonDocument.Parse(settings.ToJson()).RootElement.Clone() });
        Record(true);
        if(row.TryGetProperty("ops",out var ops))foreach(var op in ops.EnumerateArray()) {
            var name=op.GetProperty("name").GetString()!;Settings? next=op.GetProperty("op").GetString() switch {
                "add"=>settings.AddProfile(name),"switch"=>settings.ProfileNames.Contains(name)?settings.SwitchProfile(name):null,
                "rename"=>settings.RenameProfile(name,op.GetProperty("newName").GetString()!),"remove"=>settings.RemoveProfile(name),
                _=>throw new ArgumentException() };
            if(next is not null)settings=next;Record(next is not null);
        }
        Console.WriteLine(JsonSerializer.Serialize(new {name=row.GetProperty("name").GetString(),expected=trace}));
    }
    return;
}

if (args[0] == "--metadata") {
 var props=typeof(Settings).GetProperties().Where(p=>p.GetCustomAttribute<System.ComponentModel.CategoryAttribute>()!=null).Select(p=>new {key=p.Name,type=p.PropertyType.Name,category=p.GetCustomAttribute<System.ComponentModel.CategoryAttribute>()!.Category,label=p.GetCustomAttribute<System.ComponentModel.DisplayNameAttribute>()?.DisplayName??p.Name,description=p.GetCustomAttribute<System.ComponentModel.DescriptionAttribute>()?.Description??""});
 Console.WriteLine(JsonSerializer.Serialize(props));return;
}
if (args[0] == "--sessions") { SessionReference.Run(args[1], args[2]); return; }
if (args[0] == "--defaults") { Console.WriteLine(new Settings().Normalize().ToJson()); return; }
var controller = typeof(Calculator).Assembly.GetType("Meltype.Composition.CompositionController")!;
foreach (var row in JsonDocument.Parse(File.ReadAllText(args[0])).RootElement.EnumerateArray())
{
    var kind = row.GetProperty("kind").GetString()!;
    var text = row.TryGetProperty("text", out var t) ? t.GetString()! : "";
    object? value = kind switch
    {
        "calculator" => Calculator.Evaluate(text),
        "number" => NumberCandidates.For(text),
        "github-normalize" => GitHubIssueTitles.Normalize(text),
        "github-valid" => GitHubIssueTitles.IsRepository(text),
        "names" => new Predictor(null,null,null).PredictEnglishNames(text),
        "shortcuts" => KeyShortcut.ParseList(text),
        _ => controller.GetMethod(kind switch {
            "era" => "EraToYear", "unit" => "TypedUnitCandidates",
            "seasonal" => "SeasonalGreetings", "clipboard" => "ClipboardLine",
            _ => throw new InvalidDataException(kind)
        }, BindingFlags.Static | BindingFlags.NonPublic)!.Invoke(null,
            [kind == "seasonal" ? row.GetProperty("month").GetInt32() : text])
    };
    Console.WriteLine(JsonSerializer.Serialize(new { request = row, expected = value }));
}
