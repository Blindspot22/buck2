# buck2 e2e tests

This directory contains tests for buck2. Primary constituents:

 - `core/` - the primary and fully endorsed set of integration tests for buck2 core code. If you're
   working on buck2 itself, this is probably what you want.
 - `e2e_util/` - the test framework for the integration tests.
 - `e2e/` and `meta_only/e2e` - a hodgepodge of tests covering a combination of buck2 itself, the
   prelude, some macros, and various integrations. Avoid if possible. Strongly avoid in favor of
   `core/` if testing buck2 core.
 - `targets/` - target definitions accessed by `e2e` tests.
 - `prelude/` - there is currently no fully endorsed testing strategy for the prelude. This
   directory is an attempt at creating one, however its still immature and there are gaps.
   Trendsetters are welcome to try it.
 - An assortment of other things that mostly shouldn't be here.

`core` and `prelude` tests are visible in open source but not executed there.

Some of these directories have their own `README.md` files.
