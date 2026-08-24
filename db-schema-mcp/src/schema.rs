//! `get_table_schema`ツール本体と、PostgreSQLの`information_schema`/`pg_indexes`への問い合わせ処理。

use std::{collections::HashSet, env};

use anyhow::{Context, Result};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio_postgres::{Client, Config, NoTls};

/// 1カラム分のスキーマ情報。
#[derive(Debug, Serialize)]
struct ColumnSchema {
    /// カラム名。
    name: String,
    /// PostgreSQL上のデータ型(例: `"integer"`, `"text"`)。
    data_type: String,
    /// NULLを許容するカラムか。
    is_nullable: bool,
    /// デフォルト値の定義。デフォルトが設定されていなければ`None`。
    default: Option<String>,
    /// 文字列型の最大長。該当しない型なら`None`。
    character_maximum_length: Option<i32>,
    /// 数値型の精度(桁数)。該当しない型なら`None`。
    numeric_precision: Option<i32>,
    /// 数値型の小数点以下の桁数。該当しない型なら`None`。
    numeric_scale: Option<i32>,
    /// このテーブルの主キーを構成するカラムか。
    is_primary_key: bool,
}

/// 1つの外部キー制約の情報。
#[derive(Debug, Serialize)]
struct ForeignKeySchema {
    /// 制約名。
    constraint_name: String,
    /// 外部キーを構成する、このテーブル側のカラム名。
    column: String,
    /// 参照先のスキーマ名。
    foreign_schema: String,
    /// 参照先のテーブル名。
    foreign_table: String,
    /// 参照先のカラム名。
    foreign_column: String,
}

/// 1つのインデックスの定義。
#[derive(Debug, Serialize)]
struct IndexSchema {
    /// インデックス名。
    name: String,
    /// `CREATE INDEX`文としての定義(`pg_indexes.indexdef`)。
    definition: String,
}

/// `get_table_schema`ツールが返す、テーブル1つ分のスキーマ情報全体。
#[derive(Debug, Serialize)]
struct TableSchema {
    /// テーブルが属するスキーマ名。
    schema: String,
    /// テーブル名。
    table: String,
    /// カラム定義の一覧(`ordinal_position`順)。
    columns: Vec<ColumnSchema>,
    /// 主キーを構成するカラム名の一覧(定義順)。
    primary_key: Vec<String>,
    /// 外部キー制約の一覧。
    foreign_keys: Vec<ForeignKeySchema>,
    /// インデックスの一覧。
    indexes: Vec<IndexSchema>,
}

/// `get_table_schema`ツールの引数。
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTableSchemaRequest {
    /// テーブルが属するスキーマ名。省略時は`"public"`として扱われる。
    #[schemars(description = "テーブルが属するスキーマ名(省略時は public)")]
    pub schema: Option<String>,
    /// スキーマ情報を取得する対象のテーブル名。
    #[schemars(description = "スキーマ情報を取得するテーブル名")]
    pub table: String,
}

/// 指定したテーブルの主キーを構成するカラム名を、定義順に取得する。
async fn fetch_primary_key_columns(
    client: &Client, schema: &str, table: &str
) -> Result<Vec<String>, McpError> {
    let rows = client.query(
        "SELECT kcu.column_name FROM information_schema.table_constraints tc \
        JOIN information_schema.key_column_usage kcu \
            ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema \
        WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_schema = $1 AND tc.table_name = $2 \
        ORDER BY kcu.ordinal_position",
        &[&schema, &table],
    )
    .await
    .map_err(|e| McpError::internal_error(format!("主キーの取得に失敗しました: {e}"), None))?;

    Ok(rows.iter().map(|row| row.get("column_name")).collect())
}

/// 指定したテーブルの全カラムの定義を取得する。
///
/// `primary_key`に含まれるカラム名は、返す`ColumnSchema::is_primary_key`が`true`になる。
async fn fetch_columns(
    client: &Client, schema: &str, table: &str, primary_key: &HashSet<String>,
) -> Result<Vec<ColumnSchema>, McpError> {
    let rows = client.query(
        "SELECT
            column_name, \
            data_type, \
            is_nullable, \
            column_default, \
            character_maximum_length, \
            numeric_precision, \
            numeric_scale \
            FROM information_schema.columns \
            WHERE table_schema = $1 AND table_name = $2 \
            ORDER BY ordinal_position",
        &[&schema, &table],
    )
    .await
    .map_err(|e| McpError::internal_error(format!("カラム情報の取得に失敗しました: {e}"), None))?;

    Ok(rows.iter().map(|row| {
        let name = row.get("column_name");
        ColumnSchema {
            is_primary_key: primary_key.contains(&name),
            name,
            data_type: row.get("data_type"),
            is_nullable: row.get::<_, String>("is_nullable") == "YES",
            default: row.get("column_default"),
            character_maximum_length: row.get("character_maximum_length"),
            numeric_precision: row.get("numeric_precision"),
            numeric_scale: row.get("numeric_scale"),
        }
    }).collect())
}

/// 指定したテーブルに定義されている外部キー制約を取得する。
async fn fetch_foreign_keys(
    client: &Client, schema: &str, table: &str,
) -> Result<Vec<ForeignKeySchema>, McpError> {
    let rows = client.query(
        "SELECT
            tc.constraint_name, \
            kcu.column_name, \
            ccu.table_schema AS foreign_schema, \
            ccu.table_name AS foreign_table, \
            ccu.column_name AS foreign_column \
            FROM information_schema.table_constraints tc \
            JOIN information_schema.key_column_usage kcu \
                ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema \
            JOIN information_schema.constraint_column_usage ccu \
                ON tc.constraint_name = ccu.constraint_name AND tc.table_schema = ccu.table_schema \
            WHERE tc.constraint_type = 'FOREIGN KEY' AND tc.table_schema = $1 AND tc.table_name = $2 \
            ORDER BY tc.constraint_name, kcu.ordinal_position",
        &[&schema, &table],
    )
    .await
    .map_err(|e| McpError::internal_error(format!("外部キーの取得に失敗しました: {e}"), None))?;

    Ok(rows.iter().map(|row| ForeignKeySchema {
        constraint_name: row.get("constraint_name"),
        column: row.get("column_name"),
        foreign_schema: row.get("foreign_schema"),
        foreign_table: row.get("foreign_table"),
        foreign_column: row.get("foreign_column"),
    }).collect())
}

/// 指定したテーブルに定義されているインデックスを取得する。
///
/// インデックスはSQL標準ではなくPostgreSQL独自の機能のため、`information_schema`ではなく
/// PostgreSQL固有のシステムカタログビュー`pg_indexes`を使用する。
async fn fetch_indexes(
    client: &Client, schema: &str, table: &str,
) -> Result<Vec<IndexSchema>, McpError> {
    let rows = client.query(
        "SELECT indexname, indexdef FROM pg_indexes WHERE schemaname = $1 AND tablename = $2 ORDER BY indexname",
        &[&schema, &table],
    )
    .await
    .map_err(|e| McpError::internal_error(format!("インデックス情報の取得に失敗しました: {e}"), None))?;

    Ok(rows.iter().map(|row| IndexSchema {
        name: row.get(0),
        definition: row.get(1),
    }).collect())
}

/// 指定したスキーマ・テーブルが実際に存在するかを確認する。
async fn table_exists(client: &Client, schema: &str, table: &str) -> Result<bool, McpError> {
    let row = client.query_one(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = $1 AND table_name = $2)",
        &[&schema, &table],
    )
    .await
    .map_err(|e| McpError::internal_error(format!("テーブル存在確認に失敗しました: {e}"), None))?;

    Ok(row.get(0))
}

/// 環境変数からPostgreSQLへの接続設定を組み立てる。
///
/// - `PGHOST`: 未設定時は`"localhost"`
/// - `PGPORT`: 未設定時は`5432`
/// - `PGDATABASE` / `PGUSER` / `PGPASSWORD`: 必須。未設定の場合はエラーを返す
fn build_config() -> Result<Config> {
    let mut config = Config::new();

    let host = env::var("PGHOST").unwrap_or_else(|_| "localhost".to_string());

    let port: u16 = env::var("PGPORT")
        .unwrap_or_else(|_| "5432".to_string())
        .parse()
        .context("PGPORT は数値で指定してください")?;

    let dbname = env::var("PGDATABASE").context("環境変数 PGDATABASE が設定されていません")?;

    let user = env::var("PGUSER").context("環境変数 PGUSER が設定されていません")?;

    let password = env::var("PGPASSWORD").context("環境変数 PGPASSWORD が設定されていません")?;

    config.host(&host).port(port).dbname(&dbname).user(&user).password(&password);

    Ok(config)
}

/// `db-schema-mcp`のMCPサーバー本体。
///
/// DB接続設定(`config`)と、`#[tool_router]`が生成するツールのルーティング情報(`tool_router`)を保持する。
#[derive(Clone)]
pub struct DbSchemaServer {
    config: Config,
    #[allow(dead_code)]
    tool_router: ToolRouter<DbSchemaServer>,
}

impl DbSchemaServer {
    /// 環境変数からDB接続設定を読み込み、サーバーを初期化する。
    pub fn new() -> Result<Self> {
        Ok(Self {
            config: build_config()?,
            tool_router: Self::tool_router(),
        })
    }

    /// PostgreSQLへ接続する。
    ///
    /// 返された`Client`とは別に、実際の通信を担うコネクションタスクをバックグラウンドで起動する。
    async fn connect(&self) -> Result<Client, McpError> {
        let (client, connection) = self.config.connect(NoTls).await
            .map_err(|e| McpError::internal_error(format!("DB接続に失敗しました: {e}"), None))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("DB接続エラー: {e}");
            }
        });

        Ok(client)
    }
}

#[tool_router]
impl DbSchemaServer {
    /// 指定したテーブルのスキーマ情報(カラム定義・主キー・外部キー・インデックス)を取得する。
    ///
    /// テーブルが存在しない場合は、エラーを返さず`isError: true`の`CallToolResult`を返す
    /// (呼び出し側が引数を変えてリトライできる、想定内の失敗として扱うため)。
    #[tool(description = "指定したテーブルのスキーマ情報を取得します")]
    async fn get_table_schema(
        &self,
        Parameters(GetTableSchemaRequest { schema, table }): Parameters<GetTableSchemaRequest>,
    ) -> Result<CallToolResult, McpError> {
        let schema = schema.unwrap_or_else(|| "public".to_string());
        let client = self.connect().await?;

        if !table_exists(&client, &schema, &table).await? {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "テーブル '{schema}.{table}' が見つかりませんでした"
            ))]));
        }

        let primary_key = fetch_primary_key_columns(&client, &schema, &table).await?;
        let primary_key_set: HashSet<String> = primary_key.iter().cloned().collect();
        let columns = fetch_columns(&client, &schema, &table, &primary_key_set).await?;
        let foreign_keys = fetch_foreign_keys(&client, &schema, &table).await?;
        let indexes = fetch_indexes(&client, &schema, &table).await?;

        let result = TableSchema {
            schema,
            table,
            columns,
            primary_key,
            foreign_keys,
            indexes,
        };

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(format!("結果のシリアライズに失敗しました: {e}"), None))?;

        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }
}

#[tool_handler]
impl ServerHandler for DbSchemaServer {
    /// サーバー情報・対応プロトコルバージョン・利用方法の説明を返す。
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "PostgreSQL データベースのテーブルスキーマ情報を取得するツールを提供します。\
                get_table_schema にテーブル名(と任意でスキーマ名)を指定してください。".to_string(),
            )
    }
}
