# Protocol Research & Redaction Guide (`RESEARCH.md`)

## 1. Scientific Loop

1. **Observe**: Inspect URL, DOM, and Network XHR in Steel Browser (`192.168.8.11:13000`).
2. **Hypothesize**: Record candidate endpoints as `HYPOTHESIS` in `docs/research/sinopec.md`.
3. **Experiment**: Change one parameter at a time (e.g., date window A vs date window B).
4. **Verify**: Reproduce the call via `SinopecHttpTransport` (`reqwest`) after closing the browser tab.
5. **Implement & Test**: Add redacted fixtures under `tests/fixtures/` and unit tests in `server/tests/`.

## 2. Mandatory Redaction (`Redactor`)

Before any capture is written to `/data/research/` or `tests/fixtures/`, `Redactor` masks:
- 19-digit fuel card numbers (`1000111200000008816` -> `****8816`)
- 11-digit mobile numbers (`138****5678`)
- 18-character ID numbers and Unified Social Credit Codes (`911***********CDEF`)
- `Cookie`, `Set-Cookie`, `Authorization`, `password`, `smsYzm`, `random_code_sms`
