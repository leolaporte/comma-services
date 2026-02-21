use std::collections::{BTreeMap, HashSet};

use crate::categories::{categorize, CATEGORY_ORDER};
use crate::systemd::{
    get_service_info, list_services, ChangeAction, ChangeResult, PendingChange, Service,
    ServiceInfo, ServiceScope,
};
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    System,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Filter,
    Confirm,
    Applying,
    Info,
}

#[derive(Debug)]
pub struct CategoryGroup {
    pub name: &'static str,
    pub services: Vec<usize>, // indices into App::services
    pub collapsed: bool,
}

#[derive(Debug)]
pub struct App {
    pub services: Vec<Service>,
    pub toggled: HashSet<String>, // service names with pending changes
    pub original_state: std::collections::HashMap<String, bool>, // name -> was_enabled
    pub tab: Tab,
    pub mode: Mode,
    pub filter: String,
    pub categories: Vec<CategoryGroup>,
    pub cursor: usize, // index into visible_items
    pub visible_items: Vec<VisibleItem>,
    pub results: Vec<ChangeResult>,
    pub info: Option<ServiceInfo>,
    pub should_quit: bool,
}

#[derive(Debug, Clone)]
pub enum VisibleItem {
    Category(usize), // index into categories
    Service(usize),  // index into services
}

impl App {
    pub fn new() -> Result<Self> {
        let mut app = Self {
            services: Vec::new(),
            toggled: HashSet::new(),
            original_state: std::collections::HashMap::new(),
            tab: Tab::System,
            mode: Mode::Normal,
            filter: String::new(),
            categories: Vec::new(),
            cursor: 0,
            visible_items: Vec::new(),
            results: Vec::new(),
            info: None,
            should_quit: false,
        };
        app.refresh()?;
        Ok(app)
    }

    pub fn refresh(&mut self) -> Result<()> {
        let scope = match self.tab {
            Tab::System => ServiceScope::System,
            Tab::User => ServiceScope::User,
        };
        self.services = list_services(&scope)?;

        self.original_state.clear();
        for svc in &self.services {
            self.original_state.insert(svc.name.clone(), svc.enabled);
        }

        self.toggled.clear();
        self.rebuild_categories();
        self.rebuild_visible();
        self.cursor = 0;
        Ok(())
    }

    fn rebuild_categories(&mut self) {
        let mut groups: BTreeMap<&'static str, Vec<usize>> = BTreeMap::new();

        for (idx, svc) in self.services.iter().enumerate() {
            let cat = categorize(&svc.name);
            groups.entry(cat).or_default().push(idx);
        }

        self.categories = CATEGORY_ORDER
            .iter()
            .filter_map(|&cat_name| {
                groups.remove(cat_name).map(|services| CategoryGroup {
                    name: cat_name,
                    services,
                    collapsed: false,
                })
            })
            .collect();
    }

    pub fn rebuild_visible(&mut self) {
        self.visible_items.clear();
        let filter_lower = self.filter.to_lowercase();

        for (cat_idx, cat) in self.categories.iter().enumerate() {
            let matching_services: Vec<usize> = if filter_lower.is_empty() {
                cat.services.clone()
            } else {
                cat.services
                    .iter()
                    .filter(|&&svc_idx| {
                        self.services[svc_idx]
                            .name
                            .to_lowercase()
                            .contains(&filter_lower)
                    })
                    .copied()
                    .collect()
            };

            if matching_services.is_empty() {
                continue;
            }

            self.visible_items.push(VisibleItem::Category(cat_idx));

            if !cat.collapsed {
                for svc_idx in matching_services {
                    self.visible_items.push(VisibleItem::Service(svc_idx));
                }
            }
        }
    }

    pub fn move_cursor(&mut self, delta: i32) {
        if self.visible_items.is_empty() {
            return;
        }
        let len = self.visible_items.len() as i32;
        let new = (self.cursor as i32 + delta).rem_euclid(len);
        self.cursor = new as usize;
    }

    pub fn toggle_current(&mut self) {
        if let Some(VisibleItem::Service(svc_idx)) = self.visible_items.get(self.cursor) {
            let svc = &mut self.services[*svc_idx];
            svc.enabled = !svc.enabled;

            let original = self.original_state.get(&svc.name).copied().unwrap_or(false);
            if svc.enabled == original {
                self.toggled.remove(&svc.name);
            } else {
                self.toggled.insert(svc.name.clone());
            }
        }
    }

    pub fn toggle_collapse(&mut self) {
        let cat_idx = match self.visible_items.get(self.cursor) {
            Some(VisibleItem::Category(idx)) => Some(*idx),
            Some(VisibleItem::Service(svc_idx)) => {
                // Find which category this service belongs to
                self.categories
                    .iter()
                    .position(|cat| cat.services.contains(svc_idx))
            }
            None => None,
        };

        if let Some(idx) = cat_idx {
            self.categories[idx].collapsed = !self.categories[idx].collapsed;
            self.rebuild_visible();
            // Keep cursor in bounds
            if self.cursor >= self.visible_items.len() {
                self.cursor = self.visible_items.len().saturating_sub(1);
            }
        }
    }

    pub fn pending_changes(&self) -> Vec<PendingChange> {
        let scope = match self.tab {
            Tab::System => ServiceScope::System,
            Tab::User => ServiceScope::User,
        };

        self.services
            .iter()
            .filter(|svc| self.toggled.contains(&svc.name))
            .map(|svc| PendingChange {
                service: svc.name.clone(),
                scope: scope.clone(),
                action: if svc.enabled {
                    ChangeAction::Enable
                } else {
                    ChangeAction::Disable
                },
            })
            .collect()
    }

    pub fn has_pending_changes(&self) -> bool {
        !self.toggled.is_empty()
    }

    pub fn pending_count(&self) -> usize {
        self.toggled.len()
    }

    pub fn apply_done(&mut self, results: Vec<ChangeResult>) -> Result<()> {
        self.results = results;
        self.refresh()
    }

    pub fn switch_tab(&mut self) -> Result<()> {
        self.tab = match self.tab {
            Tab::System => Tab::User,
            Tab::User => Tab::System,
        };
        self.filter.clear();
        self.refresh()
    }

    pub fn is_service_dirty(&self, svc: &Service) -> bool {
        self.toggled.contains(&svc.name)
    }

    pub fn show_info(&mut self) {
        if let Some(VisibleItem::Service(svc_idx)) = self.visible_items.get(self.cursor) {
            let svc = &self.services[*svc_idx];
            let scope = match self.tab {
                Tab::System => ServiceScope::System,
                Tab::User => ServiceScope::User,
            };
            self.info = Some(get_service_info(&scope, &svc.name));
            self.mode = Mode::Info;
        }
    }
}
