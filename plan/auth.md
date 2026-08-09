# Authentication: identity provider vs. custom

**Discovery for [RAT-10](https://linear.app/ratik-gambhir/issue/RAT-10/evaluate-identity-provider-vs-custom-auth).**
Prices checked August 2026; every one of them is a published list price that can
change, so re-check before signing anything.

## The question, sharpened

"Identity provider vs. custom auth" is really three questions, and they have
different answers:

1. **Who verifies the user is who they say they are?** — Apple, for Sign in with
   Apple. A phone network, for an SMS code. Nobody is proposing we build that.
2. **Who stores the identity and issues our session tokens?** — this is the
   actual decision. Us, or a vendor.
3. **Who pays for the SMS?** — we do, in every option. SMS is a per-message cost
   that no auth vendor absorbs.

Question 2 is what this document decides. Question 3 turns out to dominate the
cost comparison, and it is nearly independent of question 2.

## What the code already commits us to

Worth writing down, because it narrows the field before any vendor is compared.

**The app's UI has already picked the sign-in methods.** `WelcomeView` and
`LogInView` offer exactly two: *Log in with Apple ID* and *Log in with phone
number*. The phone sheet (`PhoneLogInSheet` in `LogInView.swift:82`) hardcodes
`+1`, and gates its Continue on ≥7 digits. `CreateAccountView` collects name,
email, and password — but nothing navigates to it; it is reachable only via the
`-showRegister` debug launch argument (`AuthenticationFlowView.swift:15`). So
today's design is **Apple + phone OTP, passwordless**, with an email/password
screen built but parked.

**The server has signup-shaped code and no authentication.** Concretely:

- `users` (`db/migrations/0001_create_users.sql`) has `first_name`, `last_name`,
  `role`, `api_key`, `password_hash NOT NULL`. There is **no email, no phone, no
  external-identity column**, and the only lookup in `repositories/users.rs` is
  by UUID primary key. There is no column you could log in *with*.
- `POST /api/v1/users` mints an API key and returns it once. There is no
  `/login`, no `/session`, no token endpoint.
- Nothing is authenticated. `POST /api/v1/journeys` takes `user_id` from the
  request body, so anyone who can reach the public URL can write into any
  account — already flagged in `src/trace-server/README.md`.
- `ApiError` (`src/core/error.rs`) has no `Unauthorized`/`Forbidden` variant, and
  `AppState` (`src/core/state.rs`) holds only the pool. There is no middleware
  layer in `router.rs` beyond trace/timeout/CORS.

The `password_hash NOT NULL` column is the sharpest detail: it is the one piece
of auth already built, and it is the wrong shape for the passwordless flows the
UI actually offers. **Whatever we choose, a `users` migration and a new
middleware layer are required work.** No option is drop-in.

**Related tickets already in flight.** [RAT-5](https://linear.app/ratik-gambhir/issue/RAT-5/discovery-twilio-messaging-api)
(Twilio discovery) and [RAT-7](https://linear.app/ratik-gambhir/issue/RAT-7/implement-sms-otp-login-with-twilio-verify)
(implement SMS OTP with Twilio Verify) presuppose that we own the OTP flow. This
document should either confirm that or contradict it — see
[Where the money actually goes](#where-the-money-actually-goes).

## What an identity provider would actually be selling us

Auth vendors are priced for the problem of *many login methods, and the support
burden behind them*: password reset, email deliverability, verification links,
a dozen social providers, MFA enrolment, SSO/SAML, an admin console, breach
detection, SCIM.

Against the app as designed, that list is almost entirely unused:

| What an IdP gives you | Do we need it? |
| --- | --- |
| Hosted password reset + email delivery | No — no password flow in the UI |
| Ten social providers | No — Apple only |
| SSO / SAML / SCIM | No — consumer app, no enterprise buyers |
| MFA enrolment and recovery | Not yet — OTP *is* the factor here |
| Admin console for user support | Marginal — `psql` covers it at our size |
| Compliance posture (SOC 2 inheritance) | Not yet |
| Apple ID token verification | Yes — but it is ~100 lines (see below) |
| SMS delivery for OTP | **No — every vendor bills this through anyway** |

The two flows we do need are the two an IdP helps with least.

**Sign in with Apple, native, is small.** The iOS app already has to use
`ASAuthorizationAppleIDProvider` to draw the button. That hands the app an
`identity_token`: a JWT. Server-side verification is fetch Apple's JWKS from
`https://appleid.apple.com/auth/keys`, verify the signature, check `iss ==
https://appleid.apple.com`, `aud == <bundle id>`, `exp`, and the nonce we sent.
That is `jsonwebtoken` plus a cached key set — a day of work, not a quarter. The
web OAuth dance (Services ID, `client_secret` JWT, redirect URIs) that makes
Sign in with Apple look heavy **does not apply to a native app** for plain
sign-in. It applies only to token revocation, which we need for account deletion
— see [Obligations that hold either way](#obligations-that-hold-either-way).

**Phone OTP through an IdP is still SMS you pay for.** Firebase, Supabase,
Clerk, Auth0, and Cognito all either bill per SMS or make you attach your own
Twilio account. Buying an IdP to get phone login is buying a wrapper around a
bill you pay regardless.

## Cost

### Platform pricing, August 2026

| Option | Free tier | Price above it | Base fee |
| --- | --- | --- | --- |
| **Custom in `trace-server`** | n/a — runs on the Railway service we already pay for | $0 marginal | $0 |
| **Firebase Auth / Google Identity Platform** | 50,000 MAU (email/social/anon) | from $0.0055/MAU | $0 (Blaze billing account required for phone auth) |
| **Supabase Auth** | 50,000 MAU | $0.00325/MAU | $0 free / $25 per project on Pro (→100k MAU) |
| **Clerk** | 50,000 MRU (raised from 10,000 on 5 Feb 2026) | $0.02/MAU | $0 free / $25 Pro |
| **Auth0** | 25,000 MAU | ~$0.07/MAU overage | $0 free / B2C Essentials from $35/mo (500 MAU) |
| **AWS Cognito** | 10,000 MAU (cut from 50,000 in Dec 2024) | $0.015/MAU (Essentials) | $0 |

Clerk meters **MRU** — a user counts only if they come back ≥24h after signing
up — which is a genuinely narrower unit than everyone else's MAU. Auth0 has the
sharpest cliff: free to 25,000, then a paid plan whose overage rate is 3–20×
everyone else's.

**At the scale Trace is at — pre-launch, low thousands of users — every option
in that table costs $0/month.** MAU pricing is not a tiebreaker today. It is
insurance against a future where it becomes one, and the premium is paid in
lock-in rather than dollars.

### Where the money actually goes

SMS is the only line item that is non-zero from day one.

| Channel | Per US verification |
| --- | --- |
| Twilio Verify | $0.05 verification fee **+** ~$0.0083 SMS ≈ **$0.058** |
| Firebase Phone Auth | ~$0.01 US/Canada (up to ~$0.46 in the priciest regions) |
| Amazon SNS (roll your own OTP) | ~$0.00645 US |
| Sign in with Apple | **$0** |

Two monthly scenarios, SMS included:

| | 2,000 MAU / 1,500 verifications | 25,000 MAU / 15,000 verifications |
| --- | --- | --- |
| Custom + Twilio Verify | **$87** | **$870** |
| Custom + SNS (own OTP logic) | $10 | $97 |
| Firebase Auth (phone + Apple) | $15 | $150 |
| Supabase Auth + own Twilio | $87 | $895 ($25 Pro + SMS) |
| Clerk + SMS passthrough | $25–87 | $175–895 |
| Auth0 (free → Essentials) | $87 | ~$1,750 + SMS at the 25k cliff |
| Cognito + SNS | $10 | $322 |

Read those two columns together and the finding is not "IdP or custom." It is:

> **The Twilio Verify per-verification fee is a larger cost lever than the entire
> identity-provider decision.** At 15,000 verifications a month, Verify's $0.05
> fee is $750 — more than any platform bill in the table except Auth0's cliff.

And the cheapest lever of all is free: **every user who signs in with Apple sends
zero SMS.** Making Apple the prominent path (which `WelcomeView` already does —
phone is behind a second tap) is worth more than any vendor choice here.

## Recommendation

**Build authentication into `trace-server`. Sign in with Apple as the primary
identity; phone OTP as the secondary, behind a swappable provider interface.**

Reasons, in the order they matter:

1. **The two flows the UI wants are the two an IdP adds least to.** Apple is JWT
   verification against a public key set. OTP is an SMS bill plus a rate limiter.
2. **Marginal platform cost is $0 and stays $0.** We are already paying for the
   Railway service and Postgres. The `users` table exists. No new vendor account,
   no new billing relationship, no second dashboard.
3. **One source of truth for identity.** Every IdP option means a vendor's user
   record *plus* our `users` row, joined on an external ID, kept in sync on
   deletion, and reconciled when they disagree. That is real ongoing complexity
   that the "vendor is simpler" framing hides.
4. **The server is already shaped for it.** Layered handlers/services/repositories,
   Argon2 wired up (`core/crypto.rs`), a SQL builder, `ApiError` ready for one
   more variant, tests beside the code. Auth is a new vertical slice through
   scaffolding that already exists, not new scaffolding.
5. **No migration debt.** Every option needs the `users` migration and the auth
   middleware anyway. Choosing custom means we write that once, for ourselves.

**What we are accepting by choosing this:** we own the security of session
tokens, OTP rate limiting, and account linking. That is a real ongoing
responsibility, listed concretely in [Scope](#scope-if-we-build-it) below — it is
the honest cost of the $0.

### Where I would change this answer

Written as triggers, so this is re-openable on evidence rather than vibes:

- **Email/password becomes a real flow** (i.e. `CreateAccountView` gets wired up
  with reset + verification). Transactional email deliverability and reset-token
  flows are exactly what IdPs are good at, and where custom auth stops being
  cheap. → Reconsider Firebase or Supabase Auth.
- **A third and fourth social provider** (Google, Facebook). Each is another
  OAuth integration to maintain; at three-plus, a vendor wins.
- **Enterprise or B2B buyers** wanting SSO/SAML, or a SOC 2 questionnaire we
  cannot answer. → Vendor, immediately. Don't build SAML.
- **SMS volume passes ~5,000/month.** Not an IdP trigger — a *provider* trigger.
  See below.

### The separable decision: who sends the OTP

RAT-5 and RAT-7 have already pointed at **Twilio Verify**, and for launch that is
the right call, for a reason that is not price: Verify includes **Fraud Guard**,
which blocks SMS pumping (the attack where someone farms your OTP endpoint to
generate revenue-share traffic to premium numbers) at no extra cost. Rolling our
own OTP on raw SNS at $0.00645/message means owning that risk ourselves, and an
unprotected OTP endpoint can lose more in a weekend than Verify costs in a year.

So: **ship on Twilio Verify, but put it behind a trait** — `send_code` /
`check_code`, one implementation, service-level so the handler never names the
vendor. Then the 6× price delta is a swap, not a rewrite, if volume ever makes it
matter. Concretely, revisit when monthly verifications pass ~5,000 (≈$290/mo on
Verify, ≈$50 on a raw-SMS provider with our own fraud controls).

## Scope if we build it

Rough shape, so "custom auth" is not a hand-wave. Estimate: **8–12 working days**
to a complete, tested flow, most of it in the server.

**Migration `0007_add_auth.sql`**

- `users`: add `phone` (E.164, unique, nullable), `email` (unique, nullable),
  `apple_user_id` (unique, nullable — Apple's stable `sub`). Make
  `password_hash` **nullable** — passwordless users have none.
- A `CHECK` that at least one identity column is non-null.
- `refresh_tokens`: `id`, `user_id`, `token_hash`, `issued_at`, `expires_at`,
  `revoked_at`, `replaced_by`, plus device metadata. Store a hash, never the
  token.
- `phone_verifications` only if we keep OTP state ourselves; Twilio Verify holds
  the code, so this is likely just an attempt-rate table keyed by phone + IP.

**Server**

- `POST /api/v1/auth/apple` — verify the identity token against Apple's cached
  JWKS, upsert the user, issue a token pair.
- `POST /api/v1/auth/phone/start` and `/check` — Verify start/check, then upsert
  and issue.
- `POST /api/v1/auth/refresh` — rotate the refresh token, detect reuse (a
  presented-but-already-rotated token means theft: revoke the whole family).
- `POST /api/v1/auth/logout`, `DELETE /api/v1/users/me`.
- An extractor — `AuthenticatedUser` — implementing `FromRequestParts`, so a
  handler that needs a principal takes one as an argument and cannot forget to
  check. Add `ApiError::Unauthorized` (401) and `Forbidden` (403).
- **Then remove `user_id` from the journey request body** and take it from the
  principal. This is the change that closes the hole the README already names.
- Access tokens: JWT, HS256 off a secret in Railway config, ~15 min. Refresh:
  opaque random (the `core/crypto.rs` generator already does 256 bits from the
  OS CSPRNG), ~60 days, rotating.

**App**

- Keychain storage for the refresh token, an API client that attaches the access
  token and retries once through refresh on a 401, and wiring the existing
  `AppState.completeAuthentication()` to a real result instead of a closure.

**The security work we are signing up for** — the honest list: refresh-token
rotation with reuse detection; OTP attempt limits and lockout per phone and per
IP; E.164 normalization before lookup (or `+1 555 0100` and `5550100` become two
accounts); account linking when the same human arrives by Apple and then by
phone; deletion that also revokes at Apple; JWT signing-key rotation; and keeping
tokens and codes out of logs.

## Obligations that hold either way

Not conditional on the recommendation — true for any option:

- **App Store Review Guideline 4.8.** If the app offers a third-party or social
  login for the primary account, it must also offer a login that limits
  collection to name and email, lets the user keep the email private, and does
  not collect in-app interactions for ads without consent. Sign in with Apple
  satisfies it outright. Today's design (Apple + phone) is compliant; adding
  Google or Facebook later does not change that, as long as Apple stays.
- **Account deletion must revoke at Apple** (Apple's TN3194). That revocation
  call is the one place a native app needs the `client_secret` JWT: ES256, signed
  with the `.p8` key, `iss` = Team ID, `aud` = `https://appleid.apple.com`, `sub`
  = client ID, and an `exp` **capped at six months**. Generate it in-process at
  call time rather than pasting a long-lived secret into config — then there is
  no rotation reminder to miss, and the `.p8` is the only secret stored.
- **Server-to-server notifications.** Apple requires an endpoint for developers
  based in the Republic of Korea registering a Services ID from 1 Jan 2026. Not
  our jurisdiction, and native sign-in needs no Services ID — but the endpoint is
  how Apple tells you about email-forwarding changes and Apple Account deletions,
  which is useful regardless. Cheap to add later; noted, not scheduled.
- **Phone numbers are PII** with a different retention profile from a UUID. Store
  E.164, don't log it, and decide retention on account deletion.

## Open questions

1. **Is email/password in or out?** `CreateAccountView` exists but is unreachable.
   If it is out, say so and delete it — the parked screen is what makes this
   decision look closer than it is. If it is in, that changes the recommendation's
   weighting materially (see the triggers above).
2. **International phone numbers?** The sheet hardcodes `+1`. Non-US SMS is
   dramatically more expensive per message and carries most of the fraud risk.
3. **Does an API key survive?** `users.api_key` exists, is returned once, and
   nothing consumes it. Is it a future machine-to-machine credential, or dead
   weight to drop in the same migration?
4. **What happens to `role`?** It defaults to `user` and nothing reads it. Auth is
   the natural moment to either give it meaning or remove it.

## Sources

- [Clerk pricing](https://clerk.com/pricing) · [free-plan change to 50k MRU, Feb 2026](https://saasprices.net/blog/clerk-free-plan-changes)
- [Firebase Authentication pricing, 2026](https://blog.logto.io/firebase-authentication-pricing) · [Firebase phone auth](https://firebase.google.com/docs/auth/flutter/phone-auth)
- [Supabase pricing, 2026](https://uibakery.io/blog/supabase-pricing)
- [Auth0 pricing explained, 2026](https://idsync.com/guides/auth0-pricing)
- [Amazon Cognito pricing](https://aws.amazon.com/cognito/pricing/)
- [Twilio Verify pricing and alternatives, 2026](https://www.authgear.com/post/twilio-verify-pricing-and-alternatives/) · [Preventing fraud in Verify](https://www.twilio.com/docs/verify/preventing-toll-fraud)
- [Apple: new requirement for apps using Sign in with Apple for account creation](https://developer.apple.com/news/?id=j9zukcr6) · [TN3194: handling account deletions and revoking tokens](https://developer.apple.com/documentation/technotes/tn3194-handling-account-deletions-and-revoking-tokens-for-sign-in-with-apple)
- [App Store Review Guidelines 4.8, Login Services](https://developer.apple.com/app-store/review/guidelines/#login-services)
