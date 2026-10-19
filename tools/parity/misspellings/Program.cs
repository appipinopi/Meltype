// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using System.Globalization;
using System.Reflection;
using System.Runtime.InteropServices;
using Meltype.Composition;
if(args[0]=="--categories") {
    var bitmap=new byte[8192];
    var ranges=new List<int[]>();
    int start=-1;
    for(int value=0;value<=65536;value++) {
        var marked=value<65536 && CharUnicodeInfo.GetUnicodeCategory((char)value)==UnicodeCategory.NonSpacingMark;
        if(marked) { bitmap[value/8]|=(byte)(1<<(value%8)); if(start<0)start=value; }
        else if(start>=0) { ranges.Add([start,value-1]); start=-1; }
    }
    Console.WriteLine(JsonSerializer.Serialize(new { runtime=RuntimeInformation.FrameworkDescription, ranges, bitmap=Convert.ToBase64String(bitmap) }));
    return;
}
if(args[0]=="--shapes") {
    var method=typeof(MisspellingDictionary).GetMethod("Shape",BindingFlags.Static|BindingFlags.NonPublic)!;
    foreach(var reading in JsonSerializer.Deserialize<string[]>(File.ReadAllText(args[1]))!)
        Console.WriteLine(JsonSerializer.Serialize(new { reading, expected=(string)method.Invoke(null,[reading])! }));
    return;
}
var dictionary=MisspellingDictionary.Load(null);
foreach(var reading in JsonSerializer.Deserialize<string[]>(File.ReadAllText(args[0]))!) {
    try {
        var text=CompositionText.ToKatakana(reading);
        var found=dictionary.Find(reading);
        var expected=found is {} f ? text[..f.Start]+f.Right+text[(f.Start+f.Length)..] : null;
        Console.WriteLine(JsonSerializer.Serialize(new { reading, expected, error=(string?)null }));
    } catch(ArgumentException) {
        // Keep original failures as evidence; do not invent a normal result.
        Console.WriteLine(JsonSerializer.Serialize(new { reading, expected=(string?)null, error="ArgumentException" }));
    }
}
