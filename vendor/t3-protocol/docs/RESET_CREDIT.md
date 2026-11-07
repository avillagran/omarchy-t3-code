# Reset credit command

`api::provider::consume_reset_credit` sends `provider.consumeResetCredit` once.
It requires `providers:manage` and is classified as an unrepeatable mutation.
The host must obtain the user's confirmation before spending a credit; this
library does not supply UI or make account-selection decisions.

The input is the native union:

```rust,no_run
use t3_protocol::api::provider::{consume_reset_credit, ResetCreditInput};
# fn example(connection: &t3_protocol::Connection) -> t3_protocol::Result<()> {
let input = ResetCreditInput::Provider { instance_id: "codex".into() };
let result = consume_reset_credit(connection, &input)?;
# let _ = result;
# Ok(()) }
```

A source observation instead supplies `ResetCreditInput::Source` with the exact
`source_id`, `account_id` and `credit_id` from that observation. The source ID
remains an opaque raw string. Account and credit IDs use JavaScript whitespace
trimming and must remain nonempty. A valid provider instance takes precedence
when decoding an object that also contains source fields, matching the native
schema's union order. Excess fields do not enter the wire payload.

An ambiguous transport result must not be replayed automatically. The typed
function and generic registry entry have the same single-send policy. The result
remains the native JSON response so callers retain its outcome and optional
warning without inventing success or triggering another redemption.

Reference: nightly 4288, commit
`32a0531748c44b25072d7495ce0b9bd7e9450a87`.
`tools/generate-reset-credit-input-fixtures.mjs` executes the actual Effect 4.1.2
contract after checking every staged native source hash. Its 67 cases cover
union priority, invalid provider IDs, raw source IDs, JavaScript trimming,
non-string values and excess properties. Synthetic transport and C-ABI tests
verify scope, registry metadata and absence of retries. They do not spend an
account credit or prove a live provider's redemption service.
