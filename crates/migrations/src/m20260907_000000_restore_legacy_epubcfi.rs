use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		for (table, column) in [
			("bookmarks", LegacyColumn::Epubcfi),
			("reading_sessions", LegacyColumn::Epubcfi),
		] {
			if !manager.has_column(table, "epubcfi").await? {
				manager
					.alter_table(
						Table::alter()
							.table(Alias::new(table))
							.add_column(ColumnDef::new(column).text())
							.to_owned(),
					)
					.await?;
			}
		}

		Ok(())
	}

	async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
		// The columns may contain the only copy of a legacy reading position.
		Ok(())
	}
}

#[derive(Clone, Copy, DeriveIden)]
enum LegacyColumn {
	Epubcfi,
}

#[cfg(test)]
mod tests {
	use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

	use super::*;

	#[tokio::test]
	async fn preserves_existing_values_and_repairs_missing_columns() {
		let db = Database::connect("sqlite::memory:")
			.await
			.expect("database");
		db.execute_unprepared(
			"CREATE TABLE bookmarks (id INTEGER, epubcfi TEXT); \
			 CREATE TABLE reading_sessions (id INTEGER, epubcfi TEXT); \
			 INSERT INTO bookmarks VALUES (1, 'epubcfi(/6/2)'); \
			 INSERT INTO reading_sessions VALUES (1, 'epubcfi(/6/4)');",
		)
		.await
		.expect("seed legacy data");
		let manager = SchemaManager::new(&db);

		crate::m20260816_000000_drop_legacy_epubcfi::Migration
			.up(&manager)
			.await
			.expect("legacy migration");
		Migration.up(&manager).await.expect("repair migration");

		for (table, expected) in [
			("bookmarks", "epubcfi(/6/2)"),
			("reading_sessions", "epubcfi(/6/4)"),
		] {
			let row = db
				.query_one(Statement::from_string(
					DbBackend::Sqlite,
					format!("SELECT epubcfi FROM {table}"),
				))
				.await
				.expect("query")
				.expect("row");
			assert_eq!(row.try_get::<String>("", "epubcfi").unwrap(), expected);
		}

		db.execute_unprepared(
			"ALTER TABLE bookmarks DROP COLUMN epubcfi; \
			 ALTER TABLE reading_sessions DROP COLUMN epubcfi;",
		)
		.await
		.expect("simulate the destructive migration");
		Migration
			.up(&manager)
			.await
			.expect("repair missing columns");
		assert!(manager.has_column("bookmarks", "epubcfi").await.unwrap());
		assert!(manager
			.has_column("reading_sessions", "epubcfi")
			.await
			.unwrap());
	}
}
