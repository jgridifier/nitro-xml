// SPDX-License-Identifier: MIT
// Catches: Open XML packages that OpenXmlValidator (Office2019) rejects with new errors after save.

using System.Text.Json;
using DocumentFormat.OpenXml;
using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Validation;

static int Usage()
{
    Console.Error.WriteLine("Usage: ooxml-validate <path-to-ooxml-package>");
    return 2;
}

static OpenXmlPackage OpenPackage(string path)
{
    var ext = Path.GetExtension(path).ToLowerInvariant();
    return ext switch
    {
        ".xlsx" or ".xlsm" or ".xltx" or ".xltm" => SpreadsheetDocument.Open(path, false),
        ".docx" or ".docm" or ".dotx" or ".dotm" => WordprocessingDocument.Open(path, false),
        ".pptx" or ".pptm" or ".potx" or ".potm" => PresentationDocument.Open(path, false),
        _ => throw new ArgumentException($"unsupported package extension: {ext}"),
    };
}

if (args.Length != 1)
{
    return Usage();
}

var path = args[0];
if (!File.Exists(path))
{
    Console.Error.WriteLine($"file not found: {path}");
    return 2;
}

var errors = new List<Dictionary<string, string?>>();
try
{
    using var doc = OpenPackage(path);
    var validator = new OpenXmlValidator(FileFormatVersions.Office2019);
    foreach (var e in validator.Validate(doc))
    {
        errors.Add(new Dictionary<string, string?>
        {
            ["id"] = e.Id,
            ["description"] = e.Description,
            ["errorType"] = e.ErrorType.ToString(),
            ["part"] = e.Part?.Uri?.ToString(),
            ["path"] = e.Path?.XPath,
        });
    }
}
catch (Exception ex)
{
    // Unreadable / non-package inputs become a single stable error entry.
    errors.Add(new Dictionary<string, string?>
    {
        ["id"] = "package-open",
        ["description"] = ex.Message,
        ["errorType"] = "Exception",
        ["part"] = null,
        ["path"] = null,
    });
}

// Stable sorted JSON error set (PRODUCT_SPEC §5 / #48).
errors.Sort(static (a, b) =>
{
    var ka = $"{a.GetValueOrDefault("id")}|{a.GetValueOrDefault("part")}|{a.GetValueOrDefault("path")}|{a.GetValueOrDefault("description")}";
    var kb = $"{b.GetValueOrDefault("id")}|{b.GetValueOrDefault("part")}|{b.GetValueOrDefault("path")}|{b.GetValueOrDefault("description")}";
    return string.CompareOrdinal(ka, kb);
});

var json = JsonSerializer.Serialize(errors, new JsonSerializerOptions
{
    WriteIndented = false,
});
Console.WriteLine(json);
return 0;
