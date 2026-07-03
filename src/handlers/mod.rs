pub mod loans;
pub mod members;
pub mod mpesa;
pub mod penalties;
pub mod transactions;

use axum::Router;

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .nest("/members", members::routes())
        .nest("/transactions", transactions::routes())
        .nest("/loans", loans::routes())
        .nest("/penalties", penalties::routes())
        .nest("/mpesa", mpesa::routes())
}
