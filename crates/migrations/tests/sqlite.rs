use migrations::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

#[tokio::test]
async fn fork_upgrade_preserves_legacy_bookmark_positions() {
	let conn = Database::connect("sqlite::memory:").await.unwrap();
	let migrations = Migrator::migrations();
	let upgrade_index = migrations
		.iter()
		.position(|migration| {
			migration.name() == "m20260928_173434_bookmark_and_annotation_fks_to_session"
		})
		.unwrap();
	Migrator::up(&conn, Some(upgrade_index as u32))
		.await
		.unwrap();

	// Isolate bookmark preservation from unrelated foreign-key fixtures.
	conn.execute_unprepared("PRAGMA foreign_keys = OFF")
		.await
		.unwrap();
	conn.execute_unprepared(
		"INSERT INTO bookmarks (id, media_id, user_id, created_at, epubcfi) \
		 VALUES ('bookmark', 'book', 'reader', '2026-10-01 12:00:00', 'epubcfi(/6/2)')",
	)
	.await
	.unwrap();

	Migrator::up(&conn, None).await.unwrap();
	Migrator::up(&conn, None).await.unwrap();
	assert_position(&conn).await;

	Migrator::down(&conn, Some((migrations.len() - upgrade_index) as u32))
		.await
		.unwrap();
	assert_position(&conn).await;
}

async fn assert_position(conn: &impl ConnectionTrait) {
	let row = conn
		.query_one_raw(Statement::from_string(
			DbBackend::Sqlite,
			"SELECT epubcfi FROM bookmarks WHERE id = 'bookmark'",
		))
		.await
		.unwrap()
		.unwrap();
	assert_eq!(
		row.try_get::<String>("", "epubcfi").unwrap(),
		"epubcfi(/6/2)"
	);
}
