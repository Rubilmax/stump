#![cfg(feature = "postgres-tests")]
// ^ just comment out to develop i am l a z y
// if you see this please remember to uncomment it
// before committing anything

use migrations::{Migrator, MigratorTrait};
use models::{
	entity::{media, reading_session, user},
	shared::enums::FileStatus,
};
use sea_orm::{
	ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DbBackend, Set,
	Statement,
};
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

	let correction_index = Migrator::migrations()
		.iter()
		.position(|migration| {
			migration.name() == "m20260907_000000_fix_kobo_timestamp_types"
		})
		.expect("timestamp correction migration exists");
	Migrator::up(&conn, Some(correction_index as u32))
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

	Migrator::up(&conn, Some(1))
		.await
		.expect("correct timestamps before using current models");
	let session_id = seed_legacy_bookmark(&conn).await;
	Migrator::up(&conn, None)
		.await
		.expect("should have corrected timestamp types");

	let bookmark = conn
		.query_one_raw(Statement::from_string(
			DbBackend::Postgres,
			"SELECT epubcfi, session_id FROM bookmarks WHERE id = 'upgrade-bookmark'",
		))
		.await
		.unwrap()
		.unwrap();
	assert_eq!(
		bookmark.try_get::<String>("", "epubcfi").unwrap(),
		"epubcfi(/6/2)"
	);
	assert_eq!(
		bookmark.try_get::<i32>("", "session_id").unwrap(),
		session_id
	);

	for (table, column) in timestamp_columns() {
		assert_eq!(
			column_type(&conn, table, column).await,
			"timestamp with time zone"
		);
	}

	let corrected = conn
		.query_one_raw(Statement::from_string(
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
	conn.query_one_raw(Statement::from_sql_and_values(
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

async fn seed_legacy_bookmark(conn: &sea_orm::DatabaseConnection) -> i32 {
	let user = user::ActiveModel {
		username: Set("upgrade-reader".into()),
		hashed_password: Set(String::new()),
		is_server_owner: Set(true),
		is_locked: Set(false),
		..Default::default()
	}
	.insert(conn)
	.await
	.unwrap();
	let book = media::ActiveModel {
		id: Set("upgrade-book".into()),
		name: Set("Upgrade book".into()),
		extension: Set("epub".into()),
		size: Set(1234),
		pages: Set(100),
		path: Set("book.epub".into()),
		is_oneshot: Set(false),
		status: Set(FileStatus::Ready),
		..Default::default()
	}
	.insert(conn)
	.await
	.unwrap();
	let session = reading_session::ActiveModel {
		user_id: Set(user.id.clone()),
		media_id: Set(book.id.clone()),
		session_date: Set("2026-10-01".parse().unwrap()),
		readthrough_number: Set(1),
		..Default::default()
	}
	.insert(conn)
	.await
	.unwrap();
	conn.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres, "INSERT INTO bookmarks (id, media_id, user_id, created_at, epubcfi) VALUES ('upgrade-bookmark', $1, $2, '2026-10-01 12:00:00+00', 'epubcfi(/6/2)')", [book.id.into(), user.id.into()])).await.unwrap();
	session.id
}
