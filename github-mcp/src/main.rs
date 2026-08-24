//! `github-mcp`: GitHub GraphQL APIを利用してプルリクエストの情報を取得するMCPサーバー。
//!
//! `rmcp`を使い、標準入出力(stdio)経由でMCPクライアントと通信する。
//! 提供するツールの実体は[`github`]モジュールを参照。

mod github;

use anyhow::Result;
use github::GitMcpServer;
use rmcp::{ServiceExt, transport::stdio};

/// サーバーを起動し、MCPクライアントからの接続をstdio経由で待ち受ける。
#[tokio::main]
async fn main() -> Result<()> {
    let service = GitMcpServer::new()?.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
