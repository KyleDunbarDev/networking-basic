pub mod client;
pub mod common;
pub mod server;

pub mod logging {
    use tracing_subscriber::{fmt, EnvFilter};

    pub fn init() {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::from_default_env()
                    .add_directive("networking_basic=debug".parse().unwrap())
                    .add_directive("warn".parse().unwrap()),
            )
            .init();
    }
}
