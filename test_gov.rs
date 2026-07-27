use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use std::sync::Arc;
fn main() {
    let conf = Arc::new(GovernorConfigBuilder::default().finish().unwrap());
    let layer = GovernorLayer { config: conf };
}
