# Windows 7100 Authentication Forensics

Updated: 2026-08-07

## Objective

Produce reproducible Windows evidence for real account `1522` authentication on port `7100`, so the
native Rust implementation can be evaluated against the vendor process without changing the Rust
protocol or the Linux production service.

## Safety Boundary

- Real credentials may be read from an existing permission-controlled runtime source and used for
  the authorized login.
- Passwords and tokens must never be printed, serialized, screenshotted, placed on a command line,
  committed, included in chat, or written to ordinary temporary files.
- Raw captures and raw hook buffers stay in a project-local access-controlled work directory and
  are deleted after sanitized evidence is produced.
- Do not modify or replace the Linux production service or Rust binary.
- Avoid repeated login while the production Wine account session may be active. Prefer one baseline
  login and reuse that process for reconnect observations.

## Required Evidence

- Success and reconnect/failover packet captures with credential-bearing payload ranges masked.
- Timestamped process, module, connection tuple, packet length, capture offset, and SHA-256 evidence.
- Field and transform maps covering account, password, device identity, nonce/time, token,
  `Tdx_Encrypt`, `penc`, and ZSTD dictionary participation.
- A final native-Rust readiness decision, remaining unknowns, session state requirements, and a
  minimal implementation sequence.

## Acceptance

- No retained deliverable contains a password or reusable session token.
- Every packet conclusion links to a capture offset and evidence-file SHA-256.
- Any unavailable failure scenario is explicitly marked unproven rather than inferred from the
  successful path.

## 2026-08-06 Result

- A fresh real-account baseline completed with `网际风.exe` PID `40504`; production TdxW PID
  `50980` was not stopped or modified.
- 7100 is the `认证|测速` probe (`427 -> 345`). The actual `认证|登录` request was observed on
  6100 (`633` bytes), followed by 6100 dictionary/download traffic and 5188 quote connections.
- The decompressed account field matches `1522`; password evidence retains only offset 134 and
  declared length 12.
- The installed `Stock.字典` SHA-256
  `8f44f49cbf10c8d203d9cabbda256da37c1f7d43e7f99b60c08d077c47b2bc68` decodes all 66
  fresh `field44=12` frames when used as a raw-content ZSTD dictionary.
- No test-process 7709/0547 connection appeared. Pre-existing 7708/7719 production traffic was
  excluded from the success deliverable.
- Controlled unreachable/reconnect/failover experiments were not repeated because they could
  disrupt the production account session. The historical reconnect capture is retained only as
  an explicitly labeled comparison.
- Deliverables live in `docs/forensics/7100-auth-20260806/`.

## 2026-08-07 Narrow Rust Reproduction Consensus

### Goal

Turn the current standalone authentication proof into a protocol-aligned Rust implementation for
7100/6100 probing, formal 6100 login, `Stock.字典` response decoding, and 5188 server-list parsing.

### Boundaries

- Use the existing `auth_7100_client` module and repository ZSTD dependency rather than creating a
  second authentication stack.
- Treat 7100 as a probe endpoint and 6100 as the formal login endpoint.
- Read real credentials only through the existing permission-controlled runtime source. Never log,
  serialize, commit, or retain the password or reusable session values.
- Permit at most one isolated real-account `1522` 6100 login for final acceptance. Do not stop or
  modify the production vendor process.
- Do not deploy or replace the Linux production service or binary in this task.

### Acceptance Criteria

- Tests prove the probe and login packets use their distinct protocol roles and endpoints.
- `field44=12` packets decode with `Stock.字典` as a raw-content ZSTD dictionary, with dictionary
  identity exposed only as length and SHA-256.
- The download-file response parser extracts valid 5188 endpoints without retaining credentials or
  opaque token values.
- Authentication success requires decoded protocol evidence, not only response role names.
- The result exposes the selected authentication endpoint, response roles and lengths, dictionary
  fingerprint, and discovered 5188 endpoints without exposing the password.
- One authorized live 6100 login succeeds, or the exact server-side incompatibility is recorded if
  the evidence-derived implementation is rejected.

### Key Decisions And Risks

- Extend the existing module; do not add a new dependency or a parallel client abstraction.
- Preserve unknown decoded fields as length/hash evidence until their semantics are proven.
- The retained sanitized capture masks the 6100 application payload, so representative decoded
  response bytes must come from existing constants/tests or the single authorized live session.
- Hard-coded follow-up packets may contain session- or installation-specific fields; the
  implementation must identify and validate their dynamic boundary before claiming portability.

### Out Of Scope

- `Stock.dll` injection, `send`/`recv` hooking, or dynamic `Tdx_Encrypt` capture.
- Controlled authentication failure, forced disconnect, automatic reconnect, or failover testing.
- Claiming 6100 session fields are 7709/0547 bootstrap parameters without new hook evidence.
- Resident-service integration, production deployment, or long-lived same-account ownership policy.

## 2026-08-07 Implementation Result

- Rust now has distinct evidence-derived builders and entry points for the credential-bearing
  6100/7100 probe and the formal 6100 login sequence. The login decoder validates the tracked
  `Stock.字典` by length and SHA-256 before using it as a raw-content ZSTD dictionary.
- One authorized real-account login reached the expected three-response sequence, decoded the
  dictionary responses, and found the decoded `登录成功` marker. No additional real login was run.
- That live run then returned `decoded download response contained no 5188 quote endpoint`.
  Offline investigation traced this to the server-list parser accepting only four-field rows while
  the installed vendor configuration contains three-field rows such as
  `name, host, port` for 5188.
- The parser now accepts both formats. A three-field row maps its single port to both `main_port`
  and `secondary_port`; four-field behavior is unchanged. Focused parser and authentication tests
  pass, and the authentication CLI builds.
- The three-field fix is offline-verified only. It was deliberately not live-retested because the
  task's one-login authorization was already consumed and another login could disrupt the active
  account session.
- Focused parser/authentication tests pass; the full library has 95 passing tests and one known
  Windows-only infrastructure failure because `tcpdump` is unavailable.
- The retained follow-up request templates are now offline-reproducible dynamic generators. Across
  26 local `field44=12` samples, C2 changes only decoded `u32 LE @124` and C3 changes only decoded
  `u32 LE @116`; both fields are labeled `编号`. Raw-content dictionary compression at level 3
  reproduces the baseline C2/C3 bytes exactly and updates all outer lengths when C3 becomes 334 B.
- The current 271-byte template still differs in total length from the fresh 267-byte capture, so
  the dynamic generator is proven against the retained local sample family, not yet against a new
  live server session.

### Current Readiness Decision

- Sufficient now: 6100/7100 probing, formal 6100 login packet construction, response-role
  validation, raw-content dictionary decoding, and three-/four-field 5188 list extraction.
- Still unproven: live-server acceptance of dynamically advanced follow-up numbers, the 267 B fresh
  variant's remaining four-byte difference, 6100 session-field semantics, `Tdx_Encrypt`
  participation in 7709 bootstrap, long-lived reconnect/failover behavior, and equivalence with
  the production 7709/0547 session chain.

## 2026-08-07 Dynamic Follow-Up Construction

- `src/auth_7100_client.rs` now builds C2 and C3 from decoded templates instead of replaying opaque
  compressed constants directly. The login flow uses request numbers 1 and 2, preserving the
  previously accepted wire bytes.
- The builder patches only the decoded `编号` field, compresses with the verified embedded
  `Stock.字典` at level 3, and updates packet length, compressed length, decoded length, and outer
  object-span length fields.
- Focused authentication tests pass 10/10, including byte-for-byte baseline equality and advanced
  number round trips. No credential was loaded and no network login was attempted.
- This closes the offline dynamic C2/C3 construction gap. It does not close the live 267 B variant,
  reconnect/failover policy, or 7709 `Tdx_Encrypt` equivalence gaps.

## 2026-08-07 Route-State Refinement

- Authentication success is now independent from a particular downstream quote port. A decoded
  `登录成功` marker plus a non-empty active server list confirms the narrow login result.
- `selected_quote_endpoint` continues to expose an optional 5188 route, while the result now also
  exposes `selected_7709_endpoint` for an independently managed 7709 bootstrap path.
- This distinction is required by repository evidence: the fresh account-1522 run selected 5188,
  while the tracked historical download response contains 10 active 7709 endpoints and no 5188.
- Offline tests cover a mixed 5188/7709 list and the tracked 7709-only download response. No new
  account login or production-process interaction was performed.
- The baseline `plain_0118.bin` / `cipher_0118.bin` files are not accepted as a proven
  `Tdx_Encrypt` pair. They predate the current task, do not match the tracked 7709 TCP streams, and
  the capture scripts overwrite uncorrelated events under fixed filenames.

## 2026-08-07 Resident Authentication Control

- Added an explicit resident-service `POST /api/auth/login` operation and `GET /api/auth/status`.
- Credentials are loaded only from `auth_credentials::load(None, None)`; HTTP never accepts or
  returns a password. The operation runs in `spawn_blocking` and rejects concurrent attempts.
- The service probes the configured 6100/7100 authentication candidates, then performs formal 6100
  login with the verified embedded dictionary. Runtime state exposes only account, probe outcomes,
  response roles/lengths, dictionary fingerprint, active-server count, selected 5188/7709 routes,
  timestamps, and a sanitized error string.
- The control is disabled by default; `NETZIP_AUTH_LOGIN_ENABLED=1` is required before any network
  operation can be triggered. Startup authentication, automatic reconnect, and automatic 5188
  ingestion are intentionally absent until session ownership and follow-up packet semantics are
  proven from fresh Windows evidence.
- A new attempt clears prior response, dictionary, server-count, and endpoint fields before entering
  `running`, so a failed retry cannot present stale success state.
