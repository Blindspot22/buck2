# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict


from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test
from buck2.tests.e2e_util.helper.utils import filter_events, random_string


async def _registered_dispatched_materialized(
    buck: Buck,
) -> tuple[set[str], set[str], set[str]]:
    """Returns (registered, eagerly_dispatched, materialized) path sets from events."""
    register_events = await filter_events(
        buck,
        "Event",
        "data",
        "Instant",
        "data",
        "MaterializerCommand",
        "data",
        "RegisterEagerPaths",
    )
    dispatch_events = await filter_events(
        buck,
        "Event",
        "data",
        "Instant",
        "data",
        "MaterializerCommand",
        "data",
        "EagerDispatchOnDeclare",
    )
    mat_end_events = await filter_events(
        buck,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "Materialization",
    )
    registered = {p for ev in register_events for p in ev["paths"]}
    eagerly_dispatched = {ev["path"] for ev in dispatch_events}
    materialized = {ev["path"] for ev in mat_end_events if ev.get("success")}
    return registered, eagerly_dispatched, materialized


@buck_test(data_dir="combined")
async def test_combined_inputs_register_dispatch_materialize(buck: Buck) -> None:
    """Tests eager materialization with regular, projected, and tset inputs."""
    await buck.build("//:consumer")

    (
        registered,
        eagerly_dispatched,
        materialized,
    ) = await _registered_dispatched_materialized(buck)
    found_basenames = {p.rsplit("/", 1)[-1] for p in registered}

    assert "regular_out" in found_basenames
    assert "out_dir" in found_basenames
    assert not any("/out_dir/file_a" in p for p in registered)

    for tset_member in ("leaf_a_out", "leaf_b_out", "node_x_out"):
        assert tset_member in found_basenames

    assert len(registered) == 5
    assert registered == eagerly_dispatched

    missing = registered - materialized
    assert not missing


@buck_test(data_dir="combined")
async def test_content_based_path_eager_materialization(buck: Buck) -> None:
    """Tests eager materialization with content-based paths."""
    await buck.build("//:consumer_content")

    (
        registered,
        eagerly_dispatched,
        materialized,
    ) = await _registered_dispatched_materialized(buck)

    # Registered cfg-based path but eagerly materialized but cfg-based and content-based paths.
    assert len(registered) == 1
    assert len(eagerly_dispatched) == 2
    assert len(materialized) == 2


async def _get_registered_paths(buck: Buck) -> set[str]:
    """Returns the set of paths registered for eager materialization."""
    register_events = await filter_events(
        buck,
        "Event",
        "data",
        "Instant",
        "data",
        "MaterializerCommand",
        "data",
        "RegisterEagerPaths",
    )
    return {p for ev in register_events for p in ev["paths"]}


@buck_test(data_dir="hybrid_modes")
async def test_eager_materialization_default_no_eager(buck: Buck) -> None:
    """Default config (limited hybrid) + default preference: should NOT eagerly materialize."""
    await buck.build(
        "//:consumer_default",
        "-c",
        f"test.cache_buster={random_string()}",
    )
    registered = await _get_registered_paths(buck)
    assert len(registered) == 0, (
        "Expected no eager materialization for default config + default preference"
    )


@buck_test(data_dir="hybrid_modes")
async def test_eager_materialization_full_hybrid_prefer_local(buck: Buck) -> None:
    """Full hybrid + prefer_local: should eagerly materialize."""
    await buck.build(
        "//:consumer_prefer_local",
        "-c",
        f"test.cache_buster={random_string()}",
        "-c",
        "build.use_limited_hybrid=false",
    )
    registered = await _get_registered_paths(buck)
    assert len(registered) > 0, (
        "Expected eager materialization for full hybrid + prefer_local"
    )


@buck_test(data_dir="hybrid_modes")
async def test_eager_materialization_full_hybrid_default_pref(buck: Buck) -> None:
    """Full hybrid + default preference: should eagerly materialize."""
    await buck.build(
        "//:consumer_default",
        "-c",
        f"test.cache_buster={random_string()}",
        "-c",
        "build.use_limited_hybrid=false",
    )
    registered = await _get_registered_paths(buck)
    assert len(registered) > 0, (
        "Expected eager materialization for full hybrid + default preference"
    )


@buck_test(data_dir="hybrid_modes")
async def test_eager_materialization_limited_prefer_local(buck: Buck) -> None:
    """Limited hybrid + prefer_local: should eagerly materialize."""
    await buck.build(
        "//:consumer_prefer_local",
        "-c",
        f"test.cache_buster={random_string()}",
        "-c",
        "build.use_limited_hybrid=true",
    )
    registered = await _get_registered_paths(buck)
    assert len(registered) > 0, (
        "Expected eager materialization for limited hybrid + prefer_local"
    )


@buck_test(data_dir="hybrid_modes")
async def test_eager_materialization_limited_default_pref(buck: Buck) -> None:
    """Limited hybrid + default preference: should NOT eagerly materialize."""
    await buck.build(
        "//:consumer_default",
        "-c",
        f"test.cache_buster={random_string()}",
        "-c",
        "build.use_limited_hybrid=true",
    )
    registered = await _get_registered_paths(buck)
    assert len(registered) == 0, (
        "Expected no eager materialization for limited hybrid + default preference"
    )
