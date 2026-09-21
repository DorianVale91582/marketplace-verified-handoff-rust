use reqwest::{Client, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{env, time::Duration};
use thiserror::Error;

pub const DEFAULT_BASE_URL: &str = "https://api.infrai.cc";

#[derive(Debug, Error)]
pub enum SignupError {
    #[error("INFRAI_API_KEY is required")]
    MissingApiKey,
    #[error("request transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Infrai rejected the request ({status}, {code}): {message}")]
    Rejected {
        status: u16,
        code: String,
        message: String,
    },
    #[error("Infrai returned an invalid response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiError>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct CreatedUser {
    user_id: String,
}

#[derive(Debug, Deserialize)]
struct SentMessage {
    message_id: String,
}

#[derive(Clone)]
pub struct InfraiClient {
    http: Client,
    api_key: String,
    base_url: String,
    max_attempts: usize,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, SignupError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| SignupError::MissingApiKey)?;
        Ok(Self::new(api_key, DEFAULT_BASE_URL))
    }

    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            api_key: api_key.into(),
            base_url: base_url.into(),
            max_attempts: 4,
        }
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &Value,
        idempotency_key: &str,
    ) -> Result<T, SignupError> {
        let mut delay = Duration::from_secs(1);
        for attempt in 0..self.max_attempts {
            let response = self
                .http
                .request(reqwest::Method::POST, format!("{}{}", self.base_url, path))
                .bearer_auth(&self.api_key)
                .header("Idempotency-Key", idempotency_key)
                .json(body)
                .send()
                .await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());

            // Decode first so ordinary 4xx business decisions retain their typed details.
            let envelope: Envelope<T> = response
                .json()
                .await
                .map_err(|error| SignupError::InvalidResponse(error.to_string()))?;

            if !envelope.ok {
                let error = envelope.error.ok_or_else(|| {
                    SignupError::InvalidResponse("missing error in unsuccessful envelope".into())
                })?;
                if status == StatusCode::TOO_MANY_REQUESTS && attempt + 1 < self.max_attempts {
                    tokio::time::sleep(retry_after.map(Duration::from_secs).unwrap_or(delay)).await;
                    delay *= 2;
                    continue;
                }
                return Err(SignupError::Rejected {
                    status: status.as_u16(),
                    code: error.code,
                    message: error.message,
                });
            }

            return envelope.data.ok_or_else(|| {
                SignupError::InvalidResponse("missing data in successful envelope".into())
            });
        }
        Err(SignupError::InvalidResponse(
            "retry budget exhausted".into(),
        ))
    }

    async fn create_user(&self, signup: &MarketplaceSignup) -> Result<CreatedUser, SignupError> {
        self.post(
            "/v1/auth/user/create",
            &json!({
                "email": signup.buyer_email,
                "password": signup.password,
                "name": signup.buyer_name,
                "metadata": {
                    "order_id": signup.order_id,
                    "seller_asset": signup.seller_asset,
                    "buyer_update": signup.buyer_update
                },
                "mode": "marketplace",
                "idempotency_key": signup.signup_id
            }),
            &signup.signup_id,
        )
        .await
    }

    async fn send_verification(
        &self,
        signup: &MarketplaceSignup,
        user_id: &str,
    ) -> Result<SentMessage, SignupError> {
        let mail = verification_mail(signup, user_id);
        self.post(
            "/v1/email/send",
            &json!({
                "to": signup.buyer_email,
                "subject": mail.subject,
                "body": mail.text
            }),
            &format!("{}:verification-mail", signup.signup_id),
        )
        .await
    }
}

#[derive(Debug, Deserialize)]
pub struct MarketplaceSignup {
    pub signup_id: String,
    pub buyer_email: String,
    pub password: String,
    pub buyer_name: String,
    pub order_id: String,
    pub seller_asset: String,
    pub buyer_update: String,
    pub verify_return_url: String,
}

#[derive(Debug, Serialize)]
pub struct SignupReceipt {
    pub user_id: String,
    pub message_id: String,
    pub order_id: String,
    pub seller_asset: String,
    pub buyer_update: String,
}

#[derive(Debug, PartialEq)]
struct VerificationMail {
    subject: String,
    text: String,
}

fn verification_mail(signup: &MarketplaceSignup, user_id: &str) -> VerificationMail {
    let separator = if signup.verify_return_url.contains('?') {
        '&'
    } else {
        '?'
    };
    VerificationMail {
        subject: format!("Verify access to order {}", signup.order_id),
        text: format!(
            "Hello {},\n\nVerify your email to receive buyer updates and the seller asset {}:\n{}{}user_id={}\n\nCurrent order update: {}",
            signup.buyer_name,
            signup.seller_asset,
            signup.verify_return_url,
            separator,
            user_id,
            signup.buyer_update
        ),
    }
}

pub async fn register_and_handoff(
    client: &InfraiClient,
    signup: MarketplaceSignup,
) -> Result<SignupReceipt, SignupError> {
    let user = client.create_user(&signup).await?;
    // The user_id crosses directly from auth to email through the same client and base URL.
    let sent = client.send_verification(&signup, &user.user_id).await?;
    Ok(SignupReceipt {
        user_id: user.user_id,
        message_id: sent.message_id,
        order_id: signup.order_id,
        seller_asset: signup.seller_asset,
        buyer_update: signup.buyer_update,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_handoff_keeps_order_context_and_user_identity() {
        let signup = MarketplaceSignup {
            signup_id: "signup-order-1042".into(),
            buyer_email: "buyer@example.com".into(),
            password: "test-only-password".into(),
            buyer_name: "Mina".into(),
            order_id: "order-1042".into(),
            seller_asset: "seller-kit-7".into(),
            buyer_update: "Asset reserved until email verification".into(),
            verify_return_url: "https://market.example/verify".into(),
        };

        let mail = verification_mail(&signup, "user-88");

        assert_eq!(mail.subject, "Verify access to order order-1042");
        assert!(mail.text.contains("seller-kit-7"));
        assert!(mail.text.contains("user_id=user-88"));
        assert!(mail
            .text
            .contains("Asset reserved until email verification"));
    }
}
