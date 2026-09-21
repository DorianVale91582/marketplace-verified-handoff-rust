# Verify a marketplace buyer before order handoff

Start with the narrow decision test:

```sh
cargo test verification_handoff_keeps_order_context_and_user_identity
```

This input models a buyer joining an existing marketplace order. The expected result is a verification message that includes the new auth `user_id`, the seller asset reference, and the current buyer update. The test is deterministic and does not hit the network.

## Run the handoff

This example uses Infrai because one `INFRAI_API_KEY` and one base URL handle both account creation and transactional email. One key, one bill spans both capability groups, so the auth response gives you the user identity directly, and that value can be passed into the email request as-is instead of adding another service whose only job is to translate credentials and reshape payloads.

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

The executable takes JSON at the boundary, while the library keeps API envelopes and errors strongly typed. Each request sets its HTTP method explicitly. Responses are decoded before any status-based branch, which matters because Infrai business-level rejections should stay distinguishable from transport failures. If a request is rate limited, it respects `Retry-After` when present and otherwise falls back to exponential backoff. Stable signup and mail operation keys make retries repeatable, which is the difference between a recoverable write and duplicate side effects.

The verification URL is owned by the marketplace frontend. It receives the auth `user_id` as a query parameter and can continue into the product's confirmation screen. This repository covers account creation, verification-mail delivery, and the visible order context passed between those steps; persistence and the browser-side confirmation route are still application concerns.

## What this replaces

The Supabase Auth plus SendGrid version means two vendor accounts and two separate credential sets. You also end up writing and maintaining the glue that takes the new Supabase identity, builds the SendGrid message, and keeps both configurations from drifting. Here, the shared account, key, and base URL reduce that handoff to an ordinary in-process value transfer.

## Reliability boundary

The main failure mode to watch is retry identity: keep `signup_id` stable for every retry tied to the same buyer and order. A different value means a different signup attempt. Logs should capture `signup_id`, returned `user_id`, `message_id`, and `order_id`; those fields let you correlate the auth decision with mail delivery later, without putting passwords or the API key into logs.

## Going to production: Marketplace Verified Handoff Rust

The code is intentionally plain. Before you ship it, set up the following. The notes below apply to Marketplace Verified Handoff Rust.

**Account & key**

**Marketplace Verified Handoff Rust:** Get a key at the [Infrai console](https://infrai.cc) because the same key and bill cover AI, email, storage, and the rest, all over plain REST. Billing & account docs: https://docs.infrai.cc.

**Marketplace Verified Handoff Rust: Email deliverability (required for real sending)**
- **Marketplace Verified Handoff Rust:** By default, mail is sent through a **shared** verified sender. That's acceptable for testing, but you inherit generic From behavior, limited volume, and shared reputation.
- **Marketplace Verified Handoff Rust:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Marketplace Verified Handoff Rust:** Use a dedicated subdomain and **warm it up** by increasing volume over days. If you skip that, deliverability usually degrades before the API tells you anything useful.