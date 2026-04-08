//! Tests for argument building in runner tools.

use robotcode_runner::{libdoc::LibdocArgs, rebot::RebotArgs, run::RunArgs, testdoc::TestdocArgs};
use std::path::PathBuf;

#[test]
fn run_args_default_python() {
    let args = RunArgs {
        python: None,
        args: vec!["--version".to_owned()],
    };
    assert!(args.python.is_none());
    assert_eq!(args.args, vec!["--version"]);
}

#[test]
fn run_args_with_python() {
    let args = RunArgs {
        python: Some(PathBuf::from("/usr/bin/python3")),
        args: vec!["suite.robot".to_owned()],
    };
    assert_eq!(args.python.unwrap(), PathBuf::from("/usr/bin/python3"));
}

#[test]
fn rebot_args_clone() {
    let args = RebotArgs {
        python: None,
        args: vec!["output.xml".to_owned()],
    };
    let cloned = args.clone();
    assert_eq!(cloned.args, args.args);
}

#[test]
fn libdoc_args_clone() {
    let args = LibdocArgs {
        python: None,
        args: vec!["MyLibrary".to_owned(), "libdoc.html".to_owned()],
    };
    let cloned = args.clone();
    assert_eq!(cloned.args.len(), 2);
}

#[test]
fn testdoc_args_clone() {
    let args = TestdocArgs {
        python: None,
        args: vec!["suite.robot".to_owned(), "testdoc.html".to_owned()],
    };
    let cloned = args.clone();
    assert_eq!(cloned.args.len(), 2);
}
