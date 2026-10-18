// SPDX-License-Identifier: GPL-3.0-or-later
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text.Json;
using Meltype.Composition;

internal static class Synthetic
{
    private const BindingFlags Hidden = BindingFlags.Static | BindingFlags.NonPublic;
    internal static void Write(string path)
    {
        var cases = JsonDocument.Parse(File.ReadAllText(path)).RootElement;
        foreach (var c in cases.EnumerateArray())
        {
            object? result;
            if (c.GetProperty("kind").GetString() == "split")
                result = typeof(MsImeKanjiConverter).GetMethod("SplitForEngine", Hidden)!.Invoke(null, [c.GetProperty("input").GetString(), c.GetProperty("maximum").GetInt32()]);
            else result = Parse(c);
            Console.WriteLine(JsonSerializer.Serialize(new { @case = c, expected = result }));
        }
    }
    private static object? Parse(JsonElement c)
    {
        var converter = typeof(MsImeKanjiConverter);
        var headerType = converter.GetNestedType("MORRSLT", BindingFlags.NonPublic)!;
        var wordType = converter.GetNestedType("WDD", BindingFlags.NonPublic)!;
        var input = c.GetProperty("input").GetString()!;
        var output = c.GetProperty("output").GetString()!;
        var words = c.GetProperty("words").EnumerateArray().ToArray();
        var header = Activator.CreateInstance(headerType)!;
        var headerPointer = Marshal.AllocHGlobal(Marshal.SizeOf(headerType));
        var outputPointer = Marshal.StringToHGlobalUni(output);
        var readingPointer = c.TryGetProperty("reading", out var reading) && reading.ValueKind == JsonValueKind.String ? Marshal.StringToHGlobalUni(reading.GetString()!) : IntPtr.Zero;
        var wordSize = Marshal.SizeOf(wordType);
        var wordPointer = Marshal.AllocHGlobal(Math.Max(1, words.Length) * wordSize);
        try
        {
            void Set(string name, object value) => headerType.GetField(name)!.SetValue(header, value);
            var delta = c.TryGetProperty("sizeDelta", out var size) ? size.GetInt32() : 0;
            Set("dwSize", (uint)(Marshal.SizeOf(headerType) + delta));
            Set("pwchOutput", c.TryGetProperty("nullOutput", out var noOutput) && noOutput.GetBoolean() ? IntPtr.Zero : outputPointer);
            Set("cchOutput", (ushort)output.Length); Set("pwchRead", readingPointer);
            Set("cchRead", (ushort)(readingPointer == IntPtr.Zero ? 0 : reading.GetString()!.Length));
            Set("pWDD", c.TryGetProperty("nullWords", out var noWords) && noWords.GetBoolean() ? IntPtr.Zero : wordPointer);
            Set("cWDD", c.TryGetProperty("wordCount", out var count) ? count.GetInt32() : words.Length);
            for (var i = 0; i < words.Length; ++i)
            {
                var w = words[i]; var word = Activator.CreateInstance(wordType)!;
                foreach (var (name, field) in new[] { ("wReadPos", "readStart"), ("cchRead", "readLength"), ("wDispPos", "dispStart"), ("cchDisp", "dispLength"), ("nPos", "pos"), ("flags", "flags") })
                    wordType.GetField(name)!.SetValue(word, (ushort)(w.TryGetProperty(field, out var value) ? value.GetInt32() : 0));
                Marshal.StructureToPtr(word, wordPointer + i * wordSize, false);
            }
            Marshal.StructureToPtr(header, headerPointer, false);
            try
            {
                var result = converter.GetMethod("ParseClauses", Hidden)!.Invoke(null, [headerPointer, input, c.GetProperty("contextLength").GetInt32()]) as IReadOnlyList<ConversionClause>;
                return result?.Select(s => new { reading = s.Reading, text = s.Text }).ToArray();
            }
            catch (TargetInvocationException) { return null; } // Morph also returns null on a failed parse.
        }
        finally
        {
            Marshal.FreeHGlobal(headerPointer); Marshal.FreeHGlobal(outputPointer);
            if (readingPointer != IntPtr.Zero) Marshal.FreeHGlobal(readingPointer);
            Marshal.FreeHGlobal(wordPointer);
        }
    }
}
