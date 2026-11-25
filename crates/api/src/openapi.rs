use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::routes::v1::{auth, delegations, health, items, notifications};

#[derive(OpenApi)]
#[openapi(
    info(title = "S-ERP API", version = "1.0.0", description = "ERP backend — v1"),
    paths(
        health::health,
        auth::register,
        auth::login,
        auth::mfa_verify,
        auth::refresh,
        auth::logout,
        auth::mfa_setup,
        auth::mfa_enable,
        auth::mfa_disable,
        auth::act_as,
        items::create_item,
        items::list_items,
        items::get_item,
        delegations::create_delegation,
        delegations::revoke_delegation,
        delegations::my_delegations,
        notifications::subscribe,
        notifications::unsubscribe,
        notifications::list_preferences,
        notifications::set_preference,
    ),
    components(schemas(
        health::HealthStatus,
        auth::RegisterRequest,
        auth::LoginRequest,
        auth::LoginResponse,
        auth::RefreshRequest,
        auth::MfaVerifyRequest,
        auth::MfaSetupResponse,
        auth::MfaEnableRequest,
        auth::MfaEnableResponse,
        auth::ActAsRequest,
        delegations::CreateDelegationRequest,
        delegations::DelegationView,
        notifications::SubscribeRequest,
        notifications::UnsubscribeRequest,
        notifications::SetPreferenceRequest,
    )),
    tags(
        (name = "health", description = "Liveness"),
        (name = "auth", description = "Registration, login, MFA, delegated act-as tokens"),
        (name = "items", description = "Reference CQRS module (Postgres write / OpenSearch read)"),
        (name = "delegations", description = "Delegate your tasks while on leave"),
        (name = "notifications", description = "Web Push subscriptions & preferences"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}
