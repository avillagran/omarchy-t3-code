# Native speech control dispatcher

The client consumes published SDK revision
`74f0052ea0770df1fd754ca82638e9107a81e538` byte-for-byte across 129 vendor
files. There are no private SDK patches. Its Unicode 17 normalization dependency
is pinned to 0.1.25; Cargo.lock adds only that dependency and the generated legal
manifest includes its original notices.

The actual write dispatcher accepts `op: speech` for status, local model
catalog/benchmarks, explicit model preparation, download/selection/cancellation/
removal, acceleration/unload controls, cloud key presence/catalog and explicit
key changes. The worker handles these off the ordinary write queue. None of
these controls records audio, sends PCM or starts transcript processing.

Every operation selects the environment's write connection and rechecks its
current authenticated session. Reads/preparation require
`orchestration:operate`; model/configuration/key changes require
`settings:write`. The preflight and operation use the same connection, including
relay/DPoP connections. A read credential cannot authorize a different write
identity. The server remains the final scope authority.

Success returns `{ok:true,value}`. Known native invalid-request responses retain
their reason and trace ID as `{ok:false,failure}`; typed cloud failures retain
provider and reason. Transport uncertainty is an error and never authorizes a
mutation replay. Cloud key writes return no key. Unsupported native platforms
remain ordinary status responses.

The private test invokes the actual write dispatcher with separate read/write
credential files and a synthetic loopback server. It checks fresh scope
revocation, refusal before model mutation, native busy receipts, an ambiguous
mutation with exactly one send, current credential selection and secret-free
key-write output. Existing compiled-license tests verify the regenerated
dependency manifest. The dedicated runner freezes binaries/libraries under the
global compilation lock, checks source hashes, runs only the two reviewed
targets in a private offline rootless container, and retains fixtures. Strict
all-target compilation is separate from runtime selection.

This checkpoint provides the native controls and SDK consumer. Voice preference
views, independent destination/cleanup consent, host capture and safe transcript
insertion remain in progress. The complete native controller now has a separately
verified Qt artifact; see `NATIVE-VOICE-CONTROLLER-4314.md`. It does not
activate server capabilities or redirect the existing local dictation helper.
The global client runtime safety audit marker remains active.
