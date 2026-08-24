//! ツール間で共通の引数の型。

use rmcp::schemars;
use serde::Deserialize;

/// プルリクエスト番号を指定して情報を取得する系のツールに共通する引数。
///
/// `owner`・`repo`を省略した場合は、それぞれ環境変数`DEFAULT_OWNER`・`DEFAULT_REPOSITORY`が使われる
/// (どちらも未設定の場合はエラーになる)。
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct PrRequest {
    /// リポジトリの所有者。
    #[schemars(description = "リポジトリの所有者 (省略時は環境変数 DEFAULT_OWNER の値を使用)")]
    pub owner: Option<String>,
    /// リポジトリ名。
    #[schemars(description = "リポジトリ名 (省略時は環境変数 DEFAULT_REPOSITORY の値を使用)")]
    pub repo: Option<String>,
    /// プルリクエスト番号。
    #[schemars(description = "プルリクエスト番号")]
    pub pr_number: i32,
}

/// issue番号を指定して情報を取得する系のツールに共通する引数。
///
/// `owner`・`repo`を省略した場合は、それぞれ環境変数`DEFAULT_OWNER`・`DEFAULT_REPOSITORY`が使われる
/// (どちらも未設定の場合はエラーになる)。
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct IssueRequest {
    /// リポジトリの所有者。
    #[schemars(description = "リポジトリの所有者 (省略時は環境変数 DEFAULT_OWNER の値を使用)")]
    pub owner: Option<String>,
    /// リポジトリ名。
    #[schemars(description = "リポジトリ名 (省略時は環境変数 DEFAULT_REPOSITORY の値を使用)")]
    pub repo: Option<String>,
    /// issue番号。
    #[schemars(description = "issue番号")]
    pub issue_number: i32,
}
