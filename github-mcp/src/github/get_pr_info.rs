//! `get_pr_info`ツール本体。プルリクエストのタイトル・本文・作成者・コミットIDなどを取得する。

use rmcp::{ErrorData as McpError, handler::server::wrapper::Parameters, model::*, tool, tool_router};
use serde::{Deserialize, Serialize};

use super::{GitMcpServer, client::execute_query, types::PrRequest};

/// プルリクエストの基本情報を取得するGraphQLクエリ。
const PR_INFO_QUERY: &str = "
query GetPrInfo($owner: String!, $name: String!, $prNumber: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $prNumber) {
      title
      body
      author { login }
      headRefName
      commits(first: 100) { nodes { commit { oid } } }
    }
  }
}
";

/// [`PR_INFO_QUERY`]のレスポンスの`data`部分。
#[derive(Debug, Deserialize)]
struct PrInfoData {
    /// 対象リポジトリ。owner/repoの組が存在しない場合は`None`。
    repository: Option<PrInfoRepository>,
}

/// リポジトリの情報のうち、このクエリで使う部分。
#[derive(Debug, Deserialize)]
struct PrInfoRepository {
    /// 対象のプルリクエスト。指定した番号が存在しない場合は`None`。
    #[serde(rename = "pullRequest")]
    pull_request: Option<PrInfoRaw>,
}

/// プルリクエストの情報(GraphQLレスポンスの形をそのまま反映したもの)。
#[derive(Debug, Deserialize)]
struct PrInfoRaw {
    /// タイトル。
    title: String,
    /// 本文(description)。
    body: String,
    /// 作成者。アカウントが削除されている場合は`None`。
    author: Option<Author>,
    /// マージ元ブランチ名。
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    /// このプルリクエストに含まれるコミット(先頭100件)。
    commits: CommitConnection,
}

/// GitHubユーザー。
#[derive(Debug, Deserialize)]
struct Author {
    /// ログイン名。
    login: String,
}

/// コミットの一覧(GraphQLのコネクション形式)。
#[derive(Debug, Deserialize)]
struct CommitConnection {
    /// コミットのノード一覧。
    nodes: Vec<CommitNode>,
}

/// コミット1件分のノード。
#[derive(Debug, Deserialize)]
struct CommitNode {
    /// コミット本体。
    commit: CommitOid,
}

/// コミットのSHA(oid)のみを持つ型。
#[derive(Debug, Deserialize)]
struct CommitOid {
    /// コミットSHA。
    oid: String,
}

/// `get_pr_info`ツールが返す、プルリクエスト1件分の情報。
#[derive(Debug, Serialize)]
struct PullRequestInfo {
    /// タイトル。
    title: String,
    /// 本文(description)。
    body: String,
    /// 作成者のログイン名。アカウントが削除されている場合は`None`。
    author: Option<String>,
    /// マージ元ブランチ名。
    head_ref_name: String,
    /// このプルリクエストに含まれるコミットSHAの一覧(先頭100件、古い順)。
    commit_ids: Vec<String>,
}

#[tool_router(router = tool_router_get_pr_info, vis = "pub(super)")]
impl GitMcpServer {
    /// 指定したプルリクエストのタイトル・本文・作成者・マージ元ブランチ・コミットIDを取得する。
    ///
    /// 指定した`owner/repo`または`pr_number`が存在しない場合はエラーを返す。
    #[tool(description = "指定したプルリクエストのタイトル、本文、作成者、コミットIDなどの情報を取得します")]
    async fn get_pr_info(
        &self, Parameters(PrRequest { owner, repo, pr_number }): Parameters<PrRequest>,
    ) -> Result<CallToolResult, McpError> {
        let owner = self.resolve_owner(owner)?;
        let repo = self.resolve_repo(repo)?;

        let data: PrInfoData = execute_query(
            &self.client,
            &self.token,
            PR_INFO_QUERY,
            serde_json::json!({ "owner": owner, "name": repo, "prNumber": pr_number }),
        )
        .await?;

        let pull_request = data.repository.and_then(|r| r.pull_request).ok_or_else(|| {
            McpError::internal_error(
                format!("プルリクエスト #{pr_number} が見つかりませんでした ({owner}/{repo})"),
                None,
            )
        })?;

        let result = PullRequestInfo {
            title: pull_request.title,
            body: pull_request.body,
            author: pull_request.author.map(|a| a.login),
            head_ref_name: pull_request.head_ref_name,
            commit_ids: pull_request.commits.nodes.into_iter().map(|n| n.commit.oid).collect(),
        };

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(format!("結果のシリアライズに失敗しました: {e}"), None))?;

        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }
}
