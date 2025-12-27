mod routes;

use std::path::PathBuf;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

pub struct TileServer {
    #[allow(dead_code)] // Kept alive to keep server running
    runtime: Runtime,
    port: u16,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl TileServer {
    pub fn start(tiles_dir: PathBuf) -> Result<Self, std::io::Error> {
        let runtime = Runtime::new()?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (port_tx, port_rx) = oneshot::channel();

        let app = routes::create_router(tiles_dir);

        runtime.spawn(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let _ = port_tx.send(addr.port());

            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        let port = runtime.block_on(async { port_rx.await.unwrap() });

        Ok(Self {
            runtime,
            port,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for TileServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}
