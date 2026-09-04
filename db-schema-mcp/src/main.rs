//! `db-schema-mcp`: PostgreSQLのテーブルスキーマ情報を取得するMCPサーバー。
//!
//! `rmcp`を使い、Streamable HTTP経由でMCPクライアントと通信する。
//! 常駐プロセスとして起動し、複数クライアントからの接続を1プロセスで受け付ける。
//! 提供するツールの実体は[`schema`]モジュールを参照。

mod schema;

use std::{env, net::SocketAddr};

use anyhow::Result;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use schema::DbSchemaServer;

/// 待受ポート。環境変数`MCP_PORT`で上書き可能(未設定時は`8081`)。
fn bind_address() -> Result<SocketAddr> {
    let port: u16 = env::var("MCP_PORT").unwrap_or_else(|_| "8081".to_string()).parse()?;
    Ok(SocketAddr::from(([0, 0, 0, 0], port)))
}

/// サーバーを起動し、MCPクライアントからの接続をStreamable HTTP経由で待ち受ける。
#[tokio::main]
async fn main() -> Result<()> {
    let ct = tokio_util::sync::CancellationToken::new();

    let service = StreamableHttpService::new(
        || DbSchemaServer::new().map_err(std::io::Error::other),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default().with_cancellation_token(ct.child_token()),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let addr = bind_address()?;
    let tcp_listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("db-schema-mcp: listening on http://{addr}/mcp");

    axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            ct.cancel();
        })
        .await?;

    Ok(())
}
