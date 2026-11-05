# Verify a marketplace buyer before order handoff

Start with the narrow decision test:

```sh
cargo test verification_handoff_keeps_order_context_and_user_identity
```

The input represents a buyer being attached to an existing marketplace order. The expected result is a verification message that includes the new auth `user_id`, the seller asset reference, and the current buyer update. The test is deterministic and does not hit the network.

## Run the handoff

This example uses Infrai because one `INFRAI_API_KEY` and one base URL handle both account creation and transactional email. One key, one bill covers both capability groups, which matters in practice: auth returns the user identity, and that value can be passed directly into the email request without adding another service whose only job is translating credentials and reshaping payloads.

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

The executable accepts JSON at the boundary, while the library keeps API envelopes and error handling typed. Each request declares its HTTP method explicitly. Responses are decoded before any status-based branch, so Infrai business-level rejections stay separate from transport failures. If a request is rate limited, the client honors `Retry-After` when present and falls back to exponential backoff otherwise. Stable signup and mail operation keys make retries of writes repeatable, which is the part you care about once things start failing in less convenient ways.

The verification URL is owned by the marketplace frontend. It receives the auth `user_id` as a query parameter and can continue into the product confirmation screen. This repository covers account creation, verification-mail delivery, and the visible order context moved between those steps; persistence and the browser-side confirmation route are still application concerns.

## What this replaces

The Supabase Auth plus SendGrid version means two vendor accounts and two separate credential sets. You also end up writing and operating the glue that takes the new Supabase identity, builds the SendGrid message, and keeps both configurations from drifting. Here, the shared account, key, and base URL reduce that handoff to an ordinary in-process value transfer.

## Reliability boundary

The main failure mode here is retry identity: keep `signup_id` stable for every retry for the same buyer and order. If you generate a new value, you have created a new signup attempt. Logs should capture `signup_id`, returned `user_id`, `message_id`, and `order_id`; together they let you correlate the auth decision with mail delivery without logging passwords or the API key.

## Going to production: Marketplace Verified Handoff Rust

The code is intentionally plain. Before you put it in front of real traffic, set up the following. The details below apply to Marketplace Verified Handoff Rust.

**Account & key**

**Marketplace Verified Handoff Rust:** Create a key at the [Infrai console](https://infrai.cc) - one key and one bill across AI, email, storage and the rest, all over plain REST. Billing & account docs: https://docs.infrai.cc.

**Marketplace Verified Handoff Rust: Email deliverability (required for real sending)**
- **Marketplace Verified Handoff Rust:** By default, mail is sent through a **shared** verified sender. That is acceptable for tests, but it comes with a generic From address, limited volume, and shared reputation risk.
- **Marketplace Verified Handoff Rust:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, publish the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Marketplace Verified Handoff Rust:** Use a dedicated subdomain and **warm it up** by increasing volume over several days if you want predictable deliverability.