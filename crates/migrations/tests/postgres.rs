#![cfg(feature = "postgres-tests")]
// ^ just comment out to develop i am l a z y
// if you see this please remember to uncomment it
// before committing anything

use migrations::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DbBackend, Statement};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
async fn migrations_run_on_postgres() {
	let container = Postgres::default()
		.start()
		.await
		.expect("should have started postgres container");

	let url = format!(
		"postgres://postgres:postgres@{}:{}/postgres",
		container.get_host().await.expect("should have a host"),
		container
			.get_host_port_ipv4(5432)
			.await
			.expect("should have a host port")
	);

	let mut options = ConnectOptions::new(url);
	options.max_connections(1);
	let conn = Database::connect(options)
		.await
		.expect("should have connected to postgres");

	let migration_count = Migrator::migrations().len();
	Migrator::up(&conn, Some((migration_count - 1) as u32))
		.await
		.expect("should have run the pre-correction migrations");

	for (table, column) in timestamp_columns() {
		assert_eq!(
			column_type(&conn, table, column).await,
			"timestamp without time zone"
		);
	}

	conn.execute_unprepared("SET TIME ZONE 'America/Los_Angeles'")
		.await
		.expect("set non-UTC session timezone");
	conn.execute_unprepared("ALTER TABLE kobo_sync_media DISABLE TRIGGER ALL")
		.await
		.expect("disable foreign-key triggers for fixture");
	conn.execute_unprepared(
		"INSERT INTO kobo_sync_media (user_id, media_id, updated_at) \
		 VALUES ('test-user', 'test-media', TIMESTAMP '2026-01-02 03:04:05')",
	)
	.await
	.expect("insert pre-correction timestamp");
	conn.execute_unprepared("ALTER TABLE kobo_sync_media ENABLE TRIGGER ALL")
		.await
		.expect("restore foreign-key triggers");

	Migrator::up(&conn, None)
		.await
		.expect("should have corrected timestamp types");

	for (table, column) in timestamp_columns() {
		assert_eq!(
			column_type(&conn, table, column).await,
			"timestamp with time zone"
		);
	}

	let corrected = conn
		.query_one(Statement::from_string(
			DbBackend::Postgres,
			"SELECT to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS') AS value \
			 FROM kobo_sync_media",
		))
		.await
		.expect("corrected timestamp query")
		.expect("timestamp row")
		.try_get::<String>("", "value")
		.expect("timestamp value");
	assert_eq!(corrected, "2026-01-02 03:04:05");
}

fn timestamp_columns() -> [(&'static str, &'static str); 4] {
	[
		("reading_sessions", "reported_at"),
		("kobo_sync_media", "updated_at"),
		("reading_progress_resets", "reset_at"),
		("reading_progress_resets", "reported_at"),
	]
}

async fn column_type(
	conn: &sea_orm::DatabaseConnection,
	table: &str,
	column: &str,
) -> String {
	conn.query_one(Statement::from_sql_and_values(
		DbBackend::Postgres,
		"SELECT data_type FROM information_schema.columns \
		 WHERE table_schema = 'public' AND table_name = $1 AND column_name = $2",
		[table.into(), column.into()],
	))
	.await
	.expect("column type query")
	.expect("column")
	.try_get("", "data_type")
	.expect("data type")
}
