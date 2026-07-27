pub mod cycles;
pub mod groups;
pub mod loans;
pub mod meetings;
pub mod members;
pub mod mpesa;
pub mod penalties;
pub mod transactions;
pub mod users;
pub mod webhooks;
pub mod invitations;
pub mod analytics;

use axum::Router;
use std::sync::Arc;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

use crate::AppState;

pub fn routes(state: AppState) -> Router<AppState> {
    let protected_routes = Router::new()
        .nest("/users", users::routes())
        .nest("/groups", groups::routes())
        .nest("/cycles", cycles::routes())
        .nest("/meetings", meetings::routes())
        .nest("/members", members::routes())
        .nest("/transactions", transactions::routes())
        .nest("/loans", loans::routes())
        .nest("/penalties", penalties::routes())
        .nest("/invitations", invitations::routes())
        .nest("/analytics", analytics::routes())
        .nest("/mpesa", mpesa::protected_routes())
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), crate::middleware::require_auth));

    // Configure rate limiter: 5 requests per second, with a burst of 10
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(5)
            .burst_size(10)
            .finish()
            .unwrap(),
    );

    let public_mpesa_routes = Router::new()
        .nest("/mpesa", mpesa::public_routes())
        .nest("/webhooks", webhooks::routes())
        .layer(GovernorLayer::new(governor_conf.clone()));

    Router::new()
        .merge(protected_routes)
        .merge(public_mpesa_routes)
}
