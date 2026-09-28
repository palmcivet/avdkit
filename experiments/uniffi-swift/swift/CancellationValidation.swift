import Darwin
import Foundation

enum ValidationError: Error, CustomStringConvertible, Sendable {
    case failed(String)

    var description: String {
        switch self {
        case .failed(let message):
            return message
        }
    }
}

struct ProcessPair: Sendable {
    let shell: pid_t
    let child: pid_t

    var all: [pid_t] { [shell, child] }
}

func pause(milliseconds: UInt64) async throws {
    try await Task<Never, Never>.sleep(nanoseconds: milliseconds * 1_000_000)
}

func readProcesses(from marker: URL) async throws -> ProcessPair {
    for _ in 0..<200 {
        if let contents = try? String(contentsOf: marker, encoding: .utf8) {
            let values = contents.split(separator: " ").compactMap { pid_t($0) }
            if values.count == 2 {
                return ProcessPair(shell: values[0], child: values[1])
            }
        }
        try await pause(milliseconds: 25)
    }
    throw ValidationError.failed("probe did not write process IDs")
}

func processExists(_ process: pid_t) -> Bool {
    if Darwin.kill(process, 0) == 0 {
        return true
    }
    return errno == EPERM
}

func waitForExit(_ processes: ProcessPair) async throws {
    for _ in 0..<200 {
        if processes.all.allSatisfy({ !processExists($0) }) {
            return
        }
        try await pause(milliseconds: 25)
    }
    let survivors = processes.all.filter(processExists)
    throw ValidationError.failed("processes survived cancellation: \(survivors)")
}

func markerURL(_ name: String) -> URL {
    FileManager.default.temporaryDirectory
        .appendingPathComponent("avdkit-uniffi-\(name)-\(UUID().uuidString)")
}

func validateDirectTaskCancellation() async throws {
    let marker = markerURL("direct")
    defer { try? FileManager.default.removeItem(at: marker) }

    let probe = CancellationProbe(markerPath: marker.path)
    let task = Task { await probe.wait() }
    let processes = try await readProcesses(from: marker)

    task.cancel()
    try await pause(milliseconds: 250)
    guard processes.all.allSatisfy(processExists) else {
        throw ValidationError.failed(
            "direct Task cancellation unexpectedly propagated; update the documented result"
        )
    }

    probe.cancel()
    _ = await task.value
    try await waitForExit(processes)
    print("direct_task_cancel_propagates=false")
}

func validateCancellationAdapter() async throws {
    let marker = markerURL("adapter")
    defer { try? FileManager.default.removeItem(at: marker) }

    let probe = CancellationProbe(markerPath: marker.path)
    let task = Task {
        let result = await withTaskCancellationHandler(
            operation: { await probe.wait() },
            onCancel: { probe.cancel() }
        )
        try Task.checkCancellation()
        return result
    }
    let processes = try await readProcesses(from: marker)

    let cancellationStarted = Date()
    task.cancel()
    do {
        _ = try await task.value
        throw ValidationError.failed("adapted task returned instead of throwing CancellationError")
    } catch is CancellationError {
        // Expected.
    }

    try await waitForExit(processes)
    print("adapted_task_cancel_terminates_group=true")
    print(
        "adapted_cancellation_latency_ms="
            + String(Int(Date().timeIntervalSince(cancellationStarted) * 1_000))
    )
}

@main
struct CancellationValidation {
    static func main() async throws {
        guard bindingProbe() == "avdkit-uniffi-validation" else {
            throw ValidationError.failed("synchronous UniFFI call returned the wrong value")
        }
        try await validateDirectTaskCancellation()
        try await validateCancellationAdapter()
    }
}
