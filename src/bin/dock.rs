fn main() {
    tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
    .add_directive("blue_environment=info".parse().unwrap()))
    .init();
    blue_environment::dock::run();
}
