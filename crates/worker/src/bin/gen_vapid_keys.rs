use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use p256::ecdsa::SigningKey;
use p256::elliptic_curve::rand_core::OsRng;

/// One-off CLI: `cargo run -p worker --bin gen_vapid_keys`. Prints a fresh
/// VAPID keypair to paste into `.env` as `VAPID_PRIVATE_KEY`/`VAPID_PUBLIC_KEY`.
fn main() {
    let signing_key = SigningKey::random(&mut OsRng);
    let private_key_b64 = URL_SAFE_NO_PAD.encode(signing_key.to_bytes());

    let public_point = signing_key.verifying_key().to_encoded_point(false);
    let public_key_b64 = URL_SAFE_NO_PAD.encode(public_point.as_bytes());

    println!("VAPID_PRIVATE_KEY={private_key_b64}");
    println!("VAPID_PUBLIC_KEY={public_key_b64}");
}
