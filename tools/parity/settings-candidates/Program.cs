// SPDX-License-Identifier: GPL-3.0-or-later
// Wire the unmodified Core like Windows CompositionService, with controlled engines.
using System.Text.Json;
using Meltype.Composition;
using Meltype.Config;
using Meltype.Detection;

var rows=JsonDocument.Parse(File.ReadAllText(args[0])).RootElement;
var root=Path.GetFullPath(args[1]);Directory.CreateDirectory(root);
var index=0;
foreach(var row in rows.EnumerateArray()) {
    var directory=Path.Combine(root,(index++).ToString());Directory.CreateDirectory(directory);
    string At(string name)=>Path.Combine(directory,name);
    File.WriteAllText(At("config.json"),row.GetProperty("settings").GetRawText());
    if(row.TryGetProperty("files",out var files))foreach(var file in files.EnumerateObject()) {
        var path=Path.GetFullPath(At(file.Name));
        if(!path.StartsWith(directory+Path.DirectorySeparatorChar,StringComparison.Ordinal))throw new InvalidDataException();
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);File.WriteAllText(path,file.Value.GetString());
    }
    var settings=Settings.Load(At("config.json"));var userDirectory=At("dictionaries");
    var detector=CompositionDetector.CreateDefault(userDirectory);detector.SpellChecker=BuiltInWordChecker.Shared;
    detector.UseScoredSegmentation=()=>settings.ScoredSegmentation;
    detector.UseJapanesePunctuationAfterEnglish=()=>settings.JapanesePunctuationAfterEnglish;
    var languages=new LanguageMemory(At("languages.json"));detector.Memory=languages;
    var history=new ConversionHistory(At("conversions.json"));
    var user=new UserDictionary(At("userdict.txt"),importDirectory:userDirectory);
    var converter=new Converter(row);
    var options=new CompositionOptions {
        LiveConversion=()=>settings.LiveConversion,AutoCorrect=()=>settings.AutoCorrectAfterCommit && settings.DetectionLevel!=DetectionLevel.Manual,
        Level=()=>settings.DetectionLevel,KanaInput=()=>settings.InputStyle==InputStyle.Kana,
        Candidates=CandidateDictionary.Load(userDirectory),ContextRules=ContextRules.Load(userDirectory),
        History=history,UserDictionary=user,Languages=languages,MoreCandidates=converter.Candidates,
        Translations=TranslationDictionary.Load(),Meanings=MeaningDictionary.Load(),
        Misspellings=MisspellingDictionary.Load(userDirectory),RomajiTypos=RomajiTypoCorrector.Load(detector.Romaji),
        TranslationHistory=new TranslationHistory(At("translations.json")),
        TranslationCandidates=()=>settings.TranslationCandidates,CandidateMeanings=()=>settings.ShowCandidateMeanings,
        ShowTypedKeys=()=>settings.ShowTypedKeys,TabConversion=()=>settings.TabConversion,
        ContinueAfterConversion=()=>settings.ContinueAfterConversion,Learning=()=>settings.LearningEnabled,
        SeparateKeyRoles=()=>settings.SeparateKeyRoles,CorrectTypos=()=>settings.CorrectTypos,
        SlashAsMiddleDot=()=>settings.SlashAsMiddleDot,SpaceAroundEnglish=()=>settings.SpaceAroundEnglish,
        Punctuation=()=>settings.Punctuation,Predictor=new Predictor(new PhraseHistory(At("phrases.txt")),user,history),
        Predictions=()=>settings.PredictiveCandidates,
    };
    var session=new MeltypeSession(detector,converter,options,()=>settings);
    session.CanDeleteSurrounding=row.GetProperty("canDelete").GetBoolean();
    session.Private=row.TryGetProperty("private",out var privacy)&&privacy.GetBoolean();
    var expected=new List<JsonElement>();
    foreach(var key in row.GetProperty("keys").EnumerateArray()) {
        var k=key.EnumerateArray().Select(x=>x.GetInt32()).ToArray();
        var reply=session.HandleKey(k[0],k[1]==0?null:(char)k[1],(k[2]&1)!=0,(k[2]&2)!=0,(k[2]&4)!=0,(k[2]&8)!=0,row.GetProperty("before").GetString(),row.GetProperty("after").GetString());
        expected.Add(JsonDocument.Parse(reply.ToJson()).RootElement.Clone());
    }
    Console.WriteLine(JsonSerializer.Serialize(new {name=row.GetProperty("name").GetString(),expected}));
}
sealed class Converter(JsonElement row):IKanjiConverter {
    public string? Convert(string reading)=>ConvertClauses(reading)?.Select(x=>x.Text).Aggregate("",(a,b)=>a+b);
    public IReadOnlyList<ConversionClause>? ConvertClauses(string reading,string? context=null) {
        if(!row.TryGetProperty("converter",out var source)||!source.TryGetProperty("clauses",out var clauses)||!clauses.TryGetProperty(reading,out var parts))return null;
        return parts.EnumerateArray().Select(p=>new ConversionClause(p[0].GetString()!,p[1].GetString()!)).ToArray();
    }
    public IReadOnlyList<string> Candidates(string reading) {
        if(!row.TryGetProperty("converter",out var source)||!source.TryGetProperty("candidates",out var candidates)||!candidates.TryGetProperty(reading,out var words))return [];
        return words.EnumerateArray().Select(x=>x.GetString()!).ToArray();
    }
}
