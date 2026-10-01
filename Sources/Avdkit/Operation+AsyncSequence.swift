import Foundation

public extension Operation {
    var events: AsyncThrowingStream<Event, any Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                while !Task.isCancelled, let event = await nextEvent() {
                    continuation.yield(event)
                }
                continuation.finish()
            }
            continuation.onTermination = { @Sendable _ in
                self.cancel()
                task.cancel()
            }
        }
    }

    func value() async throws -> OperationResult {
        try await withTaskCancellationHandler(
            operation: {
                do {
                    let value = try await result()
                    try Task.checkCancellation()
                    return value
                } catch {
                    try Task.checkCancellation()
                    throw error
                }
            },
            onCancel: {
                self.cancel()
            }
        )
    }
}
