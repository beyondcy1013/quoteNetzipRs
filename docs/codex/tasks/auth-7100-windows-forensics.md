# Windows 7100 Authentication Forensics

Updated: 2026-08-06

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
