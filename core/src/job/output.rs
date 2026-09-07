use std::fmt::Debug;

use async_graphql::Union;
use serde::{de, Deserialize, Serialize};

use crate::filesystem::{
	image::{PlaceholderGenerationOutput, ThumbnailGenerationOutput},
	library_file_organization::LibraryFileOrganizationOutput,
	media::analysis::AnalyzeMediaOutput,
	metadata::MetadataFetchJobOutput,
	scanner::{LibraryScanOutput, SeriesScanOutput},
};

#[derive(Debug, Clone, Serialize, Deserialize, Union)]
#[serde(untagged, rename_all = "camelCase")]
pub enum CoreJobOutput {
	LibraryScan(LibraryScanOutput),
	SeriesScan(SeriesScanOutput),
	LibraryFileOrganization(LibraryFileOrganizationOutput),
	ThumbnailGeneration(ThumbnailGenerationOutput),
	PlaceholderGeneration(PlaceholderGenerationOutput),
	MetadataFetch(MetadataFetchJobOutput),
	AnalyzeMedia(AnalyzeMediaOutput),
}

/// A trait to extend the output type for a job with a common interface. Job output starts
/// in an 'empty' state (Default) and is frequently updated during execution.
///
/// The state is also serialized and stored in the DB, so it must implement [Serialize] and [`de::DeserializeOwned`].
pub trait JobOutputExt: Serialize + de::DeserializeOwned + Debug {
	/// Update the state with new data. By default, the implementation is a full replacement
	fn update(&mut self, updated: Self) {
		*self = updated;
	}

	/// Serialize the state to JSON. If serialization fails, the error is logged and None is returned.
	fn into_json(self) -> Option<serde_json::Value> {
		serde_json::to_value(&self).map_or_else(
			|error| {
				tracing::error!(?error, job_data = ?self, "Failed to serialize job data!");
				None
			},
			Some,
		)
	}
}

#[cfg(test)]
mod tests {
	use super::{CoreJobOutput, LibraryFileOrganizationOutput};

	#[test]
	fn historical_scan_output_is_not_deserialized_as_organization_output() {
		let output = serde_json::from_value(serde_json::json!({
			"totalFiles": 1,
			"totalDirectories": 2,
			"ignoredFiles": 3,
			"skippedFiles": 4,
			"ignoredDirectories": 5,
			"createdMedia": 6,
			"updatedMedia": 7,
			"createdSeries": 8,
			"updatedSeries": 9
		}))
		.expect("library scan output should deserialize");

		assert!(matches!(output, CoreJobOutput::LibraryScan(_)));
	}

	#[test]
	fn organization_output_round_trips_to_its_variant() {
		let serialized = serde_json::to_value(CoreJobOutput::LibraryFileOrganization(
			LibraryFileOrganizationOutput {
				total_epub_files: 1,
				moved_files: 2,
				already_organized_files: 3,
				skipped_files: 4,
				conflicted_files: 5,
				failed_files: 6,
			},
		))
		.expect("organization output should serialize");
		let output = serde_json::from_value(serialized)
			.expect("organization output should deserialize");

		let CoreJobOutput::LibraryFileOrganization(output) = output else {
			panic!("organization output decoded as a different job output")
		};
		assert_eq!(output.total_epub_files, 1);
		assert_eq!(output.moved_files, 2);
		assert_eq!(output.already_organized_files, 3);
		assert_eq!(output.skipped_files, 4);
		assert_eq!(output.conflicted_files, 5);
		assert_eq!(output.failed_files, 6);
	}
}
