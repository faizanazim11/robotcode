//! Generic visitor trait for the Robot Framework AST.
//!
//! Default implementations recursively visit all children.  Override only the
//! methods you care about.

use crate::parser::ast::*;

/// Visitor over the RF AST.  All methods have no-op or recursive defaults.
pub trait AstVisitor {
    fn visit_file(&mut self, node: &File) {
        for section in &node.sections {
            self.visit_section(section);
        }
    }

    fn visit_section(&mut self, node: &Section) {
        match node {
            Section::Settings(s) => self.visit_settings_section(s),
            Section::Variables(s) => self.visit_variables_section(s),
            Section::TestCases(s) => self.visit_test_cases_section(s),
            Section::Tasks(s) => self.visit_tasks_section(s),
            Section::Keywords(s) => self.visit_keywords_section(s),
            Section::Comments(s) => self.visit_comments_section(s),
            Section::Invalid(_) => {}
        }
    }

    fn visit_settings_section(&mut self, node: &SettingsSection) {
        for item in &node.body {
            self.visit_setting_item(item);
        }
    }

    fn visit_setting_item(&mut self, _node: &SettingItem) {}

    fn visit_variables_section(&mut self, node: &VariablesSection) {
        for item in &node.body {
            self.visit_variable_item(item);
        }
    }

    fn visit_variable_item(&mut self, _node: &VariableItem) {}

    fn visit_test_cases_section(&mut self, node: &TestCasesSection) {
        for tc in &node.body {
            self.visit_test_case(tc);
        }
    }

    fn visit_test_case(&mut self, node: &TestCase) {
        for item in &node.body {
            self.visit_body_item(item);
        }
    }

    fn visit_tasks_section(&mut self, node: &TasksSection) {
        for task in &node.body {
            self.visit_task(task);
        }
    }

    fn visit_task(&mut self, node: &Task) {
        for item in &node.body {
            self.visit_body_item(item);
        }
    }

    fn visit_keywords_section(&mut self, node: &KeywordsSection) {
        for kw in &node.body {
            self.visit_keyword(kw);
        }
    }

    fn visit_keyword(&mut self, node: &Keyword) {
        for item in &node.body {
            self.visit_body_item(item);
        }
    }

    fn visit_comments_section(&mut self, _node: &CommentsSection) {}

    fn visit_body_item(&mut self, node: &BodyItem) {
        match node {
            BodyItem::For(f) => self.visit_for(f),
            BodyItem::While(w) => self.visit_while(w),
            BodyItem::If(i) => self.visit_if(i),
            BodyItem::Try(t) => self.visit_try(t),
            BodyItem::KeywordCall(k) => self.visit_keyword_call(k),
            _ => {}
        }
    }

    fn visit_keyword_call(&mut self, _node: &KeywordCall) {}

    fn visit_for(&mut self, node: &ForLoop) {
        for item in &node.body {
            self.visit_body_item(item);
        }
    }

    fn visit_while(&mut self, node: &WhileLoop) {
        for item in &node.body {
            self.visit_body_item(item);
        }
    }

    fn visit_if(&mut self, node: &IfBlock) {
        for branch in &node.branches {
            for item in &branch.body {
                self.visit_body_item(item);
            }
        }
    }

    fn visit_try(&mut self, node: &TryBlock) {
        for branch in &node.branches {
            for item in &branch.body {
                self.visit_body_item(item);
            }
        }
    }
}
