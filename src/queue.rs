//! Count- and byte-bounded channels. Permits follow messages through consumption.
use crate::model::ErrorClass;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use tokio_util::sync::CancellationToken;

pub struct Packet<T> {
    pub value: T,
    _bytes: OwnedSemaphorePermit,
}
pub struct Sender<T> {
    tx: mpsc::Sender<Packet<T>>,
    bytes: Arc<Semaphore>,
    limit: usize,
    notify: Arc<tokio::sync::Notify>,
}
impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            bytes: self.bytes.clone(),
            limit: self.limit,
            notify: self.notify.clone(),
        }
    }
}
pub struct Receiver<T> {
    rx: mpsc::Receiver<Packet<T>>,
    bytes: Arc<Semaphore>,
    limit: usize,
    notify: Arc<tokio::sync::Notify>,
}
pub fn channel<T>(messages: usize, bytes: usize) -> (Sender<T>, Receiver<T>) {
    channel_with_notify(messages, bytes, Arc::new(tokio::sync::Notify::new()))
}
pub fn channel_with_notify<T>(
    messages: usize,
    bytes: usize,
    notify: Arc<tokio::sync::Notify>,
) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::channel(messages);
    let budget = Arc::new(Semaphore::new(bytes));
    (
        Sender {
            tx,
            bytes: budget.clone(),
            limit: bytes,
            notify: notify.clone(),
        },
        Receiver {
            rx,
            bytes: budget,
            limit: bytes,
            notify: notify.clone(),
        },
    )
}
impl<T> Sender<T> {
    pub async fn send(
        &self,
        value: T,
        bytes: usize,
        cancel: &CancellationToken,
    ) -> Result<(), ErrorClass> {
        if bytes > self.limit || bytes > u32::MAX as usize {
            return Err(ErrorClass::Capacity);
        }
        tokio::select! {
            biased;
            _=cancel.cancelled()=>Err(ErrorClass::Cancelled),
            result=async {
                let permit=self.bytes.clone().acquire_many_owned(bytes.max(1) as u32).await.map_err(|_|ErrorClass::Cancelled)?;
                self.tx.send(Packet {value,_bytes:permit}).await.map_err(|_|ErrorClass::Cancelled)?;self.notify.notify_one();Ok(())
            }=>result,
        }
    }
    pub fn try_send(&self, value: T, bytes: usize) -> Result<(), ErrorClass> {
        if bytes > self.limit || bytes > u32::MAX as usize {
            return Err(ErrorClass::Capacity);
        }
        let permit = self
            .bytes
            .clone()
            .try_acquire_many_owned(bytes.max(1) as u32)
            .map_err(|_| ErrorClass::Capacity)?;
        let result = self
            .tx
            .try_send(Packet {
                value,
                _bytes: permit,
            })
            .map_err(|_| ErrorClass::Capacity);
        if result.is_ok() {
            self.notify.notify_one();
        }
        result
    }
}
impl<T> Receiver<T> {
    pub fn notifier(&self) -> Arc<tokio::sync::Notify> {
        self.notify.clone()
    }
    pub async fn recv(&mut self) -> Option<Packet<T>> {
        self.rx.recv().await
    }
    pub fn try_recv(&mut self) -> Option<Packet<T>> {
        self.rx.try_recv().ok()
    }
    pub fn queued_bytes(&self) -> usize {
        self.limit - self.bytes.available_permits()
    }
    pub fn len(&self) -> usize {
        self.rx.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rx.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cancelled_full_queue_reclaims_bytes() {
        let (tx, mut rx) = channel(1, 8);
        let cancel = CancellationToken::new();
        tx.send(1, 8, &cancel).await.unwrap();
        assert_eq!(rx.queued_bytes(), 8);
        let task = tokio::spawn({
            let tx = tx.clone();
            let c = cancel.clone();
            async move { tx.send(2, 8, &c).await }
        });
        tokio::task::yield_now().await;
        cancel.cancel();
        assert_eq!(task.await.unwrap(), Err(ErrorClass::Cancelled));
        let packet = rx.recv().await.unwrap();
        assert_eq!(packet.value, 1);
        drop(packet);
        assert_eq!(rx.queued_bytes(), 0);
        assert_eq!(tx.try_send(3, 9), Err(ErrorClass::Capacity));
    }
}
