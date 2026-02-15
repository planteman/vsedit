//! Action system with menu contributions.
//!
//! Equivalent to VS Code's `vs/platform/actions/common/actions.ts`.
//! Provides action registration with menu contributions, keybindings, and when-clause guards.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use vsedit_commands::{CommandHandler, CommandRegistration, CommandRegistry};
use vsedit_contextkey::ContextKeyExpr;

/// Identifies a menu location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuId {
    CommandPalette,
    EditorContext,
    EditorTitle,
    EditorTitleContext,
    ExplorerContext,
    MenubarFile,
    MenubarEdit,
    MenubarSelection,
    MenubarView,
    MenubarGo,
    MenubarRun,
    MenubarTerminal,
    MenubarHelp,
    StatusBarItem,
    ViewTitle,
    ViewItemContext,
    SCMTitle,
    SCMContext,
    TerminalContext,
    DebugCallStackContext,
    TouchBar,
}

/// An item within a menu group.
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub command_id: String,
    pub title: String,
    pub group: Option<String>,
    pub order: Option<i32>,
    pub when: Option<ContextKeyExpr>,
}

/// Metadata about a registered action.
#[derive(Clone)]
struct ActionMeta {
    title: String,
    category: Option<String>,
    tooltip: Option<String>,
    icon: Option<String>,
    precondition: Option<ContextKeyExpr>,
}

/// Registry for actions (commands with UI metadata).
pub struct ActionRegistry {
    command_registry: Arc<CommandRegistry>,
    menus: RwLock<HashMap<MenuId, Vec<MenuItem>>>,
    actions: RwLock<HashMap<String, ActionMeta>>,
    _registrations: Mutex<Vec<CommandRegistration>>,
}

impl ActionRegistry {
    pub fn new(command_registry: Arc<CommandRegistry>) -> Self {
        Self {
            command_registry,
            menus: RwLock::new(HashMap::new()),
            actions: RwLock::new(HashMap::new()),
            _registrations: Mutex::new(Vec::new()),
        }
    }

    /// Register an action (command + menu contributions).
    pub fn register_action(
        &self,
        id: impl Into<String>,
        title: impl Into<String>,
        category: Option<String>,
        handler: CommandHandler,
        menu_items: Vec<(MenuId, MenuItem)>,
        precondition: Option<ContextKeyExpr>,
    ) {
        let id = id.into();
        let title = title.into();

        // Register the underlying command
        let reg = self.command_registry.register(&id, handler);
        self._registrations.lock().unwrap().push(reg);

        // Store action metadata
        {
            let mut actions = self.actions.write().unwrap();
            actions.insert(
                id.clone(),
                ActionMeta {
                    title,
                    category,
                    tooltip: None,
                    icon: None,
                    precondition,
                },
            );
        }

        // Register menu items
        {
            let mut menus = self.menus.write().unwrap();
            for (menu_id, item) in menu_items {
                menus.entry(menu_id).or_default().push(item);
            }
        }
    }

    /// Get all menu items for a given menu, sorted by group and order.
    pub fn get_menu_items(&self, menu_id: MenuId) -> Vec<MenuItem> {
        let menus = self.menus.read().unwrap();
        let mut items = menus.get(&menu_id).cloned().unwrap_or_default();
        items.sort_by(|a, b| {
            let ga = a.group.as_deref().unwrap_or("");
            let gb = b.group.as_deref().unwrap_or("");
            ga.cmp(gb)
                .then_with(|| a.order.unwrap_or(0).cmp(&b.order.unwrap_or(0)))
        });
        items
    }

    /// Get the title of a registered action.
    pub fn get_action_title(&self, id: &str) -> Option<String> {
        let actions = self.actions.read().unwrap();
        actions.get(id).map(|a| {
            if let Some(cat) = &a.category {
                format!("{cat}: {}", a.title)
            } else {
                a.title.clone()
            }
        })
    }

    /// Get all registered action IDs.
    pub fn get_action_ids(&self) -> Vec<String> {
        let actions = self.actions.read().unwrap();
        actions.keys().cloned().collect()
    }

    /// Check if an action's precondition is satisfied.
    pub fn is_action_enabled(
        &self,
        id: &str,
        context: &dyn vsedit_contextkey::IContext,
    ) -> bool {
        let actions = self.actions.read().unwrap();
        match actions.get(id) {
            Some(meta) => match &meta.precondition {
                Some(expr) => expr.evaluate(context),
                None => true,
            },
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_handler() -> CommandHandler {
        Box::new(|_args| Ok(None))
    }

    #[test]
    fn register_action_with_menu() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds.clone());

        let item = MenuItem {
            command_id: "test.action".into(),
            title: "Test Action".into(),
            group: Some("navigation".into()),
            order: Some(1),
            when: None,
        };

        registry.register_action(
            "test.action",
            "Test Action",
            Some("Test".into()),
            noop_handler(),
            vec![(MenuId::CommandPalette, item)],
            None,
        );

        let items = registry.get_menu_items(MenuId::CommandPalette);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].command_id, "test.action");
        assert!(cmds.has("test.action"));
    }

    #[test]
    fn action_title_with_category() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);

        registry.register_action(
            "file.save",
            "Save",
            Some("File".into()),
            noop_handler(),
            vec![],
            None,
        );

        assert_eq!(
            registry.get_action_title("file.save"),
            Some("File: Save".to_string())
        );
    }

    #[test]
    fn menu_items_sorted_by_group_and_order() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);

        for (id, group, order) in [
            ("c", "z_navigation", 2),
            ("b", "a_edit", 1),
            ("a", "z_navigation", 1),
        ] {
            let item = MenuItem {
                command_id: id.into(),
                title: id.into(),
                group: Some(group.into()),
                order: Some(order),
                when: None,
            };
            registry.register_action(id, id, None, noop_handler(), vec![(MenuId::EditorContext, item)], None);
        }

        let items = registry.get_menu_items(MenuId::EditorContext);
        assert_eq!(items[0].command_id, "b"); // a_edit
        assert_eq!(items[1].command_id, "a"); // z_navigation, order 1
        assert_eq!(items[2].command_id, "c"); // z_navigation, order 2
    }

    #[test]
    fn precondition_evaluation() {
        use vsedit_contextkey::{ContextKeyExpr, ContextKeyService, ContextKeyValue, IContext};

        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);

        let when = ContextKeyExpr::parse("editorTextFocus").unwrap();

        registry.register_action("guarded", "Guarded", None, noop_handler(), vec![], Some(when));

        let ctx = ContextKeyService::new();

        // Not satisfied
        assert!(!registry.is_action_enabled("guarded", &ctx));

        // Satisfied
        ctx.set_context("editorTextFocus", ContextKeyValue::Bool(true));
        assert!(registry.is_action_enabled("guarded", &ctx));
    }

    #[test]
    fn unknown_action_not_enabled() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        let ctx = vsedit_contextkey::ContextKeyService::new();
        assert!(!registry.is_action_enabled("nonexistent", &ctx));
    }
}
