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

    Router::new()
        .merge(protected_routes)
        .nest("/mpesa", mpesa::public_routes())
        .nest("/webhooks", webhooks::routes())
}
