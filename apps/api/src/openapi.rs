use utoipa::OpenApi;

use crate::transport::{
    auth::{
        BootstrapRequest, ChangePasswordRequest, DisableTotpRequest, LoginRequest, RefreshRequest,
        TotpVerifyRequest,
    },
    subscriptions::{
        CreateSubscriptionRequest, CreateSubscriptionResponse, DeviceResponse,
        PublicSubscriptionResponse, RotateSubscriptionLinkResponse, SubscriptionResponse,
        UpdateSubscriptionRequest,
    },
    users::{
        CreateResellerRequest, CreateUserRequest, UpdateResellerRequest, UpdateUserRequest,
        UserResponse,
    },
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::transport::audit::list_audit_logs,
        crate::transport::auth::bootstrap,
        crate::transport::auth::login,
        crate::transport::auth::refresh,
        crate::transport::auth::logout,
        crate::transport::auth::logout_all,
        crate::transport::auth::begin_totp_setup,
        crate::transport::auth::verify_totp,
        crate::transport::auth::disable_totp,
        crate::transport::auth::change_password,
        crate::transport::auth::list_sessions,
        crate::transport::users::create_user,
        crate::transport::users::list_users,
        crate::transport::users::update_user,
        crate::transport::users::delete_user,
        crate::transport::users::create_reseller,
        crate::transport::users::list_resellers,
        crate::transport::users::update_reseller,
        crate::transport::users::delete_reseller,
        crate::transport::subscriptions::create_subscription,
        crate::transport::subscriptions::update_subscription,
        crate::transport::subscriptions::delete_subscription,
        crate::transport::subscriptions::public_subscription,
        crate::transport::subscriptions::rotate_subscription_link,
        crate::transport::subscriptions::list_devices,
        crate::transport::subscriptions::list_subscriptions,
        crate::transport::subscriptions::resolve_subscription,
        crate::transport::usage::list_usage,
        crate::transport::notifications::list_notifications
    ),
    components(schemas(
        BootstrapRequest,
        ChangePasswordRequest,
        DisableTotpRequest,
        LoginRequest,
        RefreshRequest,
        TotpVerifyRequest,
        CreateUserRequest,
        UpdateUserRequest,
        CreateResellerRequest,
        UpdateResellerRequest,
        UserResponse,
        CreateSubscriptionRequest,
        UpdateSubscriptionRequest,
        CreateSubscriptionResponse,
        DeviceResponse,
        PublicSubscriptionResponse,
        SubscriptionResponse,
        RotateSubscriptionLinkResponse
    ))
)]
pub struct ApiDoc;
