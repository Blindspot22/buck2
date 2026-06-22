/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::Dupe;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::priority_semaphore::Priority;
use super::priority_semaphore::PriorityPermit;
use super::priority_semaphore::PrioritySemaphore;

/// Result of attempting to acquire a permit with priority control
pub enum AcquirePermitResult {
    /// Successfully acquired the permit
    Acquired(PriorityPermit),
    /// Cancelled before permit could be acquired
    Cancelled,
}

#[derive(Clone)]
pub struct DynamicPriorityHandle {
    priority_tx: watch::Sender<Priority>,
    priority_rx: watch::Receiver<Priority>,
    cancel_token: CancellationToken,
}

impl Dupe for DynamicPriorityHandle {}

impl std::fmt::Debug for DynamicPriorityHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicPriorityHandle")
            .field("priority", &self.priority())
            .field("cancel_triggered", &self.cancel_token.is_cancelled())
            .finish()
    }
}

impl DynamicPriorityHandle {
    pub fn new(priority: Priority) -> Self {
        let (tx, rx) = watch::channel(priority);
        Self {
            priority_tx: tx,
            priority_rx: rx,
            cancel_token: CancellationToken::new(),
        }
    }

    pub fn priority(&self) -> Priority {
        *self.priority_rx.borrow()
    }

    pub fn update(&self, priority: Priority) {
        let _ = self.priority_tx.send(priority);
    }

    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    pub async fn priority_changed(&mut self) {
        let _ = self.priority_rx.changed().await;
    }

    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel_token
    }

    /// Acquire a permit from the semaphore, respecting dynamic priority changes and cancellation.
    ///
    /// Uses a biased `select!` loop:
    /// - If cancelled → returns `AcquirePermitResult::Cancelled`
    /// - If priority changed → restarts acquisition at the new priority
    /// - Otherwise → acquires the permit normally and returns `AcquirePermitResult::Acquired`
    pub async fn acquire_permit(
        &mut self,
        sem: &PrioritySemaphore,
        permits: u32,
    ) -> buck2_error::Result<AcquirePermitResult> {
        let cancel_token = self.cancel_token().clone();
        loop {
            let priority = self.priority();
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    return Ok(AcquirePermitResult::Cancelled);
                }
                _ = self.priority_changed() => {
                    continue;
                }
                permit = sem.acquire_many(permits, priority) => {
                    return permit.map(AcquirePermitResult::Acquired);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_priority_upgrade_completes_before_low() {
        let sem = PrioritySemaphore::new("test", 1);
        // Hold the only permit
        let hold = sem.acquire(Priority::High).await.unwrap();

        // Send a second acquire to block the manager on the semaphore.
        // While the manager is stuck here, subsequent requests queue up untouched.
        let sem_blocker = sem.clone();
        let _blocker = tokio::spawn(async move {
            let _permit = sem_blocker.acquire(Priority::High).await.unwrap();
        });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        // Now queue 2 Lows — both sit in the channel since the manager is busy.
        let (order_tx, mut order_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut low_handle = DynamicPriorityHandle::new(Priority::Low);
        let order_tx1 = order_tx.clone();
        let sem1 = sem.clone();
        let _low_task = tokio::spawn(async move {
            let _permit = low_handle.acquire_permit(&sem1, 1).await.unwrap();
            order_tx1.send("low").unwrap();
        });

        let mut upgradeable_handle = DynamicPriorityHandle::new(Priority::Low);
        let upgrade_control = upgradeable_handle.clone();
        let order_tx2 = order_tx.clone();
        let sem2 = sem.clone();
        let _upgraded_task = tokio::spawn(async move {
            let _permit = upgradeable_handle.acquire_permit(&sem2, 1).await.unwrap();
            order_tx2.send("upgraded").unwrap();
        });

        // Let both Low requests land in the channel
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        // Upgrade the second one to High — drops its Low request, re-queues in high_tx
        upgrade_control.update(Priority::High);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        // Release the held permit. Manager finishes the blocker, then polls the
        // stream: high_rx has the upgraded request → serves it before the Low one.
        drop(hold);

        let first = order_rx.recv().await.unwrap();
        assert_eq!(
            first, "upgraded",
            "upgraded (High) task should complete before low task"
        );
    }

    #[tokio::test]
    async fn test_cancel_before_acquire() {
        let sem = PrioritySemaphore::new("test", 4);
        let mut handle = DynamicPriorityHandle::new(Priority::High);

        // Cancel before acquiring
        handle.cancel();

        let result = handle.acquire_permit(&sem, 1).await.unwrap();
        assert!(
            matches!(result, AcquirePermitResult::Cancelled),
            "pre-cancelled handle should return Cancelled immediately"
        );
    }
}
