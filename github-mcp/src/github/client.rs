//! GitHub GraphQL APIへの共通リクエスト処理。
//!
//! 各ツールモジュールは、クエリ文字列と変数を渡して[`execute_query`]を呼ぶだけで、
//! HTTPリクエストの送信・GraphQLエラーの判定・レスポンスのデシリアライズをまとめて行える。

use rmcp::ErrorData as McpError;
use serde::{Deserialize, de::DeserializeOwned};

/// GitHub GraphQL APIのエンドポイントURL。
pub(super) const GITHUB_GRAPHQL_URL: &str = "https://api.github.com/graphql";

/// GraphQLレスポンスの共通の形(`data`と`errors`)。
#[derive(Debug, Deserialize)]
struct GraphQlResponse<T> {
    /// クエリが成功した場合の結果。`errors`のみが返った場合は`None`になりうる。
    data: Option<T>,
    /// GraphQL側で発生したエラーの一覧。エラーが無ければ`None`。
    errors: Option<Vec<GraphQlError>>,
}

/// GraphQLエラー1件分の情報。
#[derive(Debug, Deserialize)]
struct GraphQlError {
    /// エラーメッセージ。
    message: String,
}

/// GitHub GraphQL APIに対して`query`を`variables`付きで実行し、`data`部分を`T`として返す。
///
/// GraphQL APIはHTTPステータスが200でも`errors`フィールドでエラーを返すことがあるため、
/// レスポンスをデシリアライズした後に`errors`の有無を確認し、あればエラーとして返す。
pub(super) async fn execute_query<T: DeserializeOwned>(
    client: &reqwest::Client, token: &str, query: &str, variables: serde_json::Value,
) -> Result<T, McpError> {
    let body = serde_json::json!({ "query": query, "variables": variables });

    let response = client
        .post(GITHUB_GRAPHQL_URL)
        .bearer_auth(token)
        .header("User-Agent", "github-mcp")
        .json(&body)
        .send()
        .await
        .map_err(|e| McpError::internal_error(format!("GitHub APIへのリクエストに失敗しました: {e}"), None))?;

    let graphql_response: GraphQlResponse<T> = response
        .json()
        .await
        .map_err(|e| McpError::internal_error(format!("GitHub APIのレスポンス解析に失敗しました: {e}"), None))?;

    if let Some(errors) = graphql_response.errors {
        let messages: Vec<String> = errors.into_iter().map(|e| e.message).collect();
        return Err(McpError::internal_error(
            format!("GitHub GraphQL APIがエラーを返しました: {}", messages.join(", ")),
            None,
        ));
    }

    graphql_response
        .data
        .ok_or_else(|| McpError::internal_error("GitHub APIからデータが返されませんでした".to_string(), None))
}
