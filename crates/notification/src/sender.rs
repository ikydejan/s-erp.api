use web_push::{
    ContentEncoding, IsahcWebPushClient, SubscriptionInfo, VapidSignatureBuilder, WebPushClient,
    WebPushError, WebPushMessageBuilder, URL_SAFE_NO_PAD,
};

use crate::model::PushSubscription;

#[derive(Debug, Clone)]
pub struct VapidConfig {
    pub private_key_base64: String,
    pub subject: String,
}

/// Sends one Web Push message. Returns `Ok(true)` if delivered, `Ok(false)` if
/// the push service says the subscription is gone (caller should revoke it),
/// or `Err` for a transient failure worth retrying.
pub async fn send_push(
    subscription: &PushSubscription,
    vapid: &VapidConfig,
    payload_json: &str,
) -> anyhow::Result<bool> {
    let subscription_info = SubscriptionInfo::new(
        subscription.endpoint.clone(),
        subscription.p256dh_key.clone(),
        subscription.auth_key.clone(),
    );

    let mut sig_builder = VapidSignatureBuilder::from_base64(
        &vapid.private_key_base64,
        URL_SAFE_NO_PAD,
        &subscription_info,
    )
    .map_err(|e| anyhow::anyhow!("invalid VAPID private key: {e}"))?;
    sig_builder.add_claim("sub", vapid.subject.clone());
    let signature = sig_builder
        .build()
        .map_err(|e| anyhow::anyhow!("failed to sign VAPID claim: {e}"))?;

    let mut builder = WebPushMessageBuilder::new(&subscription_info);
    builder.set_payload(ContentEncoding::Aes128Gcm, payload_json.as_bytes());
    builder.set_vapid_signature(signature);
    let message = builder
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build web push message: {e}"))?;

    let client = IsahcWebPushClient::new()
        .map_err(|e| anyhow::anyhow!("failed to create web push client: {e}"))?;
    match client.send(message).await {
        Ok(()) => Ok(true),
        Err(WebPushError::EndpointNotValid) | Err(WebPushError::EndpointNotFound) => Ok(false),
        Err(e) => Err(anyhow::anyhow!("web push delivery failed: {e}")),
    }
}
