// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Config;
var rows=JsonDocument.Parse(File.ReadAllText(args[0])).RootElement;
Directory.CreateDirectory(args[1]);var index=0;
foreach(var row in rows.EnumerateArray()) {
    var path=Path.Combine(args[1],(index++)+".json");File.WriteAllText(path,row.GetProperty("settings").GetRawText());
    var settings=Settings.Load(path);var process=row.GetProperty("process").GetString();var looksLikeGame=row.GetProperty("looksLikeGame").GetBoolean();
    var app=settings.ForApp(process);
    Console.WriteLine(JsonSerializer.Serialize(new {name=row.GetProperty("name").GetString(),enabled=settings.Enabled&&settings.IsAppEnabled(process)&&!settings.IsGame(process,looksLikeGame),profile=settings.ProfileFor(process).ToString(),level=app.DetectionLevel.ToString(),live=app.LiveConversion,startInEnglish=settings.KindFor(process)?.StartInEnglish??false}));
}
