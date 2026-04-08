//! Tests for the `analyze` module.

use robotcode_analyze::analyze::{analyze, AnalyzeArgs, OutputFormat};
use std::io::Write;

#[tokio::test]
async fn analyze_empty_paths_returns_zero_files() {
    let args = AnalyzeArgs {
        paths: vec![],
        python: None,
        output_format: OutputFormat::Text,
        fail_on_error: true,
        fail_on_warning: false,
    };
    let report = analyze(args).await.unwrap();
    assert_eq!(report.files_analyzed, 0);
    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 0);
    assert_eq!(report.exit_code, 0);
}

#[tokio::test]
async fn analyze_clean_file_exit_code_zero() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("clean.robot");

    let mut f = std::fs::File::create(&file_path).unwrap();
    writeln!(f, "*** Test Cases ***\nMy Test\n    Log    Hello\n").unwrap();

    let args = AnalyzeArgs {
        paths: vec![file_path],
        python: None,
        output_format: OutputFormat::Json,
        fail_on_error: true,
        fail_on_warning: true,
    };
    let report = analyze(args).await.unwrap();
    assert_eq!(report.files_analyzed, 1);
    assert_eq!(report.exit_code, 0);
}

#[tokio::test]
async fn analyze_exit_code_no_fail_flags() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("suite.robot");
    let mut f = std::fs::File::create(&file_path).unwrap();
    writeln!(f, "*** Test Cases ***\nT\n    Log    Hi").unwrap();

    let args = AnalyzeArgs {
        paths: vec![file_path],
        python: None,
        output_format: OutputFormat::Text,
        fail_on_error: false,
        fail_on_warning: false,
    };
    let report = analyze(args).await.unwrap();
    assert_eq!(report.exit_code, 0);
}

#[tokio::test]
async fn analysis_report_json_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("roundtrip.robot");
    let mut f = std::fs::File::create(&file_path).unwrap();
    writeln!(f, "*** Test Cases ***\nT\n    Log    1").unwrap();

    let args = AnalyzeArgs {
        paths: vec![file_path],
        python: None,
        output_format: OutputFormat::Json,
        fail_on_error: false,
        fail_on_warning: false,
    };
    let report = analyze(args).await.unwrap();
    let json = serde_json::to_string(&report).unwrap();
    let _: robotcode_analyze::analyze::AnalysisReport = serde_json::from_str(&json).unwrap();
}
