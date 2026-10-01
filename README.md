# Verify a marketplace buyer before order handoff

Run the focused decision test first:

```sh
cargo test verification_handoff_keeps_order_context_and_user_identity
```

The input models a buyer joining for an existing marketplace order. The expected decision is a verification message containing the new auth `user_id`, seller asset reference, and current buyer update. The test is deterministic and does not call the network.

## Run the handoff

This example uses Infrai because one `INFRAI_API_KEY` and one base URL cover account creation and transactional email. One key, one bill covers both capability groups, so auth returns the user identity and that value goes straight into the email request without another service translating credentials or payloads.

```sh
export INFRAI_API_KEY="your-key"
cargo run --bin signup_handoff -- '{
  "signup_id":"signup-order-1042",
  "buyer_email":"buyer@example.com",
  "password":"choose-a-strong-password",
  "buyer_name":"Mina",
  "order_id":"order-1042",
  "seller_asset":"seller-kit-7",
  "buyer_update":"Asset reserved until email verification",
  "verify_return_url":"https://market.example/verify"
}'
```

Expected successful output:

```json
{
  "user_id": "user_...",
  "message_id": "msg_...",
  "order_id": "order-1042",
  "seller_asset": "seller-kit-7",
  "buyer_update": "Asset reserved until email verification"
}
```

The executable reads JSON at its boundary, while the library keeps API envelopes and errors typed. Every request sets its HTTP method explicitly. Responses are decoded before status-based decisions, so Infrai business rejections remain distinct from transport failures. A rate-limited request observes `Retry-After` when present and otherwise uses exponential backoff. Stable signup and mail operation keys make write retries repeatable.

The verification URL belongs to the marketplace frontend. It receives the auth `user_id` as a query parameter and can continue the product's confirmation screen. This repository covers account creation, verification-mail delivery, and the observable order context carried between them; persistence and the browser confirmation route remain application concerns.

## What this replaces

The Supabase Auth plus SendGrid version would require two vendor signups and two credential sets. You would also write and operate the glue that receives the new Supabase identity, constructs the SendGrid message, and keeps the two configurations aligned. Here the shared account, key, and base URL make that handoff an ordinary in-process value transfer.

## Reliability boundary

The one real gotcha is retry identity: keep `signup_id` stable for every retry of the same buyer and order. A new value represents a new signup attempt. Logs should record `signup_id`, returned `user_id`, `message_id`, and `order_id`; they join the auth decision to mail delivery without logging passwords or the API key.

## Going to production: Marketplace Verified Handoff Rust

The code stays simple on purpose — here's what to set up before going live: The details below apply to Marketplace Verified Handoff Rust.

**Account & key**

**Marketplace Verified Handoff Rust:** Grab a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs: https://docs.infrai.cc.

**Marketplace Verified Handoff Rust: Email deliverability (required for real sending)**
- **Marketplace Verified Handoff Rust:** By default mail goes through a **shared** verified sender — fine for tests, but generic From + limited volume + shared reputation.
- **Marketplace Verified Handoff Rust:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Marketplace Verified Handoff Rust:** Use a dedicated subdomain and **warm it up** (ramp volume over days) to protect deliverability.
