import Foundation
import AVFoundation
import Darwin

func emit(_ value: [String: Any]) {
    if let data = try? JSONSerialization.data(withJSONObject: value), let text = String(data: data, encoding: .utf8) { print(text); fflush(stdout) }
}
struct BridgeError: Error, LocalizedError { let message: String; var errorDescription: String? { message } }

func captureErrorMessage(_ error: Error) -> String { error.localizedDescription }

func requestMicrophoneAccess() async throws {
    if AVCaptureDevice.authorizationStatus(for: .audio) == .authorized { return }
    if AVCaptureDevice.authorizationStatus(for: .audio) == .denied {
        throw BridgeError(message: "Allow Patter in System Settings → Privacy & Security → Microphone, then return and click Check again. Reopen Patter if macOS requests it.")
    }
    if AVCaptureDevice.authorizationStatus(for: .audio) == .restricted {
        throw BridgeError(message: "Microphone access is restricted by macOS or your administrator.")
    }
    guard await AVCaptureDevice.requestAccess(for: .audio) else { throw BridgeError(message: "Microphone access was not granted. Allow Patter in System Settings → Privacy & Security → Microphone, then quit and reopen Patter. You can open this page from Settings → General → Recording.") }
}

@main struct PatterNative {
    static func main() async {
        do {
            let args = CommandLine.arguments
            if args.count == 2, args[1] == "self-test" { try testSyntheticAudio(); return }
            if args.count == 2, args[1] == "recording-permissions" {
                let microphone: String
                switch AVCaptureDevice.authorizationStatus(for: .audio) {
                case .authorized: microphone = "allowed"
                case .notDetermined: microphone = "not_requested"
                case .denied: microphone = "denied"
                case .restricted: microphone = "restricted"
                @unknown default: microphone = "unknown"
                }
                // Core Audio has no public non-prompting consent preflight on macOS 15+.
                // ScreenCapture preflight tests a different permission. Never infer
                // denied/allowed or replay a cached result for system audio.
                emit(["microphone": microphone, "systemAudio": "managed_by_macos"])
                return
            }
            if args.count == 2, args[1] == "request-microphone-access" {
                try await requestMicrophoneAccess()
                emit(["status": "access-granted"])
                return
            }
            if args.count == 2, args[1] == "request-recording-access" {
                try await requestMicrophoneAccess()
                emit(["status": "access-granted"])
                return
            }
            guard (args.count == 3 || args.count == 4), args[1] == "record" else { throw BridgeError(message: "Usage: patter-native record DIRECTORY") }
            let destination = URL(fileURLWithPath: args[2], isDirectory: true)
            try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
            let baseOffset = args.count == 4 ? Double(args[3]) ?? .nan : 0
            guard baseOffset.isFinite, baseOffset >= 0 else { throw BridgeError(message: "Invalid recording offset.") }
            let recorder = Recorder(directory: destination, baseOffset: baseOffset)
            try await recorder.start()
            await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
                DispatchQueue.global().async { _ = readLine(); continuation.resume() }
            }
            try await recorder.stop()
            emit(["status": "stopped"])
        } catch { emit(["error": captureErrorMessage(error)]); exit(1) }
    }
}
