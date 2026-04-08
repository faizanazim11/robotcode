//! Tests for the `discover` module.

use robotcode_runner::discover::{discover, DiscoverArgs};
use std::path::PathBuf;

#[tokio::test]
async fn discover_nonexistent_path_returns_empty_suites() {
    let args = DiscoverArgs {
        paths: vec![PathBuf::from("/nonexistent/path/that/does/not/exist")],
        recursive: false,
    };
    let report = discover(args).await.unwrap();
    assert!(report.suites.is_empty());
}

#[tokio::test]
async fn discover_empty_paths_returns_empty_report() {
    let args = DiscoverArgs {
        paths: vec![],
        recursive: true,
    };
    let report = discover(args).await.unwrap();
    assert!(report.suites.is_empty());
}

#[tokio::test]
async fn discover_robot_file_parses_tests() {
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("example.robot");

    let mut f = std::fs::File::create(&file_path).unwrap();
    writeln!(
        f,
        "*** Test Cases ***\nMy Test\n    Log    Hello\n\nAnother Test\n    Log    World"
    )
    .unwrap();

    let args = DiscoverArgs {
        paths: vec![file_path.clone()],
        recursive: false,
    };
    let report = discover(args).await.unwrap();

    assert_eq!(report.suites.len(), 1);
    let suite = &report.suites[0];
    assert_eq!(suite.name, "Example");
    assert_eq!(suite.tests.len(), 2);
    assert_eq!(suite.tests[0].name, "My Test");
    assert_eq!(suite.tests[1].name, "Another Test");
}

#[tokio::test]
async fn discover_robot_file_with_keywords() {
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("keywords.robot");

    let mut f = std::fs::File::create(&file_path).unwrap();
    writeln!(
        f,
        "*** Keywords ***\nMy Keyword\n    Log    Hi\n\nAnother Keyword\n    Log    There"
    )
    .unwrap();

    let args = DiscoverArgs {
        paths: vec![file_path],
        recursive: false,
    };
    let report = discover(args).await.unwrap();

    assert_eq!(report.suites.len(), 1);
    let suite = &report.suites[0];
    assert_eq!(suite.keywords.len(), 2);
    assert_eq!(suite.keywords[0].name, "My Keyword");
    assert_eq!(suite.keywords[1].name, "Another Keyword");
}
