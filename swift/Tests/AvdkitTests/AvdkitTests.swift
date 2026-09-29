import Testing
@testable import Avdkit

@Test
func defaultConfigurationQueriesEnvironment() async throws {
    let kit = try Kit(config: defaultKitConfig())
    let report = try await kit.environment()
    #expect(!report.snapshot.paths.sdkRoot.isEmpty)
    #expect(!report.capabilities.isEmpty)
}

@Test
func operationEventsAndFinalErrorAreObservable() async throws {
    let kit = try Kit(config: defaultKitConfig())
    let package = PackageId(
        kind: .systemImage,
        api: "999",
        tag: "google_apis",
        abi: "arm64-v8a",
        qualifier: nil
    )
    let operation = await kit.install(package: package)
    var events: [Event] = []
    for try await event in operation.events {
        events.append(event)
    }
    #expect(!events.isEmpty)
    do {
        _ = try await operation.value()
        Issue.record("unsupported installation should fail")
    } catch {
        // The generated BindingError is the expected final value.
    }
}
