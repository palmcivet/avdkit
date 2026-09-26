use std::sync::atomic::{AtomicU64, Ordering};

use model::{Error, ErrorCode, Event, OperationResult};
use tokio::sync::{mpsc, oneshot, watch};

static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct Operation {
    pub id: String,
    events: mpsc::UnboundedReceiver<Event>,
    result: oneshot::Receiver<Result<OperationResult, Error>>,
    cancel: watch::Sender<bool>,
}

impl Operation {
    fn next_id() -> String {
        format!("op-{}", NEXT_OPERATION.fetch_add(1, Ordering::Relaxed))
    }

    pub fn failed(error: Error) -> Self {
        let (event_tx, events) = mpsc::unbounded_channel();
        let (result_tx, result) = oneshot::channel();
        let (cancel, _cancel_rx) = watch::channel(false);
        let _ = event_tx.send(Event::Warning {
            message: error.message.clone(),
        });
        drop(event_tx);
        let _ = result_tx.send(Err(error));
        Self {
            id: Self::next_id(),
            events,
            result,
            cancel,
        }
    }

    pub fn not_implemented(what: impl AsRef<str>) -> Self {
        Self::failed(Error::not_implemented(what))
    }

    pub async fn next_event(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    pub fn cancel(&self) {
        let _ = self.cancel.send(true);
    }

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
        let (cancel, mut cancel_rx) = watch::channel(false);
        tokio::spawn(async move {
            let _ = cancel_rx.changed().await;
            drop(event_tx);
            let _ = result_tx.send(Err(Error::new(ErrorCode::Cancelled, "operation cancelled")));
        });
        Self {
            id: Self::next_id(),
            events,
            result,
            cancel,
        }
    }
}
