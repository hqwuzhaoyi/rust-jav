use std::{sync::Arc, time::Duration};

const MAX_BLOCKING_IMAGE_WORK: usize = 4;
const MAX_BLOCKING_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const BLOCKING_IMAGE_BUDGET_WAIT: Duration = Duration::from_millis(250);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("image blocking-work budget is busy")]
    Busy,
    #[error("image blocking worker failed")]
    Worker,
}

#[derive(Clone)]
pub(crate) struct ImageBlockingBudget(Arc<Inner>);

struct Inner {
    work: Arc<tokio::sync::Semaphore>,
    bytes: Arc<tokio::sync::Semaphore>,
    max_bytes: usize,
    wait: Duration,
}

impl ImageBlockingBudget {
    pub(crate) fn new() -> Self {
        Self::with_limits(
            MAX_BLOCKING_IMAGE_WORK,
            MAX_BLOCKING_IMAGE_BYTES,
            BLOCKING_IMAGE_BUDGET_WAIT,
        )
    }

    fn with_limits(max_work: usize, max_bytes: usize, wait: Duration) -> Self {
        Self(Arc::new(Inner {
            work: Arc::new(tokio::sync::Semaphore::new(max_work)),
            bytes: Arc::new(tokio::sync::Semaphore::new(max_bytes)),
            max_bytes,
            wait,
        }))
    }

    pub(crate) async fn run<F, R>(&self, reserved_bytes: usize, work: F) -> Result<R, Error>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        if reserved_bytes > self.0.max_bytes {
            return Err(Error::Busy);
        }
        let work_permit = tokio::time::timeout(self.0.wait, self.0.work.clone().acquire_owned())
            .await
            .map_err(|_| Error::Busy)?
            .map_err(|_| Error::Busy)?;
        let byte_permits = u32::try_from(reserved_bytes).map_err(|_| Error::Busy)?;
        let byte_permit = tokio::time::timeout(
            self.0.wait,
            self.0.bytes.clone().acquire_many_owned(byte_permits),
        )
        .await
        .map_err(|_| Error::Busy)?
        .map_err(|_| Error::Busy)?;
        tokio::task::spawn_blocking(move || {
            let _work_permit = work_permit;
            let _byte_permit = byte_permit;
            work()
        })
        .await
        .map_err(|_| Error::Worker)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };

    use super::{Error, ImageBlockingBudget};

    struct ReleaseOnDrop(Arc<AtomicBool>);

    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn repeated_receiver_cancellation_keeps_work_and_bytes_reserved_until_closure_exit() {
        assert_cancel_storm_is_bounded(2, 100, 1, 2).await;
        assert_cancel_storm_is_bounded(8, 2, 1, 2).await;
    }

    async fn assert_cancel_storm_is_bounded(
        max_work: usize,
        max_bytes: usize,
        reserved_bytes: usize,
        expected_started: usize,
    ) {
        let budget =
            ImageBlockingBudget::with_limits(max_work, max_bytes, Duration::from_millis(25));
        let started = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(AtomicBool::new(false));
        let _release_on_drop = ReleaseOnDrop(release.clone());
        let mut receivers = Vec::new();

        for _ in 0..20 {
            let budget = budget.clone();
            let started = started.clone();
            let release = release.clone();
            receivers.push(tokio::spawn(async move {
                budget
                    .run(reserved_bytes, move || {
                        started.fetch_add(1, Ordering::SeqCst);
                        while !release.load(Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                    })
                    .await
            }));
        }

        tokio::time::timeout(Duration::from_secs(1), async {
            while started.load(Ordering::SeqCst) < expected_started {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        for receiver in &receivers {
            receiver.abort();
        }
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(
            started.load(Ordering::SeqCst),
            expected_started,
            "cancelled receivers must not allow an unbounded blocking queue to start"
        );
        assert_eq!(budget.run(reserved_bytes, || ()).await, Err(Error::Busy));

        release.store(true, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(budget.run(reserved_bytes, || 7).await.unwrap(), 7);
    }
}
