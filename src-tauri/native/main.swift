import Foundation
import ScreenCaptureKit
import AVFoundation
import CoreMedia
import Darwin

func emit(_ value: [String: Any]) {
    if let data = try? JSONSerialization.data(withJSONObject: value), let text = String(data: data, encoding: .utf8) { print(text); fflush(stdout) }
}
struct BridgeError: Error, LocalizedError { let message: String; var errorDescription: String? { message } }

func captureErrorMessage(_ error: Error) -> String {
    let native = error as NSError
    if native.domain == SCStreamError.errorDomain && native.code == SCStreamError.Code.userDeclined.rawValue {
        return "Computer audio access was not granted. Allow Patter in System Settings → Privacy & Security → Screen & System Audio Recording, then quit and reopen Patter. You can open this page from Settings → General → Recording."
    }
    return error.localizedDescription
}

func requestMicrophoneAccess() async throws {
    if AVCaptureDevice.authorizationStatus(for: .audio) == .restricted {
        throw BridgeError(message: "Microphone access is restricted by macOS or your administrator.")
    }
    guard await AVCaptureDevice.requestAccess(for: .audio) else { throw BridgeError(message: "Microphone access was not granted. Allow Patter in System Settings → Privacy & Security → Microphone, then quit and reopen Patter. You can open this page from Settings → General → Recording.") }
}

@available(macOS 15.0, *)
final class Recorder: NSObject, SCStreamOutput, SCStreamDelegate {
    let directory: URL
    let queue = DispatchQueue(label: "gr.tau.patter.audio")
    var stream: SCStream?
    var files: [String: AVAudioFile] = [:]
    var starts: [String: Double] = [:]
    var firstTimestamp: Double?
    let session = UUID().uuidString
    var sequence = 0
    var failure: String?
    init(directory: URL) { self.directory = directory }
    func start() async throws {
        try await requestMicrophoneAccess()
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        guard let display = content.displays.first else { throw BridgeError(message: "No display is available for computer audio capture.") }
        let filter = SCContentFilter(display: display, excludingApplications: [], exceptingWindows: [])
        let config = SCStreamConfiguration()
        config.width = 2; config.height = 2
        config.minimumFrameInterval = CMTime(value: 1, timescale: 1)
        config.capturesAudio = true; config.captureMicrophone = true
        config.excludesCurrentProcessAudio = true
        config.sampleRate = 48000; config.channelCount = 1
        let capture = SCStream(filter: filter, configuration: config, delegate: self)
        try capture.addStreamOutput(self, type: .audio, sampleHandlerQueue: queue)
        try capture.addStreamOutput(self, type: .microphone, sampleHandlerQueue: queue)
        // No screen output is registered or written. Only the two audio sources are retained.
        stream = capture
        try await capture.startCapture()
        emit(["status": "recording"])
    }
    func stream(_ stream: SCStream, didStopWithError error: Error) { queue.async { self.failure = captureErrorMessage(error); emit(["error": captureErrorMessage(error)]) } }
    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of outputType: SCStreamOutputType) {
        guard outputType == .audio || outputType == .microphone, sampleBuffer.isValid, CMSampleBufferGetNumSamples(sampleBuffer) > 0, failure == nil else { return }
        do {
            let values = try directory.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
            if let capacity = values.volumeAvailableCapacityForImportantUsage, capacity < 100_000_000 { throw BridgeError(message: "Storage is almost full. Recording stopped accepting audio; saved chunks are kept. Stop the recording and free disk space.") }
            guard let desc = CMSampleBufferGetFormatDescription(sampleBuffer), let format = AVAudioFormat(cmAudioFormatDescription: desc) as AVAudioFormat? else { return }
            var size = 0
            var block: CMBlockBuffer?
            let flag = UInt32(kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment)
            var status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sampleBuffer, bufferListSizeNeededOut: &size, bufferListOut: nil, bufferListSize: 0, blockBufferAllocator: nil, blockBufferMemoryAllocator: nil, flags: flag, blockBufferOut: &block)
            guard status == noErr else { throw BridgeError(message: "Could not read audio buffers (\(status)).") }
            let allocation = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 16)
            defer { allocation.deallocate() }
            let list = allocation.bindMemory(to: AudioBufferList.self, capacity: 1)
            status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sampleBuffer, bufferListSizeNeededOut: nil, bufferListOut: list, bufferListSize: size, blockBufferAllocator: nil, blockBufferMemoryAllocator: nil, flags: flag, blockBufferOut: &block)
            guard status == noErr, let pcm = AVAudioPCMBuffer(pcmFormat: format, bufferListNoCopy: list, deallocator: nil) else { throw BridgeError(message: "Could not prepare an audio chunk.") }
            let timestamp = CMTimeGetSeconds(CMSampleBufferGetPresentationTimeStamp(sampleBuffer))
            if firstTimestamp == nil { firstTimestamp = timestamp }
            let offset = max(0, timestamp - (firstTimestamp ?? timestamp))
            let track = outputType == .microphone ? "microphone" : "computer"
            if files[track] == nil || offset - (starts[track] ?? 0) >= 5 {
                files[track] = nil
                sequence += 1
                let filename = String(format: "%010d-%@-%05d-%@.caf", Int(offset * 1000), track, sequence, session)
                files[track] = try AVAudioFile(forWriting: directory.appendingPathComponent(filename), settings: format.settings, commonFormat: format.commonFormat, interleaved: format.isInterleaved)
                starts[track] = offset
            }
            try files[track]?.write(from: pcm)
        } catch { failure = error.localizedDescription; emit(["error": error.localizedDescription]) }
    }
    func stop() async throws {
        try await stream?.stopCapture()
        queue.sync { files.removeAll() }
        if let failure { throw BridgeError(message: failure) }
    }
}
@main struct PatterNative {
    static func main() async {
        do {
            let args = CommandLine.arguments
            if args.count == 2, args[1] == "request-recording-access" {
                try await requestMicrophoneAccess()
                // This requests consent/enumerates sources without creating a stream or audio files.
                _ = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
                emit(["status": "access-granted"])
                return
            }
            guard args.count == 3, args[1] == "record" else { throw BridgeError(message: "Usage: patter-native record DIRECTORY") }
            let destination = URL(fileURLWithPath: args[2], isDirectory: true)
            try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
            let recorder = Recorder(directory: destination)
            try await recorder.start()
            await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
                DispatchQueue.global().async { _ = readLine(); continuation.resume() }
            }
            try await recorder.stop()
            emit(["status": "stopped"])
        } catch { emit(["error": captureErrorMessage(error)]); exit(1) }
    }
}
