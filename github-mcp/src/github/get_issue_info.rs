//! `get_issue_info`ツール本体。issueのタイトルと本文を取得する。

use rmcp::{ErrorData as McpError, handler::server::wrapper::Parameters, model::*, tool, tool_router};
use serde::{Deserialize, Serialize};

use super::{GitMcpServer, client::execute_query, types::IssueRequest};

/// issueの基本情報を取得するGraphQLクエリ。
const ISSUE_INFO_QUERY: &str = "
query GetIssueInfo($owner: String!, $name: String!, $issueNumber: Int!) {
  repository(owner: $owner, name: $name) {
    issue(number: $issueNumber) {
      title
      body
    }
  }
}
";

/// [`ISSUE_INFO_QUERY`]のレスポンスの`data`部分。
#[derive(Debug, Deserialize)]
struct IssueInfoData {
    /// 対象リポジトリ。owner/repoの組が存在しない場合は`None`。
    repository: Option<IssueInfoRepository>,
}

/// リポジトリの情報のうち、このクエリで使う部分。
#[derive(Debug, Deserialize)]
struct IssueInfoRepository {
    /// 対象のissue。指定した番号が存在しない場合は`None`。
    issue: Option<IssueInfoRaw>,
}

/// issueの情報(GraphQLレスポンスの形をそのまま反映したもの)。
#[derive(Debug, Deserialize)]
struct IssueInfoRaw {
    /// タイトル。
    title: String,
    /// 本文(description)。
    body: String,
}

/// `get_issue_info`ツールが返す、issue1件分の情報。
#[derive(Debug, Serialize)]
struct IssueInfo {
    /// タイトル。
    title: String,
    /// 本文(description)。
    body: String,
}

#[tool_router(router = tool_router_get_issue_info, vis = "pub(super)")]
impl GitMcpServer {
    /// 指定したissueのタイトルと本文を取得する。
    ///
    /// 指定した`owner/repo`または`issue_number`が存在しない場合はエラーを返す。
    #[tool(description = "指定したissueのタイトルと本文(description)を取得します")]
    async fn get_issue_info(
        &self, Parameters(IssueRequest { owner, repo, issue_number }): Parameters<IssueRequest>,
    ) -> Result<CallToolResult, McpError> {
        let owner = self.resolve_owner(owner)?;
        let repo = self.resolve_repo(repo)?;

        let data: IssueInfoData = execute_query(
            &self.client,
            &self.token,
            ISSUE_INFO_QUERY,
            serde_json::json!({ "owner": owner, "name": repo, "issueNumber": issue_number }),
        )
        .await?;

        let issue = data.repository.and_then(|r| r.issue).ok_or_else(|| {
            McpError::internal_error(
                format!("issue #{issue_number} が見つかりませんでした ({owner}/{repo})"),
                None,
            )
        })?;

        let result = IssueInfo { title: issue.title, body: issue.body };

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(format!("結果のシリアライズに失敗しました: {e}"), None))?;

        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }
}
