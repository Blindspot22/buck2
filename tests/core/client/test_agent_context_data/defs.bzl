def _pass_impl(ctx):
    out = ctx.actions.declare_output("out")
    ctx.actions.write(out, "pass")
    return [DefaultInfo(default_output = out)]

pass_ = rule(impl = _pass_impl, attrs = {})
