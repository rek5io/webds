use axum::{
    Router,
    routing::{any, get},
};

fn main() {
    let args = webds::args::get_args();

    if args.debug {
        env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::init();
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let routes = Router::new()
            .route("/", get(webds::index))
            .route("/cap", any(webds::handle_cap))
            .route("/hid", any(webds::handle_hid));

        log::info!("starting up at port: {}", args.port);

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port))
            .await
            .unwrap();

        axum::serve(listener, routes).await.unwrap();
    });
}
