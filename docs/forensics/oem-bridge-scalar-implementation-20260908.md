# OEM bridge scalar implementation

## Current Status And Historical Scope

Request `112336-18d31111735c6f5a` passed fmt, example tests, Clippy,
release and 50 conditional native/Rust vectors. See
[OEM bridge tools](oem-bridge-tools.md) for current usage and report hash.
The sections below are a reverse-chronological evidence journal; pending-build
and ownership statements describe their respective checkpoints, not current
instructions. Native fixture selection now uses `NETZIP_NATIVE_EXE` or the
relative default `samples/native/netzip.exe`, with the same pinned binary hash.
Full real-session bridge and callback parity remain unverified.

## Per-vector Evidence

The observational verifier now retains all50 input/native/Rust rows, including
packed amount and little-endian float32 output bytes. Read-only rerun against
the110111 binary passed50/50; source/binary hashes stayed unchanged.
`../../diagnostics/20260908-oem-bridge-scalar-v1/native-rust-vectors-v3.json`
SHA256 `e5f692e4f3c3631ee98b09ea653b39f861fe225e29592fe381b94d76302d9479`.
Three verifier regressions pass: matching input/bytes, signed-zero mismatch,
and packed-integer mismatch despite equal float output. This strengthens
reproducibility of conditional scalar evidence, not real-session parity.

## Verified direct-mode build and final copy

110111-18d31111735c6f54 completed status0 at11:02:33: fmt,7 Rust tests,
Clippy,release and50 native/Rust controls (32 mode2,6 mode1/category,12 direct).
Management checked the build log and current source/binary hashes against
`native-rust-matrix-direct-v2.json`; report SHA256
`858be92f111470b1d536f8c289ac3abcb650b875cf800fb4f25077676749a03b`.
This supersedes the pending direct-mode status below; real-session parity is false.

`test_accepted_copy_overwrites_zero_bytes` passes three native controls at
4a22f1..4a2345: all256 bytes, including zeros, replace old symbol+1e6;
adjacent sentinels remain intact. Symbol+11e takes incoming+b8 if nonzero,
otherwise incoming+18, including zero; +17b takes low16 bits of minute index;
+181 becomes1. Prior schedule/change/volume gates are preconditions, not
executed by this test. Do not directly replace311B resolver merging with this
256B copy: these are different objects and lifecycle stages.

## Direct-mode verification retry

104121 failed before tests at workspace rustfmt: the relative-tail integration
test resolver signature needed multiline formatting. No amount algorithm failure
was observed. The signature-only correction was coordinated with terminal27.
Retry110111-18d31111735c6f54 is queued for fmt/example tests/Clippy/release and
the original50-vector comparison output. Await its callback; do not infer success
from the earlier example binary or from conditional Python tests.
Terminal29 metadata manifest SHA4093a616ef95550028c8f6a0ca28d06b8d2589ee63a18110ecb65c17049935bd
was independently checked; its next real-object snapshot task was delivered
with submitted=true. Same-session category/unit/digits/time binding remains open.

## Preopen processing predicate and metadata prerequisites

`test_preopen_interval_predicate` passes six native controls at40c483..40c4ca.
Synthetic09:30 open and allowance5 return true for minute565..569, false at
564/570/571; allowance0 returns false at569. Static preceding branches require
signed category<=20 and incoming cumulative volume==0; the test enters after
those branches and calendar conversion, so it validates only interval handling.
Caller4a1e6e..4a1e72 clears its local incremental-processing flag on true;
this is not an early return or a proven rejection of the final256B copy.

Real metadata binding must also identify symbol+a2 (cross-night branch),
+a4, signed+a7 allowance, +aa calendar-helper argument, signed+e5 last
schedule index, and12B entries at+e6 (signed short start/end at+4/+6 and
accumulated length at+8). Entry bytes+0/+1 supply hour/minute to40c440.
40bf30 writes+122/+126/+12a/+132 using calendar helpers and these schedules;
do not substitute a guessed fixed equity session for missing symbol metadata.
The request to terminal29 to extend its metadata audit timed out; input history
did not show it at the subsequent check. This paragraph preserves the pending
coordination requirement, not an acknowledgement from that owner.

## Schedule and group-state gate evidence

Raw contexts are preserved in
`../../diagnostics/20260908-oem-time-gates-v1/manifest.json`, SHA256
`6362207329fd7e6fa4f28cc5469c546c09bc26da2c655bd892ed4b69761acbc5`.
The manifest pins the executable and hashes four bounded disassemblies.
4a1cb0 calls40c160 first (negative result returns), then4a2700 (negative
result returns), before the change predicate and volume-delta gate.
Static slow-path evidence at4a2882..4a290c shows collection traversal,
symbol+142 equality selection, calls40bcb0 and writes+122/+126/+12a/+132.
Its effect is therefore not necessarily confined to the current symbol;
the meaning of group membership and live reset state remains unverified.

New `test_schedule_interval_clamping` passes11 conditional native vectors
at40c2af..40c426, after calendar/category processing. A synthetic single
570..690 minute schedule with start allowance-1/end allowance20 accepts569
as offset0,690..710 as offset119, and rejects568/711. A1320..120 midnight
schedule gives offset60 at1380 and180 at60, accepts121 as239, rejects141.
This is a schedule-relative return value, not a rewritten public timestamp.
Actual symbol schedules, weekday handling and calendar helpers are not supplied
by these synthetic controls and remain prerequisites for real integration.

## Time transition dispatch boundary

Full native bridge suite rerun:17 tests passed in41.428s using
`/tmp/netzip-5188-native-audit-venv/bin/python -m unittest discover -s scripts -p test_oem_bridge_scalars.py -v`.
This verifies conditional instruction controls, not the pending Rust build or
live callback parity.

`test_time_transition_fast_path_bounds` passed seven conditional native
controls at4a2753: deltas -201/-200/-199/0/199/200/201 reach4a2783
only for [-200,199]; the two outer cases reach4a278a. Each execution must
reach the observed endpoint within its instruction budget. These controls use
old timestamp1000, not a captured session or a u32 day/wrap boundary.
The slow endpoint dispatches further state processing; reaching it does not
prove rejection, expiration, or the final256B commit. Do not implement a
symmetric +/-200 expiration rule from this branch. Full slow-path behavior,
schedule acceptance and session-bound replay remain open.

## Volume rejection side effect

Six native4a1dbb..4a1e28 branch controls pass. Incoming minus old volume is
32-bit wrapping subtraction interpreted signed before the float comparison.
101-100 and100-100 pass;99-100 rejects;0-0xffffffff wraps to+1 and passes;
0x80000000-0 and0xffffffff-0 reject. These inputs are conditional on preceding
guards already passing and do not establish complete record acceptance.
Before testing that delta,4a1def writes incoming amount_f32 minus old amount_f32
to symbol+1b7. Every control, including rejection, writes50.0 for150-100 while
leaving the old256B volume untouched. A rejected copy therefore is not a
transactional no-op on all symbol fields. Replay must distinguish auxiliary
symbol mutations from committing the main256B subrecord; neither is a public
quote authorization. Earlier whole-record/no-side-effect assumptions must not
be used for this path.

## Copy change predicate

Ten native40a9a0 controls pass. The function first rejects unsigned timestamp
regression. Otherwise it returns true for unsigned volume increase, changed
last+18, changed8 bytes at+88 (central volumes), or changed8 bytes at+48
(central prices). Timestamp-only, amount-only and an outer price+38-only
change return false. Volume decrease with changed last returns true here;
4a1cb0 later computes signed wrapping volume difference and has a separate
negative-delta return, so this predicate is not the whole acceptance decision.
At4a1d8e the caller passes current arg3 as this and prior symbol+1e6 as arg;
false returns before the final256B copy. Thus unconditional merge of every
successfully decoded record is not established as equivalent to this path.
Tests execute original memcmp helper and require the function return sentinel.
No default projection change was made from this partial acceptance evidence.

## Entry eligibility

Seven conditional native branch controls pass for49ada7..49adee: timestamp
and reference+12b must be nonzero; last+10 may be zero when either central
book price+68/+6c is nonzero. All three zero skips the record. Negative nonzero
reference/last pass these checks. This only reaches symbol lookup, not a proven
lookup hit, accepting copy, or callback emission. Publish validation remains
separate. The test stops at exactly one observed branch endpoint, failing if
neither endpoint is reached. Runtime integration should retain skip reasons
and avoid treating a failed bridge eligibility check as a decoder bit error.

## Direct Rust implementation pending verification

The example now accepts explicit mode0 for direct amount encoding/readback.
It preserves float32 rounding before integer conversion and the marker/shift
representation, including negative inputs. invalid_conversion=false here means
the tested i64 input domain did not trigger a CPU conversion exception; it does
not mark a quote semantically valid. CLI has no publication path.
The binary comparison script now covers50 cases:32 mode2 scale,6 mode1/category
and12 direct-mode edges. Verification output is planned as
`../../diagnostics/20260908-oem-bridge-scalar-v1/native-rust-matrix-direct-v2.json`.
This is pending evidence, not a claimed pass. Full conversion still needs symbol
selection, eligibility/copy guards and actual captured-state input binding.

## Direct amount branch

Native direct writer40b109..40b128 and getter controls pass for seven
nonnegative boundary values. Float32 amount converts to integer; values above
0x7fffffff use shifted16 encoding with bit31 marker. Readback clears the marker
and multiplies by65536, matching supplement's direct decoding formula.
Conditional scalar CPU branch is explicitly selected, as with earlier helpers.
Additional executed controls: -1 and -12345 pack as0xffffffff and read back
140737488355328.0; -3784729286159 packs as signed-57750384 and reads back
136952758140928.0; i64::MAX rounds through float32 and produces marker-only
0x80000000, whose readback is0. These are native mechanism results, not valid
quote acceptance. Do not reinterpret nonnegative output as proof of correctness.
Rust direct-mode implementation remains next work; mode1/2 are already present.
Terminal29 has been tasked with real symbol category/unit/digits provenance;
management retains bridge example ownership. Message submission confirmed.

## Category selection verified conditionally

Native suite now has eleven passing tests. 55 full-prefix writer controls run
40aeb0 through40b128 with synthetic symbol metadata: categories0/15/86 select
compression mode1; categories1/9/16/26/3/8/18/23 select mode2. Units1/10/100/1000
at symbol+9e produce low scale exponents0/1/2/3; unsupported unit7 falls back0.
Symbol+9c supplies the high three scale bits (tested value2). Both resulting
flags and packed amount agree with the separately verified conditional branch.
This is not the 0104 internal amount_mode field. Mapping captured code metadata
to the actual symbol object's category/unit/digits remains to be established.

Shared decoder ownership update: terminal27 owns the baseline-tail fix,
source2ba2d9f4668109d6d1a79b088f19cf946efed29fb5af6e01398d4cbf85f1b4aa,
focused request103306 pending its verification. Management's bridge gates do
not qualify that shared revision. Native-record helper/tests belong to27;
bridge scalar files remain managed separately.

## Mode2 scale matrix

Native suite: ten tests passed, including all32 combinations of two low scale
bits and three high scale bits. High exponent7 falls back to1, matching the
existing supplement helper. Both packed word and float32 readback agree with
the instruction-ordered formula, including invalid integer conversion.
The Rust example now accepts mode2 with explicit close and scale bits.
`scripts/verify_oem_bridge_mode2.py` compares its built output against native
execution for all32 combinations, matching packed integers and float32 bytes.
This verifier is submitted with the next webClx build; do not infer it passed
from the Python-only formula comparison. Request102954 completed successfully.
Category/scale provenance, copy acceptance and real business parity remain open.

## Pending native-aligned follow-up

Python suite now passes nine tests. Additional mode1 controls demonstrate
CVTTSS2SI invalid conversion (i64 extremes, NaN, infinity) writes i32::MIN
and continues through getter to -21473836.0. The current Rust experiment's
out-of-range rejection is therefore a known implementation difference to fix,
not native equivalence. Keep a separate invalid-conversion flag when preserving
the native result; it must not imply a valid public quote.

Explicit mode2 writer40b01a..40b107 plus actual getter passes four unscaled
controls including zero volume and negative amount. Source close+18 is used:
packed=trunc(((amount_f32/volume_f32)-close_f32)*30+0.5), with float32 rounding
at every operation, then readback=(packed_f32/30+close_f32)*volume_f32.
Low/high scale bits are zero in these controls; nonzero scales and real symbol
selection are not covered. Rust source was preserved during request102729.

## Mode1 compressor/readback

102358 passed four Rust tests, fmt, Clippy and release. Native suite now passes
seven tests. Conditional mode1 writer40af68..40afed followed by accepted-copy
simulation and getter4a79c6..4a79d7 demonstrates amount164124/volume188 becomes
packed -12699 and OEM164125.875; amount100000001/volume12345 becomes100000056.
Volume0 gives OEM amount0 even when incoming amount is12345. Constants read
from the pinned image are offset1000, scale100 and rounding0.5. Arithmetic
rounds at each float32 instruction; final conversion truncates toward zero.
Rust example now accepts an optional explicit amount-mode1 argument after
category, using the same readback formula as netzip-supplement's existing
decoder. Category/mode selection remain caller-supplied; no inference is claimed.
Out-of-range CVTTSS2SI inputs are explicitly rejected in this experiment,
not silently approximated with Rust's saturating float cast. Other amount modes,
full copy guards and real-session replay remain incomplete.

## Latest implementation

Request102028 passed fmt, three example tests, Clippy and release. The next
revision adds optional explicit AMOUNT_I64/DELTA_I64/CATEGORY arguments and
category9 preprocessing to the Rust example; metadata category inference is
not implemented. Intermediate amount uses signed i64-to-f32. Final OEM amount
is explicitly null because compression/getter reconstruction is unfinished.

Native suite now passes six tests, including eight signed amount vectors and
category9 i64/i32 overflow controls. Amount slice49af6a..49af85 executes
helper5847a0 with CPU feature byte5e40c8 bit0x20 explicitly selected, exercising
the original scalar fallback (not AVX-512 and not a captured CPU configuration).
Negative amounts remain negative at this intermediate stage. Category9 add50
and signed delta add/sub50 wrap before signed division, as verified at integer
extremes. Earlier limitations on untested wrapping below are superseded by
these controls. Full conversion/copy acceptance/session parity remain open.

## Verified scope

Pinned binary SHA256:
`de712a8dde6d990e1c586f8afd4194575e35dffa2d0f81245fe29f6f8509bd29`.
Native tests: [test_oem_bridge_scalars.py](../../scripts/test_oem_bridge_scalars.py).
Rust diagnostic: [official_5188_bridge_scalars.rs](../../examples/official_5188_bridge_scalars.rs).

Three Python tests passed: ten scalar vectors and five category9 vectors,
each category vector also checks ten signed book quantities. Every instruction
slice must reach its explicit endpoint; instruction-budget exhaustion fails.
PE instructions and the native signed division helper execute in Unicorn.
Inputs and branch selection are synthetic, not captured-session state.

## Facts

- `49af5e..49af6a` copies only the volume low dword to output+20.
- `49b009..49b04d` keeps UTC day seconds below 25200 unchanged; later
  seconds become the same UTC day's 25200. u32::MAX is before that threshold.
- Category9 `49ae3d..49af01` modifies the source record in place: cumulative
  volume uses signed `(volume + 50) / 100`; signed delta at +24 uses +50
  when positive and -50 otherwise; book quantities use signed `(value+50)/100`.
  Division truncates toward zero. These statements describe tested nonoverflow
  inputs; arithmetic overflow still needs separate native controls.
- Volume 1/49 becomes 0; volume -150 becomes -1. Delta -150 becomes -2.
  Book -151/-150 becomes -1; -100 becomes 0. No blanket positivity floor
  exists in this tested intermediate conversion.

## Remaining work

Update: `101426-18d31111735c6f40` completed successfully at 10:19:17;
its log confirms release completion and status0. Subsequent native execution
adds `4a7995..4a79c6`, including getter40bb10: the volume word is interpreted
as unsigned u32 and then rounded to float32. Seven controls include -1,
-520882088, the sign boundary, low-word truncation and 2^24+1 rounding.
All four Python tests passed. The intermediate signed i32 is therefore NOT
the final OEM scalar interpretation. The Rust diagnostic now emits
`oem_volume_f32`; its new verification request is `102028-18d31111735c6f42`.
No minimum-one floor appears in this getter/conversion slice. Copy acceptance,
category provenance and actual session output still require independent checks.

The Rust example currently covers only noncategory9 volume and time slices.
webClx request `101426-18d31111735c6f40` was submitted for fmt, example tests,
Clippy and release. Its completion is not established by these Python tests.
Do not edit that example while its verification is pending.

Before replacing public projection, trace downstream getter/merge behavior,
prove metadata-to-category selection, cover overflow and amount compression,
then compare full conversion and business fixtures. Existing public minimum-one
and nonpositive-zero rules differ from this intermediate conversion, but the
intermediate difference alone does not prove the final callback rule wrong.
No canonical output or service deployment changed in this step.
