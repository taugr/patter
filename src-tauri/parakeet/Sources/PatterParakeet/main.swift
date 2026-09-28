import Foundation
import FluidAudio

struct Recording: Decodable {
    let path: String
    let offset: Double
    let track: String
}
struct Segment: Encodable {
    let start: Double
    let text: String
    let speaker: String
}

@main struct PatterParakeet {
    static func main() async {
        do {
            let args = CommandLine.arguments
            guard args.count == 4 else { throw NSError(domain: "Patter", code: 1, userInfo: [NSLocalizedDescriptionKey: "Expected model directory, recording list and output file."]) }
            // loadLocal never resolves a remote repository or downloads missing files.
            let models = try AsrModels.loadLocal(from: URL(fileURLWithPath: args[1]), version: .v3, encoderPrecision: .int8V2)
            let manager = AsrManager()
            try await manager.loadModels(models)
            let recordings = try JSONDecoder().decode([Recording].self, from: Data(contentsOf: URL(fileURLWithPath: args[2])))
            var segments: [Segment] = []
            for recording in recordings {
                var state = try TdtDecoderState()
                let result = try await manager.transcribe(URL(fileURLWithPath: recording.path), decoderState: &state)
                let words = buildWordTimings(from: result.tokenTimings ?? [])
                if words.isEmpty {
                    if !result.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        segments.append(Segment(start: recording.offset, text: result.text, speaker: recording.track))
                    }
                } else {
                    // Keep readable phrases and seekable timestamps, rather than one UI row per word.
                    var phrase: [WordTiming] = []
                    for word in words {
                        if let first = phrase.first, word.startTime - first.startTime >= 12 {
                            segments.append(Segment(start: recording.offset + first.startTime, text: phrase.map(\.word).joined(separator: " "), speaker: recording.track))
                            phrase = []
                        }
                        phrase.append(word)
                    }
                    if let first = phrase.first {
                        segments.append(Segment(start: recording.offset + first.startTime, text: phrase.map(\.word).joined(separator: " "), speaker: recording.track))
                    }
                }
            }
            segments.sort { $0.start < $1.start }
            try JSONEncoder().encode(segments).write(to: URL(fileURLWithPath: args[3]), options: .atomic)
        } catch {
            FileHandle.standardError.write(Data("Parakeet: \(error.localizedDescription)\n".utf8))
            exit(1)
        }
    }
}
