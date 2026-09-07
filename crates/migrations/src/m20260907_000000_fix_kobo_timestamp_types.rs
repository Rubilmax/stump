use sea_orm::{DbBackend, Statement, TransactionTrait};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		set_postgres_timestamp_type(manager, "TIMESTAMP WITH TIME ZONE").await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		set_postgres_timestamp_type(manager, "TIMESTAMP WITHOUT TIME ZONE").await
	}
}

async fn set_postgres_timestamp_type(
	manager: &SchemaManager<'_>,
	timestamp_type: &str,
) -> Result<(), DbErr> {
	let conn = manager.get_connection();
	if conn.get_database_backend() != DbBackend::Postgres {
		return Ok(());
	}

	let txn = conn.begin().await?;
	txn.execute(Statement::from_string(
		DbBackend::Postgres,
		"SET LOCAL TIME ZONE 'UTC'",
	))
	.await?;
	for table_columns in [
		format!(
			"ALTER TABLE reading_sessions ALTER COLUMN reported_at TYPE {timestamp_type}"
		),
		format!(
			"ALTER TABLE kobo_sync_media ALTER COLUMN updated_at TYPE {timestamp_type}"
		),
		format!(
			"ALTER TABLE reading_progress_resets \
			 ALTER COLUMN reset_at TYPE {timestamp_type}, \
			 ALTER COLUMN reported_at TYPE {timestamp_type}"
		),
	] {
		txn.execute(Statement::from_string(DbBackend::Postgres, table_columns))
			.await?;
	}
	txn.commit().await
}
