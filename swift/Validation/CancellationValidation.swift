import Darwin
import Foundation

enum ValidationError: Error, CustomStringConvertible {
    case failed(String)

    var description: String {
        switch self {
        case .failed(let message):
            return message
        }
    }
}

struct ProcessPair {
    let shell: pid_t
    let child: pid_t

    var all: [pid_t] { [shell, child] }
}

func shellQuote(_ value: String) -> String {
    "'" + value.replacingOccurrences(of: "'", with: "'\\''") + "'"
}

func writeExecutable(_ contents: String, to url: URL) throws {
    try contents.write(to: url, atomically: true, encoding: .utf8)
    guard chmod(url.path, 0o755) == 0 else {
        throw ValidationError.failed("chmod failed for \(url.path)")
    }
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
    throw ValidationError.failed("Android CLI probe did not write process IDs")
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
    throw ValidationError.failed(
        "processes survived Operation cancellation: \(processes.all.filter(processExists))"
    )
}

func prepareFakeSdk(root: URL, marker: URL) throws -> URL {
    let files = FileManager.default
    let sdk = root.appendingPathComponent("sdk")
    let bin = root.appendingPathComponent("bin")
    let emulator = sdk.appendingPathComponent("emulator")
    let platformTools = sdk.appendingPathComponent("platform-tools")
    let image = sdk.appendingPathComponent(
        "system-images/android-999/google_apis/arm64-v8a"
    )
    for directory in [bin, emulator, platformTools, image] {
        try files.createDirectory(at: directory, withIntermediateDirectories: true)
    }
    try "Pkg.Revision=1.0.0\n".write(
        to: emulator.appendingPathComponent("source.properties"),
        atomically: true,
        encoding: .utf8
    )
    try "Pkg.Revision=1.0.0\n".write(
        to: platformTools.appendingPathComponent("source.properties"),
        atomically: true,
        encoding: .utf8
    )
    try "Pkg.Revision=1.0.0\n".write(
        to: image.appendingPathComponent("source.properties"),
        atomically: true,
        encoding: .utf8
    )
    try writeExecutable("#!/bin/sh\nexit 0\n", to: emulator.appendingPathComponent("emulator"))
    try writeExecutable(
        """
        #!/bin/sh
        printf 'List of devices attached\\n\\n'
        """,
        to: platformTools.appendingPathComponent("adb")
    )
    try writeExecutable(
        """
        #!/bin/sh
        case " $* " in
          *" --version "*)
            printf '1.0.15985488\\n'
            ;;
          *" emulator create "*)
            sleep 120 &
            child=$!
            printf '%s %s' $$ "$child" > \(shellQuote(marker.path))
            wait
            ;;
          *)
            exit 0
            ;;
        esac
        """,
        to: bin.appendingPathComponent("android")
    )
    let home = root.appendingPathComponent("home")
    try files.createDirectory(at: home, withIntermediateDirectories: true)
    setenv("HOME", home.path, 1)
    setenv("ANDROID_USER_HOME", home.appendingPathComponent(".android").path, 1)
    setenv(
        "PATH",
        bin.path + ":" + (ProcessInfo.processInfo.environment["PATH"] ?? ""),
        1
    )
    return sdk
}

@main
struct CancellationValidation {
    static func main() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("avdkit-swift-\(UUID().uuidString)")
        let marker = root.appendingPathComponent("processes")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let sdk = try prepareFakeSdk(root: root, marker: marker)

        var config = defaultKitConfig()
        config.sdkRoot = sdk.path
        let kit = try Kit(config: config)
        let draft = CreateDeviceDraft(
            id: "avdkit_test_swift_cancel",
            profile: "avdkit_test_swift_profile",
            image: PackageId(
                kind: .systemImage,
                api: "999",
                tag: "google_apis",
                abi: "arm64-v8a",
                qualifier: nil
            ),
            displayName: "Swift Cancellation Probe",
            hardware: HardwareConfig(
                ramMib: nil,
                cpuCount: nil,
                screenWidth: nil,
                screenHeight: nil
            )
        )
        let operation = try await kit.executePlan(plan: kit.planCreate(draft: draft))
        let task = Task {
            try await operation.value()
        }
        let processes: ProcessPair
        do {
            processes = try await readProcesses(from: marker)
        } catch {
            do {
                _ = try await task.value
                throw error
            } catch let operationError {
                throw ValidationError.failed(
                    "operation failed before launching Android CLI: \(operationError)"
                )
            }
        }
        task.cancel()
        do {
            _ = try await task.value
            throw ValidationError.failed("cancelled operation returned successfully")
        } catch is CancellationError {
            // Expected: Swift cancellation reached Operation.cancel().
        }
        try await waitForExit(processes)
        print("swift_operation_cancel_terminates_process_group=true")
    }
}
