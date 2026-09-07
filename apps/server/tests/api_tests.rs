mod common;
mod epub;
mod graphql;
mod kobo;
mod koreader;
mod opds;
mod reading_progress;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::{json, Value};

/// server should start, the first user can register and login successfully
#[tokio::test]
async fn test_server_boots_and_auth_works() {
	let app = TestApp::new().await;
	let token = app.create_initial_account().await;
	assert!(!token.is_empty(), "expected a non-empty access token");
}

#[tokio::test]
async fn health_hides_filesystem_errors() {
	let app = TestApp::new().await;
	let response = app.server.get("/api/v2/health").await;
	response.assert_status(StatusCode::SERVICE_UNAVAILABLE);

	let payload: Value = response.json();
	assert_eq!(
		payload["dependencies"]["spa"],
		json!({"status": "error", "message": "Dependency unavailable"})
	);
}

#[tokio::test]
async fn health_hides_database_errors() {
	let app = TestApp::new().await;
	app.ctx
		.conn
		.close_by_ref()
		.await
		.expect("close test database");

	let response = app.server.get("/api/v2/health").await;
	response.assert_status(StatusCode::SERVICE_UNAVAILABLE);
	let payload: Value = response.json();
	assert_eq!(
		payload["dependencies"]["database"],
		json!({"status": "error", "message": "Dependency unavailable"})
	);
}
