//! `db-schema-mcp`: PostgreSQLのテーブルスキーマ情報を取得するMCPサーバー。
//!
//! `rmcp`を使い、標準入出力(stdio)経由でMCPクライアントと通信する。
//! 提供するツールの実体は[`schema`]モジュールを参照。

mod schema;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use schema::DbSchemaServer;

/// サーバーを起動し、MCPクライアントからの接続をstdio経由で待ち受ける。
#[tokio::main]
async fn main() -> Result<()> {
    let service = DbSchemaServer::new()?.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
