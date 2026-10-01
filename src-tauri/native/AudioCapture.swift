import Foundation
import CoreAudio
import AVFoundation
import Darwin

// Release every acquired resource after a failed start/stop; repeat cleanup is
// harmless. In particular a failed AudioDeviceStop must not leak a tap/device.
final class CaptureResources {
    private var cleanups: [() throws -> Void] = []
    func own(_ cleanup: @escaping () throws -> Void) { cleanups.append(cleanup) }
    func close() throws {
        let pending = Array(cleanups.reversed())
        cleanups.removeAll()
        var firstError: Error?
        for cleanup in pending { do { try cleanup() } catch { firstError = firstError ?? error } }
        if let firstError { throw firstError }
    }
}

// Shared host-clock timestamps preserve the two tracks and cumulative timeline
// used by the existing finalized-audio/transcription pipeline.
final class AudioChunks {
    let directory: URL
    let baseOffset: Double
    private var files: [String: AVAudioFile] = [:]
    private var starts: [String: Double] = [:]
    private var firstTimestamp: Double?
    private let session = UUID().uuidString
    private var sequence = 0
    init(directory: URL, baseOffset: Double) { self.directory = directory; self.baseOffset = baseOffset }
    func write(_ pcm: AVAudioPCMBuffer, track: String, timestamp: Double) throws {
        guard pcm.frameLength > 0, timestamp.isFinite else { return }
        let values = try directory.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
        if let capacity = values.volumeAvailableCapacityForImportantUsage, capacity < 100_000_000 { throw BridgeError(message: "Storage is almost full. Saved audio is kept; stop recording and free disk space.") }
        if firstTimestamp == nil { firstTimestamp = timestamp }
        let offset = baseOffset + max(0, timestamp - (firstTimestamp ?? timestamp))
        if files[track] == nil || offset - (starts[track] ?? 0) >= 5 || files[track]?.processingFormat != pcm.format {
            files[track] = nil
            sequence += 1
            let name = String(format: "%010d-%@-%05d-%@.caf", Int(offset * 1000), track, sequence, session)
            files[track] = try AVAudioFile(forWriting: directory.appendingPathComponent(name), settings: pcm.format.settings, commonFormat: pcm.format.commonFormat, interleaved: pcm.format.isInterleaved)
            starts[track] = offset
        }
        try files[track]?.write(from: pcm)
    }
    func finish() { files.removeAll() }
}

func audioStatus(_ status: OSStatus, _ operation: String) throws {
    guard status == noErr else {
        let recovery = operation == "Starting system audio capture" ? " Check Patter under System Settings → Privacy & Security → Screen & System Audio Recording → System Audio Recording Only. Screen recording is not needed. Return to Patter; reopen it if macOS requests it." : " Saved audio is kept. Stop recording and try again."
        throw BridgeError(message: "\(operation) failed (Core Audio \(status)).\(recovery)")
    }
}

@available(macOS 15.0, *)
final class Recorder {
    let queue = DispatchQueue(label: "gr.tau.patter.audio")
    let chunks: AudioChunks
    let resources = CaptureResources()
    private var failure: Error?
    private let microphone = AVAudioEngine()
    init(directory: URL, baseOffset: Double = 0) { chunks = AudioChunks(directory: directory, baseOffset: baseOffset) }
    func start() async throws {
        do {
            try await requestMicrophoneAccess()
            try startSystemAudio()
            let input = microphone.inputNode
            let format = input.outputFormat(forBus: 0)
            guard format.sampleRate > 0, format.channelCount > 0 else { throw BridgeError(message: "No microphone input is available. Choose an input device in macOS Sound settings.") }
            input.installTap(onBus: 0, bufferSize: 4096, format: format) { [weak self] buffer, when in
                guard let self, let copy = AVAudioPCMBuffer(pcmFormat: buffer.format, frameCapacity: buffer.frameLength) else { return }
                copy.frameLength = buffer.frameLength
                let source = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: buffer.audioBufferList))
                let destination = UnsafeMutableAudioBufferListPointer(copy.mutableAudioBufferList)
                for (from, to) in zip(source, destination) {
                    if let source = from.mData, let destination = to.mData { memcpy(destination, source, Int(min(from.mDataByteSize, to.mDataByteSize))) }
                }
                let timestamp = AVAudioTime.seconds(forHostTime: when.isHostTimeValid ? when.hostTime : mach_absolute_time())
                self.queue.async { self.write(copy, track: "microphone", timestamp: timestamp) }
            }
            resources.own { [microphone] in microphone.inputNode.removeTap(onBus: 0) }
            try microphone.start()
            resources.own { [microphone] in microphone.stop() }
            emit(["status": "recording"])
        } catch {
            try? resources.close()
            queue.sync { chunks.finish() }
            throw error
        }
    }
    private func startSystemAudio() throws {
        var excluded: [AudioObjectID] = []
        for var pid in [getpid(), getppid()] {
            var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyTranslatePIDToProcessObject, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
            var process = AudioObjectID(kAudioObjectUnknown)
            var size = UInt32(MemoryLayout<AudioObjectID>.size)
            if AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, UInt32(MemoryLayout<pid_t>.size), &pid, &size, &process) == noErr, process != kAudioObjectUnknown { excluded.append(process) }
        }
        // Outgoing sound only: no screen stream, window/display enumeration,
        // or screen permission API. Keep ordinary playback unmuted.
        let description = CATapDescription(monoGlobalTapButExcludeProcesses: excluded)
        description.name = "Patter system audio"
        description.isPrivate = true
        description.muteBehavior = .unmuted
        if #available(macOS 26.0, *) { description.bundleIDs = ["gr.tau.patter"] }
        var tap = AudioObjectID(kAudioObjectUnknown)
        try audioStatus(AudioHardwareCreateProcessTap(description, &tap), "Creating system audio tap")
        resources.own { try audioStatus(AudioHardwareDestroyProcessTap(tap), "Closing system audio tap") }
        var address = AudioObjectPropertyAddress(mSelector: kAudioTapPropertyFormat, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var asbd = AudioStreamBasicDescription()
        var size = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)
        try audioStatus(AudioObjectGetPropertyData(tap, &address, 0, nil, &size, &asbd), "Reading system audio format")
        guard let format = AVAudioFormat(streamDescription: &asbd) else { throw BridgeError(message: "System audio returned an unsupported format.") }
        let composition: [String: Any] = [
            kAudioAggregateDeviceNameKey: "Patter private audio capture",
            kAudioAggregateDeviceUIDKey: UUID().uuidString,
            kAudioAggregateDeviceIsPrivateKey: true,
            kAudioAggregateDeviceTapListKey: [[kAudioSubTapUIDKey: description.uuid.uuidString, kAudioSubTapDriftCompensationKey: true]],
        ]
        var device = AudioObjectID(kAudioObjectUnknown)
        try audioStatus(AudioHardwareCreateAggregateDevice(composition as CFDictionary, &device), "Creating audio capture device")
        resources.own { try audioStatus(AudioHardwareDestroyAggregateDevice(device), "Closing audio capture device") }
        var io: AudioDeviceIOProcID?
        try audioStatus(AudioDeviceCreateIOProcIDWithBlock(&io, device, queue) { [weak self] _, buffers, inputTime, _, _ in
            guard let self, let pcm = AVAudioPCMBuffer(pcmFormat: format, bufferListNoCopy: buffers, deallocator: nil) else { return }
            let hostTime = inputTime.pointee.mFlags.contains(.hostTimeValid) ? inputTime.pointee.mHostTime : mach_absolute_time()
            self.write(pcm, track: "computer", timestamp: AVAudioTime.seconds(forHostTime: hostTime))
        }, "Preparing system audio capture")
        guard let io else { throw BridgeError(message: "System audio did not return a capture callback.") }
        resources.own { try audioStatus(AudioDeviceDestroyIOProcID(device, io), "Closing system audio callback") }
        // Apple prompts for system audio only here if necessary. Permission
        // checks/Settings refresh never create taps or reach AudioDeviceStart.
        try audioStatus(AudioDeviceStart(device, io), "Starting system audio capture")
        resources.own { try audioStatus(AudioDeviceStop(device, io), "Stopping system audio capture") }
    }
    private func write(_ pcm: AVAudioPCMBuffer, track: String, timestamp: Double) {
        guard failure == nil else { return }
        do { try chunks.write(pcm, track: track, timestamp: timestamp) }
        catch { failure = error; emit(["error": error.localizedDescription]) }
    }
    func stop() async throws {
        var cleanupError: Error?
        do { try resources.close() } catch { cleanupError = error }
        let captureError = queue.sync { () -> Error? in chunks.finish(); return failure }
        if let error = captureError ?? cleanupError { throw error }
    }
}

// No Recorder, permission call, tap, audio engine or device IO is created by
// this regression harness: only generated silence and fake resource cleanups.
func testSyntheticAudio() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("patter-audio-fixture-\(UUID())")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let writer = AudioChunks(directory: root, baseOffset: 8)
    let format = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 1)!
    let pcm = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 4800)!
    pcm.frameLength = 4800
    memset(pcm.floatChannelData![0], 0, 4800 * MemoryLayout<Float>.size)
    let empty = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 1)!
    try writer.write(empty, track: "computer", timestamp: 1)
    try writer.write(pcm, track: "computer", timestamp: .nan)
    guard try FileManager.default.contentsOfDirectory(atPath: root.path).isEmpty else { throw BridgeError(message: "Empty/invalid capture created audio") }
    guard let borrowed = AVAudioPCMBuffer(pcmFormat: format, bufferListNoCopy: pcm.audioBufferList, deallocator: nil), borrowed.frameLength == pcm.frameLength else { throw BridgeError(message: "Core Audio buffer frame length failed") }
    try writer.write(borrowed, track: "computer", timestamp: 100)
    try writer.write(pcm, track: "microphone", timestamp: 100.25)
    try writer.write(pcm, track: "computer", timestamp: 105.5)
    writer.finish(); writer.finish()
    let files = try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)
    guard files.count == 3, files.contains(where: { $0.lastPathComponent.hasPrefix("0000008000-computer") }), files.contains(where: { $0.lastPathComponent.hasPrefix("0000008250-microphone") }), files.contains(where: { $0.lastPathComponent.hasPrefix("0000013500-computer") }) else { throw BridgeError(message: "Synthetic track timeline/rotation failed") }
    for file in files { guard try AVAudioFile(forReading: file).length == 4800 else { throw BridgeError(message: "Synthetic audio was not finalized") } }
    var released: [String] = []
    let scope = CaptureResources()
    scope.own { released.append("tap") }
    scope.own { released.append("device") }
    scope.own { released.append("callback"); throw BridgeError(message: "fixture stop failure") }
    scope.own { released.append("stop") }
    do { try scope.close(); throw BridgeError(message: "Missing synthetic cleanup failure") } catch let error as BridgeError { guard error.message == "fixture stop failure" else { throw error } }
    try scope.close()
    guard released == ["stop", "callback", "device", "tap"] else { throw BridgeError(message: "Cleanup leaked or released twice") }
    for acquired in 0...3 {
        let partial = CaptureResources()
        var count = 0
        for _ in 0..<acquired { partial.own { count += 1 } }
        try partial.close(); try partial.close()
        guard count == acquired else { throw BridgeError(message: "Partial start leaked resources") }
    }
    emit(["status": "synthetic-tests-passed"])
}
