// SPDX-License-Identifier: GPL-3.0-or-later
// Independent original InputSession/ScoreEngine host, with synthetic clocks.
using System.Text.Json;
using Meltype.Config;
using Meltype.Detection;
using Meltype.Input;
using Meltype.Learning;

var items = JsonSerializer.Deserialize<Case[]>(File.ReadAllText(args[0]))!;
foreach (var item in items)
{
    var settings = item.settings.ValueKind == JsonValueKind.Object
        ? item.settings.Deserialize<Settings>(new JsonSerializerOptions { Converters = { new System.Text.Json.Serialization.JsonStringEnumConverter() } })!.Normalize() : new Settings().Normalize();
    var environment = new Environment();
    var model = new UserModel(null);
    foreach (var learn in item.learn ?? [])
      for (var repeat=0;repeat<learn.repeat;repeat++)
        model.Learn(learn.letters, Enum.Parse<SessionOutcome>(learn.outcome), learn.decidedEnglish);
    var counts = (item.learn ?? []).Select(l=>UserModel.PrefixKey(l.letters)).OfType<string>().Distinct().ToDictionary(k=>k,
        k=>model.Get(k) is { } s ? new { japanese=s.Japanese, english=s.English } : null);
    var session = new InputSession(ScoreEngine.CreateDefault(model,()=>settings),()=>settings,environment);
    var responses = new List<object>();
    var idField = typeof(InputSession).GetField("_sessionId",System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!;
    foreach (var step in item.steps)
    {
        bool? consumed = null;
        List<KeyEvent>? events = null;
        environment.Flushes.Clear(); environment.Ended.Clear();
        switch (step.GetProperty("op").GetString())
        {
            case "key":
                var e = new KeyEvent(Int(step,"vk"),Int(step,"scan"),Flag(step,"extended"),
                    Flag(step,"isUp"),Flag(step,"injected"),step.GetProperty("time").GetInt64());
                environment.ModifierDown = Flag(step,"modifierDown");
                environment.Permission = !step.TryGetProperty("allowed",out var allow) || allow.GetBoolean()
                    ? CollectPermission.Allow : CollectPermission.Deny(step.TryGetProperty("reason",out var reason) ? reason.GetString()! : "");
                consumed = session.OnKey(e);
                break;
            case "timer": session.OnTimer(step.GetProperty("time").GetInt64()); break;
            case "context": session.OnContextChanged(step.GetProperty("time").GetInt64()); break;
            case "take": events = session.TakePendingForFlush(); break;
            case "abort": events = session.Abort(); break;
            case "snapshot": break;
            default: throw new InvalidOperationException("unsupported test operation");
        }
        // Read the original id, including non-learnable sessions. Do not infer
        // internal transitions from the output. This reference host uses JIT.
        var sessionId = (long)idField.GetValue(session)!;
        responses.Add(new { sessionId,state=session.State.ToString(),decision=Decision(session.LastDecision),consumed,
            events=events?.Select(Event).ToArray(),
            flushes=environment.Flushes.Select(f=>new {sessionId=f.SessionId,result=Decision(f.Result)}).ToArray(),
            ended=environment.Ended.Select(s=>new {sessionId=s.SessionId,letters=s.Letters,verdict=s.Verdict.ToString(),
                userCorrected=s.UserCorrected,outcome=s.Outcome.ToString()}).ToArray() });
    }
    Console.WriteLine(JsonSerializer.Serialize(new {item.name,responses,model=counts}));
}
static int Int(JsonElement e,string key) => e.TryGetProperty(key,out var v) ? v.GetInt32() : 0;
static bool Flag(JsonElement e,string key) => e.TryGetProperty(key,out var v) && v.GetBoolean();
static object Event(KeyEvent e) => new {vk=e.Vk,scan=e.Scan,extended=e.Extended,isUp=e.IsUp,injected=e.Injected,time=e.TimeMs};
static object? Decision(DetectionResult? d) => d is null ? null : new {
    verdict=d.Verdict.ToString(),text=d.Text,japanese=d.JapaneseScore,english=d.EnglishScore,summary=d.Summary,
    contributions=d.Contributions.Select(c=>new {source=c.Source,japanese=c.Japanese,english=c.English,reason=c.Reason}).ToArray() };
record Case(string name,JsonElement[] steps,JsonElement settings,Learn[]? learn);
record Learn(string letters,string outcome,bool decidedEnglish,int repeat=1);
sealed class Environment : ISessionEnvironment
{
    public bool ModifierDown;
    public CollectPermission Permission = CollectPermission.Allow;
    public readonly List<FlushRequest> Flushes = [];
    public readonly List<SessionSummary> Ended = [];
    public bool IsModifierDown() => ModifierDown;
    public CollectPermission CanCollect() => Permission;
    public void RequestFlush(FlushRequest request) => Flushes.Add(request);
    public void SessionEnded(SessionSummary summary) => Ended.Add(summary);
}
