//! Snapshot tests for the Robot Framework parser.

use robotcode_rf_parser::parser::parse;

#[test]
fn snapshot_parse_simple() {
    let source = r#"*** Test Cases ***
My Test
    Log    Hello World
    ${result}=    Evaluate    1 + 2
"#;
    let file = parse(source);
    insta::assert_json_snapshot!(file);
}

#[test]
fn snapshot_parse_variables() {
    let source = r#"*** Variables ***
${SCALAR}    hello
@{LIST}      a    b    c
&{DICT}      key=value    other=stuff
"#;
    let file = parse(source);
    insta::assert_json_snapshot!(file);
}

#[test]
fn snapshot_parse_settings() {
    let source = r#"*** Settings ***
Library    Collections
Library    String    WITH NAME    Str
Resource    helpers.robot
Documentation    Suite documentation
...    continued on next line
Suite Setup    Log    starting
Test Tags    smoke    regression
"#;
    let file = parse(source);
    insta::assert_json_snapshot!(file);
}

#[test]
fn snapshot_parse_keywords() {
    let source = r#"*** Keywords ***
My Keyword
    [Arguments]    ${arg1}    ${arg2}=default
    [Documentation]    Does something
    [Tags]    kw_tag
    Log    ${arg1}
    RETURN    ${arg2}

Embedded ${value} Keyword
    Log    ${value}
"#;
    let file = parse(source);
    insta::assert_json_snapshot!(file);
}

#[test]
fn snapshot_parse_control_flow() {
    let source = r#"*** Test Cases ***
Control Flow Test
    FOR    ${item}    IN    a    b    c
        Log    ${item}
    END
    IF    ${condition}
        Log    true branch
    ELSE IF    ${other}
        Log    else if branch
    ELSE
        Log    else branch
    END
    TRY
        Fail    intentional
    EXCEPT    intentional    type=LITERAL
        Log    caught
    FINALLY
        Log    always runs
    END
    WHILE    ${x} < 10
        BREAK
    END
    RETURN    done
"#;
    let file = parse(source);
    insta::assert_json_snapshot!(file);
}
