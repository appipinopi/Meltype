// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using Meltype.Composition;
using Meltype.Detection;
using Meltype.Config;
internal static class Raw {
    internal static void Write(string path) {
        foreach(var row in JsonDocument.Parse(File.ReadAllText(path)).RootElement.EnumerateArray()) {
            var detector=CompositionDetector.CreateDefault();detector.SpellChecker=BuiltInWordChecker.Shared;
            var text=new CompositionText(detector) { Level=()=>row.GetProperty("manual").GetBoolean()?DetectionLevel.Manual:DetectionLevel.Balanced };
            foreach(var c in row.GetProperty("raw").GetString()!)text.Append(c);
            var segments=text.Segments(final:true);
            var output=new List<object>();
            for(var i=0;i<segments.Count;i++) {
                var segment=segments[i];
                var reading=segment.IsEnglish?segment.Raw:text.KanaForSegment(i)??"";
                var spans=new List<object>();
                if(!segment.IsEnglish)for(var start=0;start<reading.Length;start++)for(var length=1;length<=reading.Length-start;length++)
                    spans.Add(new {start,length,raw=text.RawForReading(i,start,length)});
                output.Add(new {reading,english=segment.IsEnglish,spans});
            }
            Console.WriteLine(JsonSerializer.Serialize(new { @case=row,segments=output }));
        }
    }
}
