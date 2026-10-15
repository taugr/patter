// Opt-in, file-fed capture source. Compiled alongside production AudioCapture.swift.
// No Recorder, audio engine, tap, playback or permission API is used.
import Foundation
import AVFoundation
import Darwin
struct BridgeError: Error, LocalizedError {
    let message: String
    var errorDescription: String? { message }
}
func emit(_ value: [String: Any]) {
    let data = try! JSONSerialization.data(withJSONObject: value)
    print(String(data: data, encoding: .utf8)!); fflush(stdout)
}
func requestMicrophoneAccess() async throws {
    throw BridgeError(message: "Device capture is forbidden in the file-fed harness")
}
@main struct FileCapture {
    static func main() {
        do {
            let args = CommandLine.arguments
            guard args.count == 5, ["normal", "failed", "empty"].contains(args[4]),
                  let offset = Double(args[3]), offset.isFinite, offset >= 0 else {
                throw BridgeError(message: "Expected destination, synthetic audio, offset, mode")
            }
            let destination = URL(fileURLWithPath: args[1], isDirectory: true)
            try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
            let chunks = AudioChunks(directory: destination, baseOffset: offset)
            if args[4] != "empty" {
                let audio = try AVAudioFile(forReading: URL(fileURLWithPath: args[2]))
                let format = audio.processingFormat
                let frames = AVAudioFrameCount(format.sampleRate / 4)
                let pcm = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames)!
                while audio.framePosition < audio.length {
                    let position = audio.framePosition
                    try audio.read(into: pcm, frameCount: frames)
                    try chunks.write(pcm, track: "microphone", timestamp: 100 + Double(position) / format.sampleRate)
                }
            }
            emit(["status": "recording"])
            guard readLine() == "stop" else { throw BridgeError(message: "Missing Stop command") }
            chunks.finish()
            if args[4] == "failed" {
                throw BridgeError(message: "Synthetic stop failure; finalized chunks are retained")
            }
            emit(["status": "stopped"])
        } catch { emit(["error": error.localizedDescription]); exit(1) }
    }
}
