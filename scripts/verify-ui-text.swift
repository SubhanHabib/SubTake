// Verify visible editor chrome with macOS Vision, independently of UI callbacks.
// Usage: swift scripts/verify-ui-text.swift IMAGE [--language en|fr]
// Requires 3 distinct header/rail labels; this is not a truncation/layout check.
import Foundation
import Vision
import ImageIO

func normalized(_ text: String) -> String {
    text.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: Locale(identifier: "en_US_POSIX"))
        .components(separatedBy: CharacterSet.alphanumerics.inverted)
        .filter { !$0.isEmpty }.joined(separator: " ")
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(2)
}

let args = Array(CommandLine.arguments.dropFirst())
guard args.count == 1 || (args.count == 3 && args[1] == "--language") else {
    fail("Usage: swift scripts/verify-ui-text.swift IMAGE [--language en|fr]")
}
let language = args.count == 3 ? args[2] : "en"
guard ["en", "fr"].contains(language) else { fail("Unsupported OCR language: \(language)") }
let url = URL(fileURLWithPath: args[0])
guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
    fail("Cannot decode screenshot: \(url.path)")
}
// French values match assets/localization.json; untranslated native labels retain
// English. Save and Record both become Enregistrer, which must count only once.
// Header alternatives allow the guard to detect visible fonts independently of
// the separately tracked rail truncation bug.
let expected = language == "fr"
    ? ["Curseur", "Webcam", "Sous-titres", "Exporter", "Open projects", "Enregistrer", "Presets"]
    : ["Cursor", "Webcam", "Captions", "Export", "Open projects", "Save", "Record", "Presets"]
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.recognitionLanguages = language == "fr" ? ["fr-FR", "en-US"] : ["en-US"]
request.usesLanguageCorrection = false
request.customWords = expected
request.minimumTextHeight = 0.005
do {
    try VNImageRequestHandler(cgImage: image, options: [:]).perform([request])
} catch {
    fail("Vision OCR failed: \(error)")
}
var observations: [[String: Any]] = []
var chromeText: [String] = []
for observation in request.results ?? [] {
    guard let candidate = observation.topCandidates(1).first else { continue }
    let box = observation.boundingBox
    // Vision coordinates start at bottom-left. The editor's 68px rail and
    // 56px header remain outside the media preview at both smoke sizes.
    let inChrome = box.maxX <= 0.12 || box.minY >= 0.88
    observations.append([
        "text": candidate.string, "confidence": candidate.confidence,
        "in_editor_chrome": inChrome,
        "bounds": [box.minX, box.minY, box.width, box.height],
    ])
    if inChrome && candidate.confidence >= 0.2 {
        chromeText.append(" " + normalized(candidate.string) + " ")
    }
}
let matched = expected.filter { label in
    chromeText.contains { $0.contains(" " + normalized(label) + " ") }
}
let passed = matched.count >= 3
let report: [String: Any] = [
    "status": passed ? "passed" : "failed", "image": url.path,
    "language": language, "minimum_distinct_labels": 3,
    "expected_labels": expected, "matched_labels": matched,
    "missing_labels": expected.filter { !matched.contains($0) },
    "scope": "Visible editor header/rail labels only; media fixture text does not count",
    "observations": observations,
]
do {
    let data = try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
    print(String(decoding: data, as: UTF8.self))
} catch {
    fail("Cannot serialize OCR result: \(error)")
}
if !passed {
    FileHandle.standardError.write(Data("UI_TEXT_FAILED: found \(matched.count)/\(expected.count) expected editor labels; require at least 3\n".utf8))
}
exit(passed ? 0 : 1)
