pub mod auth;
pub mod cycles;
pub mod groups;
pub mod loans;
pub mod meetings;
pub mod members;
pub mod mpesa;
pub mod penalties;
pub mod transactions;

use axum::Router;

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::routes())
        .nest("/groups", groups::routes())
        .nest("/cycles", cycles::routes())
        .nest("/meetings", meetings::routes())
        .nest("/members", members::routes())
        .nest("/transactions", transactions::routes())
        .nest("/loans", loans::routes())
        .nest("/penalties", penalties::routes())
        .nest("/mpesa", mpesa::routes())
}
