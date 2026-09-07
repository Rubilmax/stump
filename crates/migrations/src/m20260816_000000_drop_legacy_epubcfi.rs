use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
		// Keep the legacy values until an EPUB-aware CFI-to-Readium backfill exists.
		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		ensure_legacy_columns(manager).await
	}
}

async fn ensure_legacy_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
	if !manager.has_column("bookmarks", "epubcfi").await? {
		manager
			.alter_table(
				Table::alter()
					.table(Bookmarks::Table)
					.add_column(ColumnDef::new(Bookmarks::Epubcfi).text())
					.to_owned(),
			)
			.await?;
	}
	if !manager.has_column("reading_sessions", "epubcfi").await? {
		manager
			.alter_table(
				Table::alter()
					.table(ReadingSessions::Table)
					.add_column(ColumnDef::new(ReadingSessions::Epubcfi).text())
					.to_owned(),
			)
			.await?;
	}

	Ok(())
}

#[derive(DeriveIden)]
enum Bookmarks {
	Table,
	Epubcfi,
}

#[derive(DeriveIden)]
enum ReadingSessions {
	Table,
	Epubcfi,
}
