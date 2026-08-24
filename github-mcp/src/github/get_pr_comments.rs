//! `get_pr_comments`ツール本体。プルリクエストの会話コメント・レビュー本体・
//! レビューコメント(付けられたファイルパス・行番号付き)を取得する。

use rmcp::{ErrorData as McpError, handler::server::wrapper::Parameters, model::*, tool, tool_router};
use serde::{Deserialize, Serialize};

use super::{GitMcpServer, client::execute_query, types::PrRequest};

/// プルリクエストのコメント・レビューコメントを取得するGraphQLクエリ。
const PR_COMMENTS_QUERY: &str = "
query GetPrComments($owner: String!, $name: String!, $prNumber: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $prNumber) {
      comments(first: 100) { nodes { id body } }
      reviews(first: 100) { nodes { id body state } }
      reviewThreads(first: 100) {
        nodes {
          path
          comments(first: 100) { nodes { id body line } }
        }
      }
    }
  }
}
";

/// [`PR_COMMENTS_QUERY`]のレスポンスの`data`部分。
#[derive(Debug, Deserialize)]
struct PrCommentsData {
    /// 対象リポジトリ。owner/repoの組が存在しない場合は`None`。
    repository: Option<PrCommentsRepository>,
}

/// リポジトリの情報のうち、このクエリで使う部分。
#[derive(Debug, Deserialize)]
struct PrCommentsRepository {
    /// 対象のプルリクエスト。指定した番号が存在しない場合は`None`。
    #[serde(rename = "pullRequest")]
    pull_request: Option<PrCommentsRaw>,
}

/// プルリクエストのコメント情報(GraphQLレスポンスの形をそのまま反映したもの)。
#[derive(Debug, Deserialize)]
struct PrCommentsRaw {
    /// PR本体に対する会話コメント(先頭100件)。
    comments: CommentConnection,
    /// レビュー本体(先頭100件)。
    reviews: ReviewConnection,
    /// コード差分に対するレビューコメントのスレッド(先頭100件)。
    #[serde(rename = "reviewThreads")]
    review_threads: ReviewThreadConnection,
}

/// 会話コメントの一覧(GraphQLのコネクション形式)。
#[derive(Debug, Deserialize)]
struct CommentConnection {
    /// コメントのノード一覧。
    nodes: Vec<CommentNode>,
}

/// 会話コメント1件分。
#[derive(Debug, Deserialize)]
struct CommentNode {
    /// コメントID。
    id: String,
    /// 本文。
    body: String,
}

/// レビュー本体の一覧(GraphQLのコネクション形式)。
#[derive(Debug, Deserialize)]
struct ReviewConnection {
    /// レビューのノード一覧。
    nodes: Vec<ReviewNode>,
}

/// レビュー本体1件分。
#[derive(Debug, Deserialize)]
struct ReviewNode {
    /// レビューID。
    id: String,
    /// レビュー本文。
    body: String,
    /// レビューの状態(APPROVED、CHANGES_REQUESTEDなど)。
    state: String,
}

/// レビュースレッドの一覧(GraphQLのコネクション形式)。
#[derive(Debug, Deserialize)]
struct ReviewThreadConnection {
    /// レビュースレッドのノード一覧。
    nodes: Vec<ReviewThreadNode>,
}

/// 1つのファイルに対するレビュースレッド。
#[derive(Debug, Deserialize)]
struct ReviewThreadNode {
    /// コメントが付けられたファイルのパス。
    path: String,
    /// このスレッド内のレビューコメント(先頭100件)。
    comments: ReviewCommentConnection,
}

/// レビューコメントの一覧(GraphQLのコネクション形式)。
#[derive(Debug, Deserialize)]
struct ReviewCommentConnection {
    /// レビューコメントのノード一覧。
    nodes: Vec<ReviewCommentNode>,
}

/// レビューコメント1件分。
#[derive(Debug, Deserialize)]
struct ReviewCommentNode {
    /// コメントID。
    id: String,
    /// 本文。
    body: String,
    /// コメントが付けられた行番号。差分の対象外になった行など、行に紐付かない場合は`None`。
    line: Option<i32>,
}

/// `get_pr_comments`ツールが返す、PR本体への会話コメント1件分。
#[derive(Debug, Serialize)]
struct PrComment {
    /// コメントID。
    id: String,
    /// 本文。
    body: String,
}

/// `get_pr_comments`ツールが返す、レビュー本体1件分。
#[derive(Debug, Serialize)]
struct Review {
    /// レビューID。
    id: String,
    /// レビュー本文。
    body: String,
    /// レビューの状態(APPROVED、CHANGES_REQUESTEDなど)。
    state: String,
}

/// `get_pr_comments`ツールが返す、レビューコメント1件分。
#[derive(Debug, Serialize)]
struct ReviewComment {
    /// コメントID。
    id: String,
    /// 本文。
    body: String,
    /// コメントが付けられた行番号。行に紐付かない場合は`None`。
    line: Option<i32>,
}

/// `get_pr_comments`ツールが返す、1ファイル分のレビューコメントのまとまり。
#[derive(Debug, Serialize)]
struct ReviewThread {
    /// コメントが付けられたファイルのパス。
    path: String,
    /// このファイルに対するレビューコメントの一覧。
    comments: Vec<ReviewComment>,
}

/// `get_pr_comments`ツールが返す、プルリクエスト1件分のコメント情報全体。
#[derive(Debug, Serialize)]
struct PrComments {
    /// PR本体に対する会話コメントの一覧。
    comments: Vec<PrComment>,
    /// レビュー本体の一覧。
    reviews: Vec<Review>,
    /// ファイルごとのレビューコメントの一覧。
    review_threads: Vec<ReviewThread>,
}

#[tool_router(router = tool_router_get_pr_comments, vis = "pub(super)")]
impl GitMcpServer {
    /// 指定したプルリクエストの会話コメント・レビュー本体・レビューコメント(ファイルパス・行番号付き)を取得する。
    ///
    /// 指定した`owner/repo`または`pr_number`が存在しない場合はエラーを返す。
    #[tool(
        description = "指定したプルリクエストの会話コメント・レビュー本体・レビューコメント(ファイルパス・行番号付き)を取得します"
    )]
    async fn get_pr_comments(
        &self, Parameters(PrRequest { owner, repo, pr_number }): Parameters<PrRequest>,
    ) -> Result<CallToolResult, McpError> {
        let owner = self.resolve_owner(owner)?;
        let repo = self.resolve_repo(repo)?;

        let data: PrCommentsData = execute_query(
            &self.client,
            &self.token,
            PR_COMMENTS_QUERY,
            serde_json::json!({ "owner": owner, "name": repo, "prNumber": pr_number }),
        )
        .await?;

        let pull_request = data.repository.and_then(|r| r.pull_request).ok_or_else(|| {
            McpError::internal_error(
                format!("プルリクエスト #{pr_number} が見つかりませんでした ({owner}/{repo})"),
                None,
            )
        })?;

        let result = PrComments {
            comments: pull_request
                .comments
                .nodes
                .into_iter()
                .map(|n| PrComment { id: n.id, body: n.body })
                .collect(),
            reviews: pull_request
                .reviews
                .nodes
                .into_iter()
                .map(|n| Review { id: n.id, body: n.body, state: n.state })
                .collect(),
            review_threads: pull_request
                .review_threads
                .nodes
                .into_iter()
                .map(|t| ReviewThread {
                    path: t.path,
                    comments: t
                        .comments
                        .nodes
                        .into_iter()
                        .map(|c| ReviewComment { id: c.id, body: c.body, line: c.line })
                        .collect(),
                })
                .collect(),
        };

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(format!("結果のシリアライズに失敗しました: {e}"), None))?;

        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }
}
