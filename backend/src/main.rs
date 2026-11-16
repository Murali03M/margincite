use std::sync::Arc;

use margincite::ai::run::AiRunJob;
use margincite::config::Config;
use margincite::crawl::client::{CrawlClient, Network};
use margincite::crawl::render::{ChromeRenderer, NoRenderer, Renderer};
use margincite::crawl::runner::{CrawlDeps, CrawlJob, CrawlSettings};
use margincite::integrations::bing::sync::BingSyncJob;
use margincite::integrations::google::sync::GoogleSyncJob;
use margincite::jobs::lanes::lanes;
use margincite::jobs::worker::{self, JobHandler, Timing};
use margincite::keywords::volumes::KeywordVolumesJob;
use margincite::rankings::run::RankRunJob;
use margincite::speed::run::SpeedRunJob;
use margincite::{service, AppState};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let config = match Config::from_env(|key| std::env::var(key).ok())
        .and_then(Config::with_secret_key_file)
    {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };
    let pool = connect(&config.database_url).await;
    if let Err(err) = sqlx::migrate!().run(&pool).await {
        tracing::error!(error = %err, "migrations failed");
        eprintln!("Could not prepare the database.");
        std::process::exit(1);
    }
    let addr = config.bind_addr;
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!(error = %err, %addr, "bind failed");
            eprintln!("Could not listen on {addr}.");
            std::process::exit(1);
        }
    };
    tracing::info!(%addr, "listening");
    let state = AppState::new(pool, Arc::new(config));
    let shutdown = CancellationToken::new();
    let workers = tokio::spawn(run_workers(state.clone(), shutdown.clone()));
    tokio::select! {
        result = axum::serve(listener, service(state)) => {
            if let Err(err) = result {
                tracing::error!(error = %err, "server stopped");
            }
        }
        _ = tokio::signal::ctrl_c() => tracing::info!("shutting down"),
    }
    shutdown.cancel();
    workers.await.ok();
}

async fn connect(database_url: &str) -> sqlx::PgPool {
    match sqlx::PgPool::connect(database_url).await {
        Ok(pool) => pool,
        Err(err) => {
            tracing::error!(error = %err, "database connection failed");
            eprintln!("Could not connect to the database.");
            std::process::exit(1);
        }
    }
}

async fn renderer_for(state: &AppState) -> Arc<dyn Renderer> {
    let Some(path) = state.config.rendering_browser() else {
        if state.config.chrome_path.is_some() {
            tracing::warn!(
                "CHROME_PATH is set but page rendering is off: set RENDER_SITES_I_TRUST=yes to turn it on. \
                 Rendering lets the pages being crawled make this machine fetch internal addresses, \
                 so use it only on your own machine for sites you own."
            );
        } else {
            tracing::info!("page rendering is off");
        }
        return Arc::new(NoRenderer);
    };
    match ChromeRenderer::launch(path, Network::Public).await {
        Ok(renderer) => {
            tracing::info!(path = %path.display(), "page rendering uses Chrome");
            Arc::new(renderer)
        }
        Err(err) => {
            tracing::error!(error = %err, "Chrome did not start");
            Arc::new(NoRenderer)
        }
    }
}

/// AI runs, rank runs, and page speed and search volume checks wait on slow
/// outside services for minutes, so each group gets a worker loop of its own
/// (see `jobs::lanes`) and crawls and syncs never wait behind them.
async fn run_workers(state: AppState, shutdown: CancellationToken) {
    let renderer = renderer_for(&state).await;
    let deps = CrawlDeps {
        pool: state.pool.clone(),
        client: CrawlClient::new("unset", Network::Public),
        renderer,
        settings: CrawlSettings::default(),
        config: Some(state.config.clone()),
    };
    let handlers: Vec<Arc<dyn JobHandler>> = vec![
        Arc::new(CrawlJob { deps }),
        Arc::new(GoogleSyncJob::new(
            state.pool.clone(),
            state.config.clone(),
            state.google.clone(),
        )),
        Arc::new(BingSyncJob::new(
            state.pool.clone(),
            state.config.clone(),
            state.bing.clone(),
        )),
        Arc::new(SpeedRunJob::new(
            state.pool.clone(),
            state.config.clone(),
            state.pagespeed.clone(),
        )),
        Arc::new(KeywordVolumesJob::new(
            state.pool.clone(),
            state.config.clone(),
            state.dataforseo.clone(),
        )),
        Arc::new(RankRunJob::new(
            state.pool.clone(),
            state.config.clone(),
            state.dataforseo.clone(),
        )),
        Arc::new(AiRunJob::new(
            state.pool.clone(),
            state.config.clone(),
            state.engines.clone(),
        )),
    ];
    let lanes = lanes();
    let loops = lanes.map(|lane| {
        worker::run_lane(
            state.pool.clone(),
            handlers.clone(),
            lane,
            Timing::default(),
            shutdown.clone(),
        )
    });
    let [ai, rank, checks, general] = loops;
    tokio::join!(ai, rank, checks, general);
}
