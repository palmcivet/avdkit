use std::sync::atomic::{AtomicU64, Ordering};
use std::{future::Future, pin::Pin};

use model::{Error, ErrorCode, Event, OperationResult};
use process::CancellationToken;
use tokio::sync::{mpsc, oneshot};

static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);

/// Handle for consuming events, cancelling work, and obtaining one final result.
#[derive(Debug)]
pub struct Operation {
    /// Process-local operation identifier.
    pub id: String,
    events: mpsc::UnboundedReceiver<Event>,
    result: oneshot::Receiver<Result<OperationResult, Error>>,
    cancellation: CancellationToken,
}

impl Operation {
    fn next_id() -> String {
        format!("op-{}", NEXT_OPERATION.fetch_add(1, Ordering::Relaxed))
    }

    /// Creates an already-completed failed operation.
    pub fn failed(error: Error) -> Self {
        let (event_tx, events) = mpsc::unbounded_channel();
        let (result_tx, result) = oneshot::channel();
        let cancellation = CancellationToken::new();
        let _ = event_tx.send(Event::Warning {
            message: error.message.clone(),
        });
        drop(event_tx);
        let _ = result_tx.send(Err(error));
        Self {
            id: Self::next_id(),
            events,
            result,
            cancellation,
        }
    }

    /// Creates an already-completed unsupported operation.
    pub fn not_implemented(what: impl AsRef<str>) -> Self {
        Self::failed(Error::not_implemented(what))
    }

    pub(crate) fn spawn<F>(run: F) -> Self
    where
        F: FnOnce(
                String,
                mpsc::UnboundedSender<Event>,
                CancellationToken,
            )
                -> Pin<Box<dyn Future<Output = Result<OperationResult, Error>> + Send>>
            + Send
            + 'static,
    {
        let id = Self::next_id();
        let (event_tx, events) = mpsc::unbounded_channel();
        let (result_tx, result) = oneshot::channel();
        let cancellation = CancellationToken::new();
        let operation = Self {
            id: id.clone(),
            events,
            result,
            cancellation: cancellation.clone(),
        };
        let task = run(id, event_tx, cancellation);
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = result_tx.send(task.await);
            });
        } else {
            let _ = result_tx.send(Err(Error::new(
                ErrorCode::Internal,
                "an async runtime is required to execute a plan",
            )));
        }
        operation
    }

    /// Waits for the next event, returning `None` after the event stream closes.
    pub async fn next_event(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    /// Requests cancellation. Repeated calls are safe.
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Waits for and consumes the operation's final result.
    pub async fn result(self) -> Result<OperationResult, Error> {
        match self.result.await {
            Ok(result) => result,
            Err(_) => Err(Error::new(
                ErrorCode::Internal,
                "operation result was dropped",
            )),
        }
    }

    #[cfg(test)]
    pub(crate) fn cancellable() -> Self {
        let (event_tx, events) = mpsc::unbounded_channel();
        let (result_tx, result) = oneshot::channel();
        let cancellation = CancellationToken::new();
        let cancellation_for_task = cancellation.clone();
        tokio::spawn(async move {
            cancellation_for_task.cancelled().await;
            drop(event_tx);
            let _ = result_tx.send(Err(Error::new(ErrorCode::Cancelled, "operation cancelled")));
        });
        Self {
            id: Self::next_id(),
            events,
            result,
            cancellation,
        }
    }
}
