use std::{
	collections::{HashMap, VecDeque},
	io::ErrorKind,
	path::{Path, PathBuf},
	sync::{Arc, Mutex},
};

use async_graphql::SimpleObject;
use models::{
	entity::{library_config, media, media_metadata, series},
	shared::enums::{FileStatus, LibraryPattern},
};
use sea_orm::{
	prelude::*, sea_query::Query, IntoActiveModel, QueryOrder, QuerySelect, Set,
	TransactionTrait,
};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{
	filesystem::series::{BuiltSeries, SeriesBuilder},
	job::{
		error::JobError, stump_job::StumpJob, JobContext, JobExecuteLog, JobLifecycle,
		JobOutputExt, JobProgress, JobStatus, JobTaskOutput, WorkingState,
	},
};

const MAX_COMPONENT_BYTES: usize = 200;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryFileOrganizationTask {
	media_id: String,
	writer: String,
	series: Option<String>,
}

/// Summary counters for a library-file organization job.
#[derive(Clone, Debug, Default, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFileOrganizationOutput {
	/// The number of ready EPUB files considered for organization.
	pub total_epub_files: u64,
	/// The number of EPUB files moved to their organized location.
	pub moved_files: u64,
	/// The number of EPUB files already at their organized location.
	pub already_organized_files: u64,
	/// The number of EPUB files without usable metadata.
	pub skipped_files: u64,
	/// The number of EPUB files whose destination already existed.
	pub conflicted_files: u64,
	/// The number of EPUB files that could not be organized.
	pub failed_files: u64,
}

impl JobOutputExt for LibraryFileOrganizationOutput {
	fn update(&mut self, updated: Self) {
		self.total_epub_files += updated.total_epub_files;
		self.moved_files += updated.moved_files;
		self.already_organized_files += updated.already_organized_files;
		self.skipped_files += updated.skipped_files;
		self.conflicted_files += updated.conflicted_files;
		self.failed_files += updated.failed_files;
	}
}

#[derive(Clone)]
pub struct LibraryFileOrganizationJob {
	pub id: String,
	pub path: String,
	pattern: Option<LibraryPattern>,
	canonical_root: Option<PathBuf>,
	series_id_by_path: Arc<Mutex<HashMap<PathBuf, String>>>,
}

impl LibraryFileOrganizationJob {
	pub fn new(id: String, path: String) -> Self {
		Self {
			id,
			path,
			pattern: None,
			canonical_root: None,
			series_id_by_path: Arc::new(Mutex::new(HashMap::new())),
		}
	}

	fn target_directory(&self, task: &LibraryFileOrganizationTask) -> PathBuf {
		target_directory(Path::new(&self.path), &task.writer, task.series.as_deref())
	}

	fn owner_directory(&self, task: &LibraryFileOrganizationTask) -> PathBuf {
		owner_directory(
			Path::new(&self.path),
			self.pattern.unwrap_or_default(),
			&task.writer,
			task.series.as_deref(),
		)
	}

	fn failed(path: &str, message: impl Into<String>) -> JobTaskOutput<Self> {
		let message = message.into();
		tracing::error!(path, %message, "Failed to organize library file");
		JobTaskOutput {
			output: LibraryFileOrganizationOutput {
				failed_files: 1,
				..Default::default()
			},
			subtasks: vec![],
			logs: vec![JobExecuteLog::error(message).with_ctx(path.to_string())],
		}
	}

	fn conflict(path: &str, target: &Path) -> JobTaskOutput<Self> {
		let message = format!(
			"Refused to overwrite an existing file at {}",
			target.display()
		);
		tracing::warn!(path, target = ?target, "Library file organization conflict");
		JobTaskOutput {
			output: LibraryFileOrganizationOutput {
				conflicted_files: 1,
				..Default::default()
			},
			subtasks: vec![],
			logs: vec![JobExecuteLog::warn(&message).with_ctx(path.to_string())],
		}
	}

	async fn update_media_location(
		&self,
		ctx: &JobContext,
		book: &media::Model,
		target: &Path,
		owner_directory: &Path,
	) -> Result<(), DbErr> {
		let known_owner_id = self
			.series_id_by_path
			.lock()
			.ok()
			.and_then(|owners| owners.get(owner_directory).cloned());
		let owner_id = update_media_location(
			ctx,
			book,
			&self.id,
			target,
			owner_directory,
			known_owner_id.as_deref(),
		)
		.await?;

		if let Ok(mut owners) = self.series_id_by_path.lock() {
			owners.insert(owner_directory.to_path_buf(), owner_id);
		}
		Ok(())
	}
}

#[async_trait::async_trait]
impl JobLifecycle for LibraryFileOrganizationJob {
	const NAME: &'static str = "library_file_organization";

	type Output = LibraryFileOrganizationOutput;
	type Task = LibraryFileOrganizationTask;

	fn description(&self) -> Option<String> {
		Some(self.path.clone())
	}

	async fn init(
		&mut self,
		ctx: &JobContext,
	) -> Result<WorkingState<Self::Output, Self::Task>, JobError> {
		let config = library_config::Entity::find()
			.filter(library_config::Column::LibraryId.eq(&self.id))
			.one(ctx.conn())
			.await?
			.ok_or_else(|| {
				JobError::InitFailed("Library is missing configuration".to_string())
			})?;

		let root = PathBuf::from(&self.path);
		let canonical_root = fs::canonicalize(&root).await.map_err(|error| {
			JobError::InitFailed(format!(
				"Could not resolve library root {}: {error}",
				root.display()
			))
		})?;
		let root_metadata = fs::metadata(&canonical_root).await.map_err(|error| {
			JobError::InitFailed(format!(
				"Could not inspect library root {}: {error}",
				root.display()
			))
		})?;
		if !root_metadata.is_dir() {
			return Err(JobError::InitFailed(format!(
				"Library root is not a directory: {}",
				root.display()
			)));
		}

		self.pattern = Some(config.library_pattern);
		self.canonical_root = Some(canonical_root);

		ctx.report_progress(JobProgress::msg("Finding EPUB files with metadata"));
		let media_with_metadata = media::Entity::find()
			.filter(media::Column::DeletedAt.is_null())
			.filter(media::Column::Status.eq(FileStatus::Ready))
			.filter(media::Entity::epub_filter())
			.filter(
				media::Column::SeriesId.in_subquery(
					Query::select()
						.column(series::Column::Id)
						.from(series::Entity)
						.and_where(series::Column::LibraryId.eq(&self.id))
						.to_owned(),
				),
			)
			.left_join(media_metadata::Entity)
			.select_only()
			.column(media::Column::Id)
			.column(media_metadata::Column::Writers)
			.column(media_metadata::Column::Series)
			.order_by_asc(media::Column::Path)
			.into_tuple::<(String, Option<String>, Option<String>)>()
			.all(ctx.conn())
			.await?;

		let mut output = LibraryFileOrganizationOutput {
			total_epub_files: media_with_metadata.len() as u64,
			..Default::default()
		};
		let mut tasks = VecDeque::new();

		for (media_id, writers, series) in media_with_metadata {
			let task = writers
				.as_deref()
				.and_then(|writers| writers.split(',').find_map(sanitize_component))
				.map(|writer| LibraryFileOrganizationTask {
					media_id,
					writer,
					series: series.as_deref().and_then(sanitize_component),
				});

			if let Some(task) = task {
				tasks.push_back(task);
			} else {
				output.skipped_files += 1;
			}
		}
		let logs = (output.skipped_files > 0)
			.then(|| {
				JobExecuteLog::warn(&format!(
					"Skipped {} EPUB files without valid media metadata and a writer",
					output.skipped_files
				))
			})
			.into_iter()
			.collect();

		ctx.report_progress(JobProgress::msg(&format!(
			"Found {} EPUB files to organize",
			tasks.len()
		)));

		Ok(WorkingState {
			output: Some(output),
			tasks,
			logs,
		})
	}

	async fn execute_task(
		&self,
		ctx: &JobContext,
		task: Self::Task,
	) -> Result<JobTaskOutput<Self>, JobError> {
		let Some(canonical_root) = self.canonical_root.as_deref() else {
			return Err(JobError::TaskFailed(
				"Library root was not initialized".to_string(),
			));
		};

		let Some(book) = media::Entity::find_by_id(&task.media_id)
			.filter(media::Column::DeletedAt.is_null())
			.filter(media::Column::Status.eq(FileStatus::Ready))
			.filter(media::Entity::epub_filter())
			.filter(
				media::Column::SeriesId.in_subquery(
					Query::select()
						.column(series::Column::Id)
						.from(series::Entity)
						.and_where(series::Column::LibraryId.eq(&self.id))
						.to_owned(),
				),
			)
			.one(ctx.conn())
			.await?
		else {
			return Ok(Self::failed(
				&task.media_id,
				"Media record disappeared before it could be organized",
			));
		};

		let task_is_sanitized = sanitize_component(&task.writer).as_deref()
			== Some(task.writer.as_str())
			&& task.series.as_deref().is_none_or(|series_name| {
				sanitize_component(series_name).as_deref() == Some(series_name)
			});
		if !task_is_sanitized {
			return Err(JobError::TaskFailed(
				"Organization task contains an unsafe path component".to_string(),
			));
		}

		let source = PathBuf::from(&book.path);
		let source_display = source.display().to_string();
		match fs::symlink_metadata(&source).await {
			Ok(metadata) if metadata.file_type().is_symlink() => {
				return Ok(Self::failed(
					&source_display,
					"Source path is a symbolic link",
				));
			},
			Ok(metadata) if !metadata.is_file() => {
				return Ok(Self::failed(
					&source_display,
					"Source path is not a regular file",
				));
			},
			Ok(_) => {},
			Err(error) => {
				return Ok(Self::failed(
					&source_display,
					format!("Could not inspect source file: {error}"),
				));
			},
		}
		let canonical_source = match fs::canonicalize(&source).await {
			Ok(path) if path.starts_with(canonical_root) => path,
			Ok(_) => {
				return Ok(Self::failed(
					&source_display,
					"Source file resolves outside the library root",
				));
			},
			Err(error) => {
				return Ok(Self::failed(
					&source_display,
					format!("Could not resolve source file: {error}"),
				));
			},
		};
		let Some(filename) = source.file_name() else {
			return Ok(Self::failed(&source_display, "Source path has no filename"));
		};
		let target_directory = self.target_directory(&task);
		let target = target_directory.join(filename);
		let owner_directory = self.owner_directory(&task);

		let already_organized = source == target;
		if !already_organized {
			match fs::symlink_metadata(&target).await {
				Ok(_) => return Ok(Self::conflict(&source_display, &target)),
				Err(error) if error.kind() == ErrorKind::NotFound => {},
				Err(error) => {
					return Ok(Self::failed(
						&source_display,
						format!("Could not inspect destination: {error}"),
					));
				},
			}
		}

		if let Err(error) = ensure_contained_directory(
			Path::new(&self.path),
			canonical_root,
			&task.writer,
			task.series.as_deref(),
		)
		.await
		{
			return Ok(Self::failed(
				&source_display,
				format!("Could not prepare destination directory: {error}"),
			));
		}

		if already_organized {
			return match self
				.update_media_location(ctx, &book, &target, &owner_directory)
				.await
			{
				Ok(()) => Ok(JobTaskOutput {
					output: LibraryFileOrganizationOutput {
						already_organized_files: 1,
						..Default::default()
					},
					subtasks: vec![],
					logs: vec![],
				}),
				Err(error) => Ok(Self::failed(
					&source_display,
					format!("Could not update organized media record: {error}"),
				)),
			};
		}

		if let Err(error) = rename_without_overwrite(&source, &target).await {
			return if error.kind() == ErrorKind::AlreadyExists {
				Ok(Self::conflict(&source_display, &target))
			} else {
				Ok(Self::failed(
					&source_display,
					format!("Could not move file to {}: {error}", target.display()),
				))
			};
		}

		if let Err(database_error) = self
			.update_media_location(ctx, &book, &target, &owner_directory)
			.await
		{
			return match rename_without_overwrite(&target, &source).await {
				Ok(()) => {
					Ok(Self::failed(
						&source_display,
						format!(
							"Database update failed; filesystem move was rolled back: {database_error}"
						),
					))
				},
				Err(rollback_error) => Err(JobError::TaskFailed(format!(
					"Database update failed for {} ({database_error}) and the move from {} could not be rolled back: {rollback_error}",
					source.display(),
					target.display(),
				))),
			};
		}

		if let Some(source_parent) = canonical_source.parent() {
			remove_empty_ancestors(source_parent, canonical_root).await;
		}

		Ok(JobTaskOutput {
			output: LibraryFileOrganizationOutput {
				moved_files: 1,
				..Default::default()
			},
			subtasks: vec![],
			logs: vec![],
		})
	}

	async fn finalize(
		&self,
		ctx: &JobContext,
		_output: &Self::Output,
	) -> Result<(), JobError> {
		if let Err(error) = ctx
			.enqueue(StumpJob::library_scan(
				self.id.clone(),
				self.path.clone(),
				None,
			))
			.await
		{
			ctx.fail(JobStatus::Failed, &format!("Finalization failed: {error}"))
				.await?;
			return Err(error);
		}
		Ok(())
	}
}

async fn rename_without_overwrite(source: &Path, target: &Path) -> std::io::Result<()> {
	let source = source.to_path_buf();
	let target = target.to_path_buf();
	tokio::task::spawn_blocking(move || {
		rename_without_overwrite_blocking(&source, &target)
	})
	.await
	.map_err(std::io::Error::other)?
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn c_path(path: &Path) -> std::io::Result<std::ffi::CString> {
	use std::os::unix::ffi::OsStrExt;

	std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| {
		std::io::Error::new(
			ErrorKind::InvalidInput,
			format!("Path contains a null byte: {}", path.display()),
		)
	})
}

#[cfg(target_os = "linux")]
fn rename_without_overwrite_blocking(
	source: &Path,
	target: &Path,
) -> std::io::Result<()> {
	let source = c_path(source)?;
	let target = c_path(target)?;
	let result = unsafe {
		libc::syscall(
			libc::SYS_renameat2,
			libc::AT_FDCWD,
			source.as_ptr(),
			libc::AT_FDCWD,
			target.as_ptr(),
			libc::RENAME_NOREPLACE,
		)
	};
	if result == 0 {
		Ok(())
	} else {
		Err(std::io::Error::last_os_error())
	}
}

#[cfg(target_os = "macos")]
fn rename_without_overwrite_blocking(
	source: &Path,
	target: &Path,
) -> std::io::Result<()> {
	let source = c_path(source)?;
	let target = c_path(target)?;
	let result =
		unsafe { libc::renamex_np(source.as_ptr(), target.as_ptr(), libc::RENAME_EXCL) };
	if result == 0 {
		Ok(())
	} else {
		Err(std::io::Error::last_os_error())
	}
}

#[cfg(target_family = "windows")]
fn rename_without_overwrite_blocking(
	source: &Path,
	target: &Path,
) -> std::io::Result<()> {
	use std::os::windows::ffi::OsStrExt;

	fn wide_path(path: &Path) -> std::io::Result<Vec<u16>> {
		let mut value = path.as_os_str().encode_wide().collect::<Vec<_>>();
		if value.contains(&0) {
			return Err(std::io::Error::new(
				ErrorKind::InvalidInput,
				format!("Path contains a null byte: {}", path.display()),
			));
		}
		value.push(0);
		Ok(value)
	}

	#[link(name = "Kernel32")]
	unsafe extern "system" {
		fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
	}

	let source = wide_path(source)?;
	let target = wide_path(target)?;
	let result = unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), 0) };
	if result != 0 {
		Ok(())
	} else {
		let error = std::io::Error::last_os_error();
		if matches!(error.raw_os_error(), Some(80) | Some(183)) {
			Err(std::io::Error::new(ErrorKind::AlreadyExists, error))
		} else {
			Err(error)
		}
	}
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_family = "windows")))]
fn rename_without_overwrite_blocking(
	_source: &Path,
	_target: &Path,
) -> std::io::Result<()> {
	Err(std::io::Error::new(
		ErrorKind::Unsupported,
		"Atomic no-overwrite moves are unsupported on this platform",
	))
}

fn target_directory(root: &Path, writer: &str, series: Option<&str>) -> PathBuf {
	let author_directory = root.join(writer);
	series.map_or(author_directory.clone(), |series_name| {
		author_directory.join(series_name)
	})
}

fn owner_directory(
	root: &Path,
	pattern: LibraryPattern,
	writer: &str,
	series: Option<&str>,
) -> PathBuf {
	let author_directory = root.join(writer);
	match (pattern, series) {
		(LibraryPattern::SeriesBased, Some(series_name)) => {
			author_directory.join(series_name)
		},
		_ => author_directory,
	}
}

fn sanitize_component(value: &str) -> Option<String> {
	let mut sanitized = value
		.trim()
		.chars()
		.map(|character| {
			if character.is_control()
				|| matches!(
					character,
					'/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*'
				) {
				'_'
			} else {
				character
			}
		})
		.collect::<String>();

	while sanitized.ends_with([' ', '.']) {
		sanitized.pop();
	}
	if sanitized.is_empty() || matches!(sanitized.as_str(), "." | "..") {
		return None;
	}

	let mut end = sanitized.len().min(MAX_COMPONENT_BYTES);
	while !sanitized.is_char_boundary(end) {
		end -= 1;
	}
	sanitized.truncate(end);
	while sanitized.ends_with([' ', '.']) {
		sanitized.pop();
	}
	if sanitized.is_empty() {
		return None;
	}

	let reserved_stem = sanitized
		.split('.')
		.next()
		.unwrap_or_default()
		.to_ascii_uppercase();
	let is_reserved = matches!(
		reserved_stem.as_str(),
		"CON"
			| "PRN" | "AUX"
			| "NUL" | "COM1"
			| "COM2" | "COM3"
			| "COM4" | "COM5"
			| "COM6" | "COM7"
			| "COM8" | "COM9"
			| "LPT1" | "LPT2"
			| "LPT3" | "LPT4"
			| "LPT5" | "LPT6"
			| "LPT7" | "LPT8"
			| "LPT9"
	);
	if is_reserved || sanitized.starts_with('.') {
		sanitized.insert(0, '_');
		let mut end = sanitized.len().min(MAX_COMPONENT_BYTES);
		while !sanitized.is_char_boundary(end) {
			end -= 1;
		}
		sanitized.truncate(end);
	}

	Some(sanitized)
}

async fn ensure_contained_directory(
	root: &Path,
	canonical_root: &Path,
	writer: &str,
	series: Option<&str>,
) -> std::io::Result<()> {
	let mut current = root.to_path_buf();
	for component in std::iter::once(writer).chain(series) {
		current.push(component);
		match fs::symlink_metadata(&current).await {
			Ok(metadata) if metadata.file_type().is_symlink() => {
				return Err(std::io::Error::new(
					ErrorKind::PermissionDenied,
					format!(
						"Destination directory is a symbolic link: {}",
						current.display()
					),
				));
			},
			Ok(metadata) if !metadata.is_dir() => {
				return Err(std::io::Error::new(
					ErrorKind::NotADirectory,
					format!("Destination path is not a directory: {}", current.display()),
				));
			},
			Ok(_) => {},
			Err(error) if error.kind() == ErrorKind::NotFound => {
				fs::create_dir(&current).await?;
			},
			Err(error) => return Err(error),
		}

		let resolved = fs::canonicalize(&current).await?;
		let metadata = fs::metadata(&resolved).await?;
		if !resolved.starts_with(canonical_root) || !metadata.is_dir() {
			return Err(std::io::Error::new(
				ErrorKind::PermissionDenied,
				format!(
					"Destination directory escapes library root: {}",
					current.display()
				),
			));
		}
	}
	Ok(())
}

async fn update_media_location(
	ctx: &JobContext,
	book: &media::Model,
	library_id: &str,
	target: &Path,
	owner_directory: &Path,
	known_owner_id: Option<&str>,
) -> Result<String, DbErr> {
	let target = target.to_str().ok_or_else(|| {
		DbErr::Custom("Destination path is not valid UTF-8".to_string())
	})?;
	let owner_path = owner_directory
		.to_str()
		.ok_or_else(|| DbErr::Custom("Series path is not valid UTF-8".to_string()))?;
	let txn = ctx.conn().begin().await?;

	let result = async {
		let owner = if known_owner_id.is_none() {
			series::Entity::find()
				.filter(series::Column::LibraryId.eq(library_id))
				.filter(series::Column::Path.eq(owner_path))
				.order_by_asc(series::Column::CreatedAt)
				.one(&txn)
				.await?
		} else {
			None
		};

		let owner_id = if let Some(owner_id) = known_owner_id {
			owner_id.to_string()
		} else if let Some(owner) = owner {
			let owner_id = owner.id.clone();
			if owner.status != FileStatus::Ready || owner.deleted_at.is_some() {
				let mut active = owner.into_active_model();
				active.status = Set(FileStatus::Ready);
				active.deleted_at = Set(None);
				active.update(&txn).await?;
			}
			owner_id
		} else {
			let BuiltSeries {
				series: active,
				metadata,
			} = SeriesBuilder::new(owner_directory, library_id)
				.build()
				.map_err(|error| DbErr::Custom(error.to_string()))?;
			let inserted = active.insert(&txn).await?;
			if let Some(mut metadata) = metadata {
				metadata.series_id = Set(inserted.id.clone());
				metadata.insert(&txn).await?;
			}
			inserted.id
		};

		let mut active = book.clone().into_active_model();
		active.path = Set(target.to_string());
		active.series_id = Set(Some(owner_id.clone()));
		active.status = Set(FileStatus::Ready);
		active.update(&txn).await?;
		Ok::<String, DbErr>(owner_id)
	}
	.await;

	match result {
		Ok(owner_id) => {
			txn.commit().await?;
			Ok(owner_id)
		},
		Err(error) => {
			if let Err(rollback_error) = txn.rollback().await {
				tracing::error!(
					?rollback_error,
					"Failed to roll back organization database transaction"
				);
			}
			Err(error)
		},
	}
}

async fn remove_empty_ancestors(start: &Path, root: &Path) {
	let mut current = start.to_path_buf();
	while current != root && current.starts_with(root) {
		match fs::remove_dir(&current).await {
			Ok(()) => {},
			Err(error) if error.kind() == ErrorKind::NotFound => {},
			Err(error) if error.kind() == ErrorKind::DirectoryNotEmpty => break,
			Err(error) => {
				tracing::warn!(path = ?current, ?error, "Failed to remove empty source directory");
				break;
			},
		}

		let Some(parent) = current.parent() else {
			break;
		};
		current = parent.to_path_buf();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Ctx;
	use ::tests::{db::test_database, fake_data};
	use models::entity::{job, log};
	use sea_orm::{
		ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
		Schema,
	};

	async fn create_job_test_tables(db: &DatabaseConnection) {
		let schema = Schema::new(DbBackend::Sqlite);
		for statement in [
			schema.create_table_from_entity(job::Entity),
			schema.create_table_from_entity(log::Entity),
		] {
			db.execute(db.get_database_backend().build(&statement))
				.await
				.unwrap();
		}
	}

	async fn insert_book(
		db: &DatabaseConnection,
		id: &str,
		series_id: &str,
		path: &Path,
	) {
		let book = fake_data::Media {
			id: Some(id.to_string()),
			name: Some(id.to_string()),
			extension: Some("epub".to_string()),
			series_id: series_id.to_string(),
			..Default::default()
		}
		.insert(db)
		.await;
		let mut active = book.into_active_model();
		active.path = Set(path.to_string_lossy().into_owned());
		active.update(db).await.unwrap();

		media_metadata::ActiveModel {
			media_id: Set(Some(id.to_string())),
			writers: Set(Some(
				"Writer, Coauthor One, Coauthor Two, Coauthor Three".to_string(),
			)),
			series: Set(Some("Series".to_string())),
			..Default::default()
		}
		.insert(db)
		.await
		.unwrap();
	}

	#[test]
	fn sanitizes_untrusted_path_components() {
		assert_eq!(
			sanitize_component("  Jane/Smith:Writer.  "),
			Some("Jane_Smith_Writer".to_string())
		);
		assert_eq!(sanitize_component("CON"), Some("_CON".to_string()));
		assert_eq!(sanitize_component(".."), None);
		assert_eq!(sanitize_component(".hidden"), Some("_.hidden".to_string()));
	}

	#[test]
	fn limits_components_without_splitting_unicode() {
		let sanitized = sanitize_component(&"é".repeat(150)).unwrap();
		assert!(sanitized.len() <= MAX_COMPONENT_BYTES);
		assert!(sanitized.is_char_boundary(sanitized.len()));

		let hidden = sanitize_component(&format!(".{}", "é".repeat(150))).unwrap();
		assert!(hidden.len() <= MAX_COMPONENT_BYTES);
		assert!(hidden.is_char_boundary(hidden.len()));
	}

	#[test]
	fn builds_author_and_optional_series_directories() {
		let root = Path::new("/library");
		assert_eq!(target_directory(root, "Author", None), root.join("Author"));
		assert_eq!(
			target_directory(root, "Author", Some("Series")),
			root.join("Author").join("Series")
		);
	}

	#[test]
	fn chooses_database_owner_for_each_library_pattern() {
		let root = Path::new("/library");
		assert_eq!(
			owner_directory(root, LibraryPattern::SeriesBased, "Author", Some("Series")),
			root.join("Author").join("Series")
		);
		assert_eq!(
			owner_directory(
				root,
				LibraryPattern::CollectionBased,
				"Author",
				Some("Series")
			),
			root.join("Author")
		);
	}

	#[tokio::test]
	async fn removes_only_empty_ancestors_below_root() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().join("library");
		let author = root.join("Author");
		let old_series = author.join("Old Series");
		fs::create_dir_all(&old_series).await.unwrap();
		fs::write(author.join("keep.txt"), b"keep").await.unwrap();

		remove_empty_ancestors(&old_series, &root).await;

		assert!(!old_series.exists());
		assert!(author.exists());
		assert!(root.exists());
	}

	#[tokio::test]
	async fn moves_without_overwriting_an_existing_file() {
		let temp = tempfile::tempdir().unwrap();
		let source = temp.path().join("source.epub");
		let target = temp.path().join("target.epub");
		fs::write(&source, b"source").await.unwrap();
		fs::write(&target, b"target").await.unwrap();

		let error = rename_without_overwrite(&source, &target)
			.await
			.unwrap_err();
		assert_eq!(error.kind(), ErrorKind::AlreadyExists);
		assert_eq!(fs::read(&source).await.unwrap(), b"source");
		assert_eq!(fs::read(&target).await.unwrap(), b"target");

		fs::remove_file(&target).await.unwrap();
		rename_without_overwrite(&source, &target).await.unwrap();
		assert!(!source.exists());
		assert_eq!(fs::read(&target).await.unwrap(), b"source");
	}

	#[tokio::test]
	async fn organizes_applied_metadata_with_multiple_writers_without_overwriting_conflicts(
	) {
		let db = test_database().await;
		create_job_test_tables(&db).await;
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().join("library");
		let incoming = root.join("Incoming");
		let destination = root.join("Writer").join("Series");
		fs::create_dir_all(&incoming).await.unwrap();
		fs::create_dir_all(&destination).await.unwrap();

		let move_source = incoming.join("move.epub");
		let conflict_source = incoming.join("conflict.epub");
		let move_target = destination.join("move.epub");
		let conflict_target = destination.join("conflict.epub");
		fs::write(&move_source, b"move source").await.unwrap();
		fs::write(&conflict_source, b"conflict source")
			.await
			.unwrap();
		fs::write(&conflict_target, b"existing target")
			.await
			.unwrap();

		let library = fake_data::Library {
			id: Some("library".to_string()),
			path: Some(root.to_string_lossy().into_owned()),
			..Default::default()
		}
		.insert(&db)
		.await;
		let incoming_series = fake_data::Series {
			id: Some("incoming-series".to_string()),
			path: Some(incoming.to_string_lossy().into_owned()),
			library_id: Some(library.id.clone()),
			..Default::default()
		}
		.insert(&db)
		.await;
		insert_book(&db, "move", &incoming_series.id, &move_source).await;
		insert_book(&db, "conflict", &incoming_series.id, &conflict_source).await;

		let stump_job = StumpJob::library_file_organization(
			library.id.clone(),
			root.to_string_lossy().into_owned(),
		);
		let core = Ctx::for_testing(db);
		let ctx = JobContext::new(
			core.apalis_state.clone(),
			"organization-job".to_string(),
			&stump_job,
		)
		.await
		.unwrap();
		let mut organizer = LibraryFileOrganizationJob::new(
			library.id,
			root.to_string_lossy().into_owned(),
		);
		let working = organizer.init(&ctx).await.unwrap();
		let mut output = working.output.unwrap();
		let mut tasks = working.tasks;
		while let Some(task) = tasks.pop_front() {
			output.update(organizer.execute_task(&ctx, task).await.unwrap().output);
		}
		organizer.finalize(&ctx, &output).await.unwrap();

		assert_eq!(output.total_epub_files, 2);
		assert_eq!(output.moved_files, 1);
		assert_eq!(output.conflicted_files, 1);
		assert!(!move_source.exists());
		assert_eq!(fs::read(&move_target).await.unwrap(), b"move source");
		assert_eq!(
			fs::read(&conflict_source).await.unwrap(),
			b"conflict source"
		);
		assert_eq!(
			fs::read(&conflict_target).await.unwrap(),
			b"existing target"
		);

		let moved = media::Entity::find_by_id("move")
			.one(ctx.conn())
			.await
			.unwrap()
			.unwrap();
		assert_eq!(moved.path, move_target.to_string_lossy().into_owned());
		let owner = series::Entity::find_by_id(moved.series_id.unwrap())
			.one(ctx.conn())
			.await
			.unwrap()
			.unwrap();
		assert_eq!(owner.path, destination.to_string_lossy().into_owned());

		let conflicted = media::Entity::find_by_id("conflict")
			.one(ctx.conn())
			.await
			.unwrap()
			.unwrap();
		assert_eq!(
			conflicted.path,
			conflict_source.to_string_lossy().into_owned()
		);
		assert_eq!(conflicted.series_id.as_deref(), Some("incoming-series"));
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn rejects_symlinked_destination_directories() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().join("library");
		let target = root.join("target");
		fs::create_dir_all(&target).await.unwrap();
		symlink(&target, root.join("Author")).unwrap();

		let error = ensure_contained_directory(&root, &root, "Author", None)
			.await
			.unwrap_err();

		assert_eq!(error.kind(), ErrorKind::PermissionDenied);
	}
}
