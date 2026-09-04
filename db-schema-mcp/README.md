# db-schema-mcp

PostgreSQLのテーブルスキーマ情報を取得するMCPサーバー。

## 起動

リポジトリルートで以下を実行する(`.env`が必要。`.env.example`を参照)。

```bash
docker compose up -d db-schema-mcp
```

停止する場合:

```bash
docker compose down
```

デフォルトでは`8081`番ポートで待ち受ける(`.env`の`MCP_PORT`で変更可能)。

## MCPクライアントへの登録

`~/.claude.json`の`mcpServers`(または`.mcp.json`)を以下のように編集する。

```json
{
  "db-schema-mcp": {
    "type": "http",
    "url": "http://localhost:8081/mcp"
  }
}
```

`claude mcp add`コマンドで登録する場合:

```bash
claude mcp add --transport http db-schema-mcp http://localhost:8081/mcp -s user
```

サーバーが起動していない状態でクライアントから接続しようとすると失敗するため、  
事前に`docker compose up -d db-schema-mcp`でコンテナを起動しておくこと。
