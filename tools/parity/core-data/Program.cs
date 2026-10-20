// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Detection;
using Meltype.Learning;

var mode=args[0];var directory=Path.GetFullPath(args[1]);
Directory.CreateDirectory(directory);
string FileAt(string name)=>Path.Combine(directory,name);
if(mode=="seed") {
    new Settings { LiveConversion=false,PredictiveCandidates=false,LearningEnabled=false }.Normalize().Save(FileAt("config.json"));
    var language=new LanguageMemory(FileAt("languages.json"));
    language.Remember("review",true,true);language.Remember("sushi",true,true);language.Remember("sushi",true,true);language.Remember("go",false,true);
    var conversions=new ConversionHistory(FileAt("conversions.json"));conversions.Remember("はし","箸");conversions.Remember("よてい","予定");
    var translations=new TranslationHistory(FileAt("translations.json"));translations.Remember("き","tree");translations.Remember("き","tree");translations.Remember("はし","bridge");
    var words=new UserDictionary(FileAt("userdict.txt"),builtIn:false);words.Add("めると","Meltype");words.Add("はし","箸");words.Add("はし","橋");
    var phrases=new PhraseHistory(FileAt("phrases.txt"));phrases.Remember("きょうのよてい","今日の予定");phrases.Remember("きょうのよてい","今日の予定");phrases.Remember("きょうのてんき","今日の天気");
    var snippets=new SnippetStore(FileAt("snippets.json"));snippets.Set("sig","署名\r\nMeltype 😀");
    var model=new UserModel(FileAt("model.json"));model.Learn("github",SessionOutcome.StayedAccepted,true);model.Learn("nihongo",SessionOutcome.JapaneseAccepted,false);model.Save();
    return;
}
if(mode=="backup") { File.WriteAllBytes(args[2],Backup.Create(directory,"upstream-test").Content);return; }
if(mode=="restore") { Console.WriteLine(Backup.Restore(File.ReadAllBytes(args[2]),directory));return; }
if(mode=="snapshot") {
    var detector=CompositionDetector.CreateDefault();var corrector=RomajiTypoCorrector.Load(detector.Romaji);
    var languages=new LanguageMemory(FileAt("languages.json")) { IsCommonJapanese=corrector.IsCommonJapanese,IsReadableRomaji=word=>detector.Romaji.Analyze(word).IsValid };
    var language=languages.Entries().OrderBy(x=>x.Word,StringComparer.Ordinal).ToDictionary(x=>x.Word,x=>new { english=x.English,count=Math.Max(1,x.Count),active=x.Active });
    var conversions=new ConversionHistory(FileAt("conversions.json")).Entries().ToDictionary(x=>x.Reading,x=>x.Text);
    var th=new TranslationHistory(FileAt("translations.json"));var translations=new SortedDictionary<string,Dictionary<string,int>>(StringComparer.Ordinal);
    if(File.Exists(FileAt("translations.json")))foreach(var reading in JsonDocument.Parse(File.ReadAllText(FileAt("translations.json"))).RootElement.EnumerateObject())translations[reading.Name]=th.Get(reading.Name).ToDictionary(x=>x.Word,x=>x.Count);
    var words=new UserDictionary(FileAt("userdict.txt"),builtIn:false);var dictionary=words.Words.Select(x=>x.Reading).Distinct().Order(StringComparer.Ordinal).ToDictionary(x=>x,x=>words.Lookup(x));
    var phrases=new PhraseHistory(FileAt("phrases.txt"));var predictions=new Dictionary<string,IEnumerable<string>>();foreach(var prefix in new[]{"きょ","きょう","よて"})predictions[prefix]=phrases.StartingWith(prefix).ToArray();
    var model=new UserModel(FileAt("model.json"));var prefixes=new Dictionary<string,object>();foreach(var key in new[]{"github","nihong"})if(model.Get(key) is {} stats)prefixes[key]=new { japanese=stats.Japanese,english=stats.English };
    Console.WriteLine(JsonSerializer.Serialize(new { language,conversions,translations,dictionary,predictions,phraseCount=phrases.Count,snippets=new SnippetStore(FileAt("snippets.json")).Entries().ToDictionary(x=>x.Name,x=>x.Text),prefixes }));return;
}
if(mode=="settings") {
    var index=0;
    foreach(var row in JsonDocument.Parse(File.ReadAllText(args[2])).RootElement.EnumerateArray()) {
        var path=FileAt("settings-"+(index++)+".json");File.WriteAllText(path,row.GetProperty("config").GetRawText());
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
if(mode=="keys") {
    var rows=JsonDocument.Parse(File.ReadAllText(args[2])).RootElement;
    var index=0;
    foreach(var row in rows.EnumerateArray()) {
        var profile=Path.Combine(directory,(index++).ToString());Directory.CreateDirectory(profile);
        File.WriteAllText(Path.Combine(profile,"config.json"),row.GetProperty("settings").GetRawText());
        if(row.TryGetProperty("files",out var files))foreach(var file in files.EnumerateObject()) {
            var path=Path.GetFullPath(Path.Combine(profile,file.Name));
            if(!path.StartsWith(profile+Path.DirectorySeparatorChar,StringComparison.Ordinal))throw new InvalidDataException();
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);File.WriteAllText(path,file.Value.GetString());
        }
        var session=MeltypeSession.CreateDefault(profile,new NullConverter(),null,BuiltInWordChecker.Shared);session.CanDeleteSurrounding=row.GetProperty("canDelete").GetBoolean();
        var output=new List<JsonElement>();
        foreach(var key in row.GetProperty("keys").EnumerateArray()) {
            var k=key.EnumerateArray().Select(x=>x.GetInt32()).ToArray();
            var reply=session.HandleKey(k[0],k[1]==0?null:(char)k[1],(k[2]&1)!=0,(k[2]&2)!=0,(k[2]&4)!=0,(k[2]&8)!=0,row.GetProperty("before").GetString(),row.GetProperty("after").GetString());
            output.Add(JsonDocument.Parse(reply.ToJson()).RootElement.Clone());
        }
        Console.WriteLine(JsonSerializer.Serialize(new {name=row.GetProperty("name").GetString(),expected=output}));
    }
    return;
}
throw new ArgumentException(mode);
sealed class NullConverter:IKanjiConverter {
    public string? Convert(string reading)=>null;
    public IReadOnlyList<ConversionClause>? ConvertClauses(string reading,string? context=null)=>null;
}
