# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

def _impl(ctx):
    constraints = {}
    for dep in ctx.attrs.constraint_values:
        constraint_value_info = dep[ConstraintValueInfo]
        constraints[constraint_value_info.setting.label] = constraint_value_info
    return [
        DefaultInfo(),
        PlatformInfo(
            label = str(ctx.label.raw_target()),
            configuration = ConfigurationInfo(
                constraints = constraints,
                values = {},
            ),
        ),
    ]

test_platform = rule(
    impl = _impl,
    is_configuration_rule = True,
    attrs = {
        "constraint_values": attrs.list(attrs.dep(providers = [ConstraintValueInfo])),
    },
)
