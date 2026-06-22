# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

def _platform(ctx):
    configuration = ConfigurationInfo(
        constraints = {
            ctx.attrs.setting.label.raw_target(): ConstraintValueInfo(
                setting = ctx.attrs.setting[ConstraintSettingInfo],
                label = ctx.label.raw_target(),
            ),
        },
        values = {},
    )

    platform = ExecutionPlatformInfo(
        label = ctx.label.raw_target(),
        configuration = configuration,
        executor_config = CommandExecutorConfig(
            local_enabled = True,
            remote_enabled = True,
            remote_execution_properties = {
                "platform": "linux-remote-execution",
            },
            remote_execution_use_case = "buck2-testing",
            use_limited_hybrid = read_config("build", "use_limited_hybrid", "true") == "true",
        ),
    )

    return [
        DefaultInfo(),
        platform,
        configuration,
    ]

platform = rule(
    impl = _platform,
    attrs = {
        "setting": attrs.configuration_label(),
    },
)

def _platforms(ctx):
    return [
        DefaultInfo(),
        ExecutionPlatformRegistrationInfo(
            platforms = [x[ExecutionPlatformInfo] for x in ctx.attrs.platforms],
        ),
    ]

platforms = rule(
    impl = _platforms,
    attrs = {
        "platforms": attrs.list(attrs.dep(providers = [ExecutionPlatformInfo])),
    },
)

def _target_platform(ctx):
    return [
        DefaultInfo(),
        PlatformInfo(
            label = str(ctx.label.raw_target()),
            configuration = ConfigurationInfo(constraints = {}, values = {}),
        ),
    ]

target_platform = rule(
    impl = _target_platform,
    attrs = {},
)

def _config_setting(ctx):
    return [DefaultInfo(), ConstraintSettingInfo(label = ctx.label.raw_target())]

config_setting = rule(
    impl = _config_setting,
    attrs = {},
)

def _producer_impl(ctx):
    out = ctx.actions.declare_output("out", has_content_based_path = False)
    ctx.actions.run(
        cmd_args([
            "fbpython",
            "-c",
            "import sys; open(sys.argv[1], 'w').write('produced\\n')",
            out.as_output(),
        ]),
        category = "produce",
        prefer_remote = True,
    )
    return [DefaultInfo(default_output = out)]

producer = rule(impl = _producer_impl, attrs = {})

def _consumer_impl(ctx):
    src = ctx.attrs.dep[DefaultInfo].default_outputs[0]
    out = ctx.actions.declare_output("consumer_out", has_content_based_path = False)
    ctx.actions.run(
        cmd_args([
            "fbpython",
            "-c",
            "import sys; open(sys.argv[1], 'w').write(open(sys.argv[2]).read())",
            out.as_output(),
            src,
        ]),
        category = "consume",
        prefer_local = ctx.attrs.prefer_local,
        eager_materialization_enabled = True,
    )
    return [DefaultInfo(default_output = out)]

consumer = rule(
    impl = _consumer_impl,
    attrs = {
        "dep": attrs.dep(),
        "prefer_local": attrs.bool(default = False),
    },
)
