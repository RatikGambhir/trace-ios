# SMS for the login flow: Twilio Messaging API vs. Verify

**Discovery for [RAT-5](https://linear.app/ratik-gambhir/issue/RAT-5/discovery-twilio-messaging-api).**
Prices and carrier limits checked August 2026.

The ticket asks about the **Messaging API** (Programmable Messaging — we generate
a code, we send the text, we check it). [RAT-7](https://linear.app/ratik-gambhir/issue/RAT-7/implement-sms-otp-login-with-twilio-verify)
already assumes **Verify**. This document is the justification that was missing
between them, and it lands on Verify — for one reason that has nothing to do with
convenience.

## The decisive reason: A2P 10DLC

Sending application-to-person SMS from a US 10-digit long code requires
registering with The Campaign Registry: a **brand**, then a **campaign**. This is
carrier policy, not a Twilio upsell. Unregistered traffic from a Twilio long code
gets extra carrier fees and heavier filtering.

Trace has no EIN, so we would register as a **Sole Proprietor brand**, and that
tier is capped hard:

| | Sole Proprietor 10DLC |
| --- | --- |
| Brand registration | $4 one-time |
| Campaign vetting | $15 one-time |
| Campaign | $2/month |
| **T-Mobile** | **1,000 message segments per day** |
| **AT&T** | **15 messages per minute** |
| Trust Score | none — throughput is fixed, not earned |

A thousand segments a day across T-Mobile is fine for a beta and is a wall the
day a launch works. Lifting it means a real business entity and a Standard brand,
or T-Mobile's Special Business Review. Hitting it returns error `30023` (daily
cap reached), with `30026` as the 70%-consumed warning — i.e. **new users stop
being able to log in**, which is the worst possible failure to discover in
production.

Twilio's own documentation resolves this directly:

> If you're only using 10DLC numbers to send user verification text messages, you
> can use Twilio Verify rather than registering for A2P 10DLC.

Verify sends over Twilio's own registered sender infrastructure. **No brand, no
campaign, no sole-proprietor throughput ceiling, no registration work before the
first login works.** That alone settles the ticket. Everything below is
confirmation.

## What else Verify covers that we would otherwise write

With the Messaging API, "send an SMS" is the easy 10% of an OTP flow. The rest is
ours:

| Concern | Messaging API | Verify |
| --- | --- | --- |
| Code generation and storage | We build it — a table, a hash, a constant-time compare | Held by Twilio |
| Expiry | We build it | 10 minutes (2 min–24 h on request) |
| Wrong-code attempt limit | We build it | 5 checks, then the verification must expire |
| Resend flooding | We build it | 5 sends per recipient per 10 minutes |
| Code length | We pick | 4–10 digits, default 6, set per service |
| **SMS pumping / toll fraud** | **Ours to detect and eat** | **Fraud Guard, included in the price** |
| Extra channels later | New integration per channel | Same two calls for voice, WhatsApp, email, TOTP, push, passkeys, Silent Network Auth |

SMS pumping is the one to weigh properly. It is the attack where someone farms an
unprotected OTP endpoint to drive revenue-share traffic to premium numbers; the
bill lands on us and arrives before the alert does. Verify's Fraud Guard is
included at no extra cost and blocks it at the network level, with selectable
aggressiveness (Basic <0.1% false positives, Standard <1%, Max <2%). Building the
equivalent on raw Messaging means our own velocity heuristics and destination
controls, tuned by us, against an adversary who tests continuously.

The channel row also matters more than it looks. Verify's Silent Network
Authentication verifies possession of a number through the carrier data
connection with no SMS at all, and bills nothing when the carrier flow can't
complete. If phone login ever gets expensive or the OTP screen ever becomes a
funnel problem, that is a config change on an integration we already have, not a
new project.

## Cost

**Messaging API** — $0.0083 per US long-code segment, plus carrier surcharges,
plus $1.15/month for the number, plus the 10DLC fees above.

**Verify** — **$0.05 per successful verification** plus the channel fee
(~$0.0083 for US SMS). The $0.05 lands only when a check is *approved*; an
abandoned verification costs the SMS attempt alone. So the real unit is per
completed login, not per message sent.

At 1.2 SMS per completed login (some resends, some abandonment):

| Completed logins / month | Messaging API | Verify | Delta |
| --- | --- | --- | --- |
| 500 | ~$8 | ~$30 | $22 |
| 1,500 | ~$18 | ~$90 | $72 |
| 15,000 | ~$152 † | ~$899 | $747 |

† and only if the 1,000-segment T-Mobile cap has been lifted by then — at 15,000
logins a month it has not been, on a Sole Proprietor brand.

Verify costs roughly 6× per message. **At the volumes Trace will see for a long
while, 6× of a small number is $20–70 a month** — less than an hour of the
engineering time it would take to build the OTP state machine, never mind
maintain it or absorb one pumping incident. The multiplier only becomes real
money in the bottom row, and that row is a good problem.

## Recommendation

**Use Twilio Verify for phone login. Do not build on the Messaging API.**

Revisit when **monthly verifications pass ~5,000** (≈$290/mo on Verify against
≈$50 on raw messaging). At that point three levers open up, in increasing order
of effort: put Silent Network Auth in front of SMS so most logins send no
message; move to a Standard 10DLC brand with our own OTP logic; or price a second
SMS vendor. Keeping the integration behind a small trait (below) is what makes
that a swap rather than a rewrite.

This is the same conclusion [`auth.md`](auth.md) reaches from the other
direction: the SMS channel — not the identity store — is where Trace's auth money
goes, and every user who signs in with Apple instead sends zero messages.

## Integrating it into `trace-server`

**There is no HTTP client in the service today.** `Cargo.toml` has axum, sqlx,
tokio — nothing outbound. Twilio publishes no official Rust SDK; its REST API is
form-encoded POSTs with HTTP Basic auth, so this is `reqwest` plus two functions,
not a vendor SDK.

Add `reqwest` with `default-features = false` and `rustls-tls` + `json`. This is
not incidental: sqlx is already on `runtime-tokio-rustls`, and the runtime image
is `debian:bookworm-slim` with `ca-certificates` and nothing else. Pulling in
reqwest's default OpenSSL backend would add a system dependency the Dockerfile
does not install, and the build would fail — or worse, succeed and fail at
runtime.

**Credentials.** Authenticate with an **API Key SID + Secret** (`SK…`), not the
Account SID + Auth Token. Both work; the API Key can be revoked and rotated on
its own, while a leaked Auth Token is the whole account. Three Railway variables:
`TWILIO_ACCOUNT_SID`, `TWILIO_API_KEY_SID`, `TWILIO_API_KEY_SECRET`, plus
`TWILIO_VERIFY_SERVICE_SID`. They belong in `AppState` alongside the pool —
`core/state.rs` holds only `db` today.

**Two calls**, both under `https://verify.twilio.com/v2/Services/{ServiceSid}`:

- `POST /Verifications` — `To` (E.164) and `Channel=sms`
- `POST /VerificationCheck` — `To` and `Code`, returning `status: approved` or
  `pending`

**Shape it to the existing layers.** A `services/verification.rs` behind a small
trait — `send_code(&E164)` / `check_code(&E164, &str)` — with the Twilio
implementation in `core/` beside `crypto.rs`, matching how the codebase already
separates primitives from user logic. Handlers never name Twilio. Tests get a
fake implementation, so `cargo test` still needs no network and no database, as
the router tests already assume.

**Error mapping** into the existing `ApiError`:

| Twilio | Ours |
| --- | --- |
| `60200` invalid parameter (malformed number) | `422` validation |
| `60202` max check attempts reached | `429`-ish — needs a new variant |
| `60203` max send attempts reached | same |
| `20429` account rate limit | `503` |
| transport failure / timeout | `503`, logged in full |

`ApiError` has no `Unauthorized` or `TooManyRequests` variant yet; RAT-7 adds
both, and RAT-10's auth work needs the first anyway.

**Normalize to E.164 before anything touches it.** `PhoneLogInSheet` hardcodes
`+1` and accepts ≥7 digits, so `(555) 010-0000` and `5550100000` both arrive.
Normalize at the request-validation boundary — the `Validated*` twin pattern
`models/requests/` already uses — or the same human ends up with two accounts.

Estimate for RAT-7: **3–5 working days**, most of it in tests and the
rate-limiting around the endpoints rather than the Twilio calls themselves.

## Operational settings to get right on day one

- **Geo Permissions: US and Canada only.** The account default allows sending
  worldwide. Since the app only offers `+1`, restricting destinations removes the
  entire category of fraud where an attacker points our OTP endpoint at premium
  international routes. One console setting; do it before the first live send.
- **Fraud Guard on Standard** to start (<1% false positives). Basic if real users
  report being blocked.
- **Our own rate limit as well as Verify's.** Verify caps per recipient; it does
  not know that one IP just requested codes for 400 different numbers. Limit per
  IP at our edge too.
- **Never log the code, and treat the phone number as PII** — don't put it in
  `tracing` fields. `core/crypto.rs` already sets the precedent that secrets stay
  out of `Debug`.
- **Trial accounts can only send to numbers verified in the console.** Expect
  that to bite during local development, and upgrade before any TestFlight build
  where other people log in.
- **Verify holds the code, so we store no OTP state** — only an attempt-rate
  table if we want per-IP limiting in Postgres rather than in memory.

## Open questions

1. **US and Canada only, or international from launch?** The `+1` hardcode says
   US. International SMS is several times more expensive per message and carries
   most of the fraud risk; the answer sets Geo Permissions and the country picker
   together.
2. **Is there a business entity?** A Standard 10DLC brand needs an EIN. Verify
   sidesteps it for login, but the moment Trace wants to send *any* non-OTP
   message — a trip summary, a re-engagement nudge — that question comes back and
   brings 10DLC registration with it.
3. **What is the resend UX?** Twilio's send limit is 5 per number per 10 minutes;
   the app should show its own cooldown rather than let a user hammer a button
   into a `60203`.
4. **Do we want Silent Network Auth in the first version?** It removes the SMS
   entirely for supported carriers. It is a Verify channel we would already be
   integrated with, but it adds a fallback path to design and test.

## Sources

- [Twilio A2P 10DLC](https://www.twilio.com/en-us/phone-numbers/a2p-10dlc) · [Programmable Messaging and A2P 10DLC](https://www.twilio.com/docs/messaging/compliance/a2p-10dlc) · [Sole Proprietor registration](https://www.twilio.com/docs/messaging/compliance/a2p-10dlc/direct-sole-proprietor-registration-overview) · [Sole Proprietor brands FAQ](https://support.twilio.com/hc/en-us/articles/9550596959643-A2P-10DLC-Sole-Proprietor-Brands-FAQ)
- [T-Mobile daily message limits](https://support.twilio.com/hc/en-us/articles/1260804800549-T-Mobile-daily-message-limits-for-long-code-messaging-with-A2P-10DLC) · [Throughput and Trust Scores](https://support.twilio.com/hc/en-us/articles/1260803225669-Message-throughput-MPS-and-Trust-Scores-for-A2P-10DLC-in-the-US) · [Error 30023](https://www.twilio.com/docs/api/errors/30023) · [Error 30026](https://www.twilio.com/docs/api/errors/30026)
- [Verify docs](https://www.twilio.com/docs/verify) · [Rate limits and timeouts](https://www.twilio.com/docs/verify/api/rate-limits-and-timeouts) · [Programmable rate limits](https://www.twilio.com/docs/verify/api/programmable-rate-limits) · [Developer best practices](https://www.twilio.com/docs/verify/developer-best-practices) · [Error 60202](https://www.twilio.com/docs/api/errors/60202) · [Error 60203](https://www.twilio.com/docs/api/errors/60203)
- [Preventing toll fraud in Verify](https://www.twilio.com/docs/verify/preventing-toll-fraud) · [Anti-fraud developer's guide](https://www.twilio.com/docs/usage/anti-fraud-developer-guide) · [SMS pumping protection options](https://www.twilio.com/en-us/blog/developers/best-practices/sms-pumping-protection-options)
- [Messaging pricing](https://www.twilio.com/en-us/pricing/messaging) · [US SMS pricing](https://www.twilio.com/en-us/sms/pricing/us) · [Verify pricing and alternatives, 2026](https://www.authgear.com/post/twilio-verify-pricing-and-alternatives/)
- [API keys overview](https://www.twilio.com/docs/iam/api-keys) · [Generating a Rust client for Twilio's API](https://www.twilio.com/docs/openapi/generating-a-rust-client-for-twilios-api)
