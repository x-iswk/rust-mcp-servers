//! `GitMcpServer`本体と、GitHub関連ツールの共通処理(owner/repoの解決、`ServerHandler`実装)。
//!
//! 個々のツールの実体は[`get_pr_info`]・[`get_pr_comments`]・[`get_issue_info`]モジュールを、
//! GitHub GraphQL APIへの共通リクエスト処理は[`client`]モジュールを参照。

mod client;
mod get_issue_info;
mod get_pr_comments;
mod get_pr_info;
mod types;

use std::env;

use anyhow::{Context, Result};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool_handler,
};

/// `github-mcp`のMCPサーバー本体。
///
/// GitHub GraphQL APIへの接続情報(`client`・`token`)と、ツール引数でowner/repoが
/// 省略された場合の既定値(`default_owner`・`default_repository`)、および各ツール
/// モジュールが生成する`ToolRouter`を合成したもの(`tool_router`)を保持する。
#[derive(Clone)]
pub struct GitMcpServer {
    /// GitHub GraphQL APIへのリクエストに使うHTTPクライアント。
    client: reqwest::Client,
    /// GitHub APIの認証に使うトークン(環境変数`GITHUB_TOKEN`)。
    token: String,
    /// ツール引数で`owner`が省略された場合に使う既定値(環境変数`DEFAULT_OWNER`)。
    default_owner: Option<String>,
    /// ツール引数で`repo`が省略された場合に使う既定値(環境変数`DEFAULT_REPOSITORY`)。
    default_repository: Option<String>,
    #[allow(dead_code)]
    tool_router: ToolRouter<GitMcpServer>,
}

impl GitMcpServer {
    /// 環境変数からGitHub認証情報とowner/repoの既定値を読み込み、サーバーを初期化する。
    ///
    /// 各ツールモジュールが個別に生成する`ToolRouter`(`tool_router_get_pr_info`・
    /// `tool_router_get_pr_comments`・`tool_router_get_issue_info`)を`+`で合成し、
    /// 1つの`ToolRouter`にまとめる。
    pub fn new() -> Result<Self> {
        let token = env::var("GITHUB_TOKEN").context("環境変数 GITHUB_TOKEN が設定されていません")?;

        Ok(Self {
            client: reqwest::Client::new(),
            token,
            default_owner: env::var("DEFAULT_OWNER").ok(),
            default_repository: env::var("DEFAULT_REPOSITORY").ok(),
            tool_router: Self::tool_router_get_pr_info()
                + Self::tool_router_get_pr_comments()
                + Self::tool_router_get_issue_info(),
        })
    }

    /// ツール引数の`owner`を解決する。
    ///
    /// 引数で指定されていればそれを使い、省略時は`default_owner`を使う。
    /// どちらも無い場合はエラーを返す。
    fn resolve_owner(&self, owner: Option<String>) -> Result<String, McpError> {
        owner.or_else(|| self.default_owner.clone()).ok_or_else(|| {
            McpError::invalid_params(
                "owner が指定されておらず、環境変数 DEFAULT_OWNER も設定されていません".to_string(),
                None,
            )
        })
    }

    /// ツール引数の`repo`を解決する。
    ///
    /// 引数で指定されていればそれを使い、省略時は`default_repository`を使う。
    /// どちらも無い場合はエラーを返す。
    fn resolve_repo(&self, repo: Option<String>) -> Result<String, McpError> {
        repo.or_else(|| self.default_repository.clone()).ok_or_else(|| {
            McpError::invalid_params(
                "repo が指定されておらず、環境変数 DEFAULT_REPOSITORY も設定されていません".to_string(),
                None,
            )
        })
    }
}

#[tool_handler(router = self.tool_router.clone())]
impl ServerHandler for GitMcpServer {
    /// サーバー情報・対応プロトコルバージョン・利用方法の説明を返す。
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "GitHub GraphQL API を利用してプルリクエスト・issueの情報を取得するツールを提供します。\
                get_pr_info でPRの基本情報、get_pr_comments でレビューコメント、\
                get_issue_info でissueのタイトル・本文を取得できます。\
                owner/repo を省略した場合は環境変数 DEFAULT_OWNER / DEFAULT_REPOSITORY を使用します。"
                    .to_string(),
            )
    }
}
