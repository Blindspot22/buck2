/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use starlark::environment::GlobalsBuilder;
use starlark::values::FrozenValue;

use crate::interpreter::rule_defs::transitive_set::FrozenTransitiveSetDefinition;
use crate::interpreter::rule_defs::transitive_set::transitive_set::TransitiveSetGen;
use crate::interpreter::rule_defs::transitive_set::transitive_set_args_projection::TransitiveSetArgsProjectionGen;
use crate::interpreter::rule_defs::transitive_set::transitive_set_json_projection::TransitiveSetJsonProjectionGen;
use crate::interpreter::rule_defs::transitive_set::traversal::TransitiveSetProjectionTraversalGen;
use crate::interpreter::rule_defs::transitive_set::traversal::TransitiveSetTraversalGen;

#[starlark_module]
#[starlark_types(
    TransitiveSetGen<FrozenValue> as TransitiveSet,
    TransitiveSetArgsProjectionGen<FrozenValue> as TransitiveSetArgsProjection,
    FrozenTransitiveSetDefinition as TransitiveSetDefinition,
    TransitiveSetJsonProjectionGen<FrozenValue> as TransitiveSetJsonProjection,
    TransitiveSetTraversalGen<FrozenValue> as TransitiveSetIterator,
    TransitiveSetProjectionTraversalGen<FrozenValue> as TransitiveSetArgsProjectionIterator
)]
pub fn register_transitive_set_types(globals: &mut GlobalsBuilder) {}
