# WPT selector parsing fixtures

Source: [web-platform-tests/wpt](https://github.com/web-platform-tests/wpt)
`css/selectors/parsing/parse-*.html` + `invalid-pseudos.html`, revision
`b89af32bc8f42d678f444eb0703bca015ddcf240` (2026-09-05).

Each `*.json` mirrors one upstream `*.html` file. Cases were extracted from
the inline `<script>` blocks by a one-off extractor; the loop helpers
(`run_tests_on_anplusb_selector`, `assert_valid`) are hand-expanded.

Case kinds mirror `css/support/parsing-testcommon.js` semantics:

| kind        | upstream call                    | harness assertion      |
|-------------|----------------------------------|------------------------|
| `valid`     | `test_valid_selector`            | strict parse succeeds  |
| `forgiving` | `test_valid_forgiving_selector` / `assert_valid(false, …)` | strict parse succeeds (selector valid; forgiving contents only affect matching) |
| `invalid`   | `test_invalid_selector`          | strict parse fails     |

`serializations` fields are recorded but intentionally **not** asserted:
muskitty-selectors does not implement selector serialization (`selectorText`).
They become relevant once a serializer lands.

WPT is subject to the W3C 3-clause BSD license. These derived fixtures are
used for testing only.
