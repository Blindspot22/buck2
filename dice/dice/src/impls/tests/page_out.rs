/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! End-to-end tests for `Dice::page_out` and the worker's page-in step.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;
use tempfile::tempdir;

use crate::DiceKeyDyn;
use crate::DiceStorage;
use crate::api::computations::DiceComputations;
use crate::api::cycles::DetectCycles;
use crate::api::key::Key;
use crate::api::key::NoValueSerialize;
use crate::api::key::PagableValueSerialize;
use crate::api::key::ValueSerialize;
use crate::api::user_data::UserComputationData;
use crate::impls::dice::Dice;

/// Per-test compute counter, injected via `UserComputationData` so tests don't share state.
#[derive(Clone, Dupe)]
struct ComputeCounter(Arc<AtomicUsize>);

impl ComputeCounter {
    fn new() -> Self {
        Self(Arc::new(AtomicUsize::new(0)))
    }

    fn count(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Allocative, Clone, Dupe, Debug, Display, PartialEq, Eq, Hash, Pagable)]
#[pagable_typetag(DiceKeyDyn)]
struct PagableKey(u32);

#[async_trait]
impl Key for PagableKey {
    type Value = u64;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if let Ok(c) = ctx.per_transaction_data().data.get::<ComputeCounter>() {
            c.0.fetch_add(1, Ordering::SeqCst);
        }
        u64::from(self.0) * 100
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        PagableValueSerialize::<Self::Value>::new()
    }
}

#[derive(Allocative, Clone, Dupe, Debug, Display, PartialEq, Eq, Hash, Pagable)]
#[pagable_typetag(DiceKeyDyn)]
struct NonPagableKey(u32);

#[async_trait]
impl Key for NonPagableKey {
    type Value = u64;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        u64::from(self.0) * 7
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        NoValueSerialize::<Self::Value>::new()
    }
}

fn make_dice(storage: DiceStorage) -> Arc<Dice> {
    let mut builder = Dice::builder();
    builder.set_pagable_storage(storage);
    builder.build(DetectCycles::Disabled)
}

fn user_data_with_counter(counter: &ComputeCounter) -> UserComputationData {
    let mut d = UserComputationData::new();
    d.data.set(counter.dupe());
    d
}

/// Page out, then look up the same key — should hydrate from disk, not recompute.
#[tokio::test]
async fn paged_out_value_is_hydrated_on_next_lookup() -> anyhow::Result<()> {
    let counter = ComputeCounter::new();
    let tmp = tempdir()?;
    let storage = DiceStorage::open(tmp.path())?;
    let dice = make_dice(storage);

    let mut tx = dice
        .updater_with_data(user_data_with_counter(&counter))
        .commit()
        .await;
    let v1: u64 = tx.compute(&PagableKey(7)).await?;
    assert_eq!(v1, 700);
    assert_eq!(counter.count(), 1, "first lookup should compute");
    drop(tx);

    dice.wait_for_idle().await;
    dice.page_out().await?;

    let mut tx = dice
        .updater_with_data(user_data_with_counter(&counter))
        .commit()
        .await;
    let v2: u64 = tx.compute(&PagableKey(7)).await?;
    assert_eq!(v2, 700);
    assert_eq!(
        counter.count(),
        1,
        "second lookup should hydrate from storage, not recompute"
    );

    Ok(())
}

/// After page_out + rehydrate, multiple repeated lookups stay served from memory
/// (they go through the in-memory hydrated value, not back through the storage).
#[tokio::test]
async fn rehydrated_value_stays_in_memory() -> anyhow::Result<()> {
    let counter = ComputeCounter::new();
    let tmp = tempdir()?;
    let storage = DiceStorage::open(tmp.path())?;
    let dice = make_dice(storage);

    let mut tx = dice
        .updater_with_data(user_data_with_counter(&counter))
        .commit()
        .await;
    let _: u64 = tx.compute(&PagableKey(3)).await?;
    drop(tx);

    dice.wait_for_idle().await;
    dice.page_out().await?;

    // First post-page-out lookup hydrates and rehydrates.
    let mut tx = dice
        .updater_with_data(user_data_with_counter(&counter))
        .commit()
        .await;
    let _: u64 = tx.compute(&PagableKey(3)).await?;
    drop(tx);
    dice.wait_for_idle().await;

    // Subsequent lookups hit the in-memory hydrated node — no recompute, and no need
    // to call into storage again. We verify "no recompute" via the counter; we trust
    // that the lookup result was VersionedGraphResult::Match (not MatchPagedOut).
    for _ in 0..5 {
        let mut tx = dice
            .updater_with_data(user_data_with_counter(&counter))
            .commit()
            .await;
        let _: u64 = tx.compute(&PagableKey(3)).await?;
        drop(tx);
    }

    assert_eq!(
        counter.count(),
        1,
        "all lookups after the initial compute should be cache hits"
    );

    Ok(())
}

/// Keys whose `value_serialize` returns `NoValueSerialize` should silently be skipped
/// by `page_out` — the node stays hydrated, lookups continue to hit the in-memory cache.
#[tokio::test]
async fn page_out_skips_no_value_serialize_keys() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let storage = DiceStorage::open(tmp.path())?;
    let dice = make_dice(storage);

    let mut tx = dice.updater().commit().await;
    let v1: u64 = tx.compute(&NonPagableKey(5)).await?;
    assert_eq!(v1, 35);
    drop(tx);

    dice.wait_for_idle().await;
    dice.page_out().await?;

    // Lookup should still succeed without panic. If page_out had paged this node out,
    // the worker would try to hydrate via `NoValueSerialize::pagable_deserialize_value`
    // which is `unimplemented!()` — that would panic. So a successful lookup confirms
    // the node was correctly skipped.
    let mut tx = dice.updater().commit().await;
    let v2: u64 = tx.compute(&NonPagableKey(5)).await?;
    assert_eq!(v2, 35);

    Ok(())
}

/// `Dice::page_out` is a no-op when no `DiceStorage` was configured.
#[tokio::test]
async fn page_out_without_storage_is_noop() -> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Disabled);
    dice.page_out().await?;
    Ok(())
}
