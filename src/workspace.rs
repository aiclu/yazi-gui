use crate::fs_ops::FolderEntry;
use crate::yazi::FileEntry;
use gpui::HighlightStyle;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io;
use std::ops::Range;

pub(crate) const NAVIGATION_HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NavigationTarget {
    ComputerView,
    Directory(String),
}

impl NavigationTarget {
    fn same_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ComputerView, Self::ComputerView) => true,
            (Self::Directory(left), Self::Directory(right)) => {
                tree_path_key(left) == tree_path_key(right)
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NavigationHistory {
    entries: Vec<NavigationTarget>,
    cursor: usize,
}

impl NavigationHistory {
    pub(crate) fn new(initial: NavigationTarget) -> Self {
        Self {
            entries: vec![initial],
            cursor: 0,
        }
    }

    pub(crate) fn current(&self) -> &NavigationTarget {
        &self.entries[self.cursor]
    }

    pub(crate) fn can_go_back(&self) -> bool {
        self.cursor > 0
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }

    pub(crate) fn back_target(&self) -> Option<(usize, NavigationTarget)> {
        self.cursor.checked_sub(1).and_then(|cursor| {
            self.entries
                .get(cursor)
                .cloned()
                .map(|target| (cursor, target))
        })
    }

    pub(crate) fn forward_target(&self) -> Option<(usize, NavigationTarget)> {
        let cursor = self.cursor + 1;
        self.entries
            .get(cursor)
            .cloned()
            .map(|target| (cursor, target))
    }

    pub(crate) fn commit_new(&mut self, target: NavigationTarget) -> bool {
        if self.current().same_as(&target) {
            return false;
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push(target);
        self.cursor = self.entries.len() - 1;
        if self.entries.len() > NAVIGATION_HISTORY_LIMIT {
            let removed = self.entries.len() - NAVIGATION_HISTORY_LIMIT;
            self.entries.drain(..removed);
            self.cursor = self.cursor.saturating_sub(removed);
        }
        true
    }

    pub(crate) fn commit_cursor(&mut self, cursor: usize) -> bool {
        if cursor >= self.entries.len() {
            return false;
        }
        self.cursor = cursor;
        true
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn cursor(&self) -> usize {
        self.cursor
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortField {
    Name,
    Modified,
    Size,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SortState {
    pub(crate) field: SortField,
    pub(crate) direction: SortDirection,
}

impl Default for SortState {
    fn default() -> Self {
        Self {
            field: SortField::Name,
            direction: SortDirection::Ascending,
        }
    }
}

pub enum Preview {
    Empty,
    Loading,
    Dir,
    Binary {
        size: u64,
    },
    Image {
        path: String,
    },
    Code {
        text: String,
        highlights: Vec<(Range<usize>, HighlightStyle)>,
    },
    Text(String),
}

pub struct Tab {
    pub(crate) cwd: String,
    pub(crate) history: NavigationHistory,
    pub(crate) files: Vec<FileEntry>,
    /// 选中的文件名（单选时含 1 个，多选时多个）。
    pub(crate) selected: Vec<String>,
    /// Shift 范围选择的锚点文件名，避免排序后索引失效。
    pub(crate) anchor: Option<String>,
    /// 当前标签页的列表排序方式。
    pub(crate) sort: SortState,
    /// 是否处于「此电脑」虚拟视图（列出所有磁盘，不经过 yazi）。
    pub(crate) computer_view: bool,
    pub(crate) refreshing: bool,
    /// 当前标签页刷新请求的令牌，避免其他标签页的请求覆盖本页状态。
    pub(crate) refresh_request_id: u64,
    /// 当前标签页磁盘扫描请求的令牌。
    pub(crate) drive_scan_id: u64,
    pub(crate) search: Option<SearchState>,
    pub(crate) search_generation: u64,
    pub(crate) preview: Preview,
    pub(crate) preview_path: Option<String>,
}

pub struct SearchState {
    pub(crate) cwd: String,
    pub(crate) query: String,
    pub(crate) results: Vec<FileEntry>,
    pub(crate) generation: u64,
    pub(crate) scanning: bool,
}

impl Tab {
    pub(crate) fn new(cwd: &str) -> Self {
        Self {
            cwd: cwd.to_string(),
            history: NavigationHistory::new(NavigationTarget::Directory(cwd.to_string())),
            files: Vec::new(),
            selected: Vec::new(),
            anchor: None,
            sort: SortState::default(),
            computer_view: false,
            refreshing: false,
            refresh_request_id: 0,
            drive_scan_id: 0,
            search: None,
            search_generation: 0,
            preview: Preview::Empty,
            preview_path: None,
        }
    }

    pub(crate) fn new_computer() -> Self {
        let mut tab = Self::new("");
        tab.computer_view = true;
        tab.history = NavigationHistory::new(NavigationTarget::ComputerView);
        tab
    }

    pub(crate) fn invalidate_refresh(&mut self) {
        self.refresh_request_id = self.refresh_request_id.wrapping_add(1);
        self.refreshing = false;
    }

    pub(crate) fn clear_preview(&mut self) {
        self.preview = Preview::Empty;
        self.preview_path = None;
    }

    pub(crate) fn cancel_search(&mut self) {
        self.search_generation = self.search_generation.wrapping_add(1);
        self.search = None;
    }
}

#[derive(Clone)]
pub enum FolderTreeRowKind {
    Folder(FolderEntry),
    Loading,
    Error(String),
}

#[derive(Clone)]
pub struct FolderTreeRow {
    pub(crate) depth: usize,
    pub(crate) kind: FolderTreeRowKind,
}

pub(crate) struct FolderTreeState {
    pub(crate) expanded: HashMap<String, String>,
    pub(crate) children: HashMap<String, Vec<FolderEntry>>,
    pub(crate) loading: HashMap<String, u64>,
    pub(crate) errors: HashMap<String, String>,
    next_scan_id: u64,
}

impl Default for FolderTreeState {
    fn default() -> Self {
        Self {
            expanded: HashMap::new(),
            children: HashMap::new(),
            loading: HashMap::new(),
            errors: HashMap::new(),
            next_scan_id: 0,
        }
    }
}

impl FolderTreeState {
    pub(crate) fn is_expanded(&self, path: &str) -> bool {
        self.expanded.contains_key(&tree_path_key(path))
    }

    pub(crate) fn toggle(&mut self, path: &str) -> bool {
        let key = tree_path_key(path);
        if self.expanded.remove(&key).is_some() {
            false
        } else {
            self.expanded.insert(key, path.to_string());
            true
        }
    }

    pub(crate) fn expanded_paths(&self) -> Vec<String> {
        self.expanded.values().cloned().collect()
    }

    pub(crate) fn reset(&mut self) {
        self.expanded.clear();
        self.children.clear();
        self.loading.clear();
        self.errors.clear();
        self.next_scan_id = self.next_scan_id.wrapping_add(1);
    }

    pub(crate) fn invalidate(&mut self, path: &str) {
        let key = tree_path_key(path);
        self.children.remove(&key);
        self.errors.remove(&key);
    }

    pub(crate) fn begin_scan(&mut self, path: &str, force: bool) -> Option<u64> {
        let key = tree_path_key(path);
        if !force && (self.children.contains_key(&key) || self.loading.contains_key(&key)) {
            return None;
        }
        if force {
            self.invalidate(path);
        }
        self.next_scan_id = self.next_scan_id.wrapping_add(1);
        let scan_id = self.next_scan_id;
        self.loading.insert(key, scan_id);
        Some(scan_id)
    }

    pub(crate) fn finish_scan(
        &mut self,
        path: &str,
        scan_id: u64,
        result: io::Result<Vec<FolderEntry>>,
    ) {
        let key = tree_path_key(path);
        if self.loading.get(&key).copied() != Some(scan_id) {
            return;
        }
        self.loading.remove(&key);
        match result {
            Ok(children) => {
                self.errors.remove(&key);
                self.children.insert(key, children);
            }
            Err(error) => {
                self.children.remove(&key);
                self.errors.insert(key, error.to_string());
            }
        }
    }

    pub(crate) fn visible_rows(&self, drive_roots: &[String]) -> Vec<FolderTreeRow> {
        let mut rows = Vec::new();
        for drive in drive_roots {
            rows.push(FolderTreeRow {
                depth: 0,
                kind: FolderTreeRowKind::Folder(FolderEntry {
                    path: drive.clone(),
                    name: drive.clone(),
                    is_link: false,
                }),
            });
            if self.is_expanded(drive) {
                self.append_children(&mut rows, drive, 1);
            }
        }
        rows
    }

    fn append_children(&self, rows: &mut Vec<FolderTreeRow>, path: &str, depth: usize) {
        let key = tree_path_key(path);
        if let Some(error) = self.errors.get(&key) {
            rows.push(FolderTreeRow {
                depth,
                kind: FolderTreeRowKind::Error(error.clone()),
            });
            return;
        }
        if self.loading.contains_key(&key) {
            rows.push(FolderTreeRow {
                depth,
                kind: FolderTreeRowKind::Loading,
            });
            return;
        }
        let Some(children) = self.children.get(&key) else {
            return;
        };
        for child in children {
            rows.push(FolderTreeRow {
                depth,
                kind: FolderTreeRowKind::Folder(child.clone()),
            });
            if self.is_expanded(&child.path) && !child.is_link {
                self.append_children(rows, &child.path, depth + 1);
            }
        }
    }
}

/// 工作区的持久状态：标签页、当前标签页、磁盘树以及异步请求令牌。
pub(crate) struct WorkspaceState {
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active: usize,
    pub(crate) tab_view_start: usize,
    pub(crate) drive_roots: Vec<String>,
    pub(crate) folder_tree: FolderTreeState,
}

#[derive(Clone, Debug)]
pub(crate) struct RefreshRequest {
    pub(crate) tab_index: usize,
    pub(crate) cwd: String,
    pub(crate) request_id: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchRequest {
    pub(crate) tab_index: usize,
    pub(crate) cwd: String,
    pub(crate) query: String,
    pub(crate) generation: u64,
}

impl WorkspaceState {
    pub(crate) fn new(cwd: &str) -> Self {
        Self {
            tabs: vec![Tab::new(cwd)],
            active: 0,
            tab_view_start: 0,
            drive_roots: Vec::new(),
            folder_tree: FolderTreeState::default(),
        }
    }

    pub(crate) fn cur(&self) -> &Tab {
        &self.tabs[self.active]
    }

    pub(crate) fn cur_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    pub(crate) fn begin_refresh(&mut self) -> Option<RefreshRequest> {
        let tab_index = self.active;
        let tab = self.tabs.get_mut(tab_index)?;
        if tab.computer_view || tab.refreshing {
            return None;
        }
        tab.refresh_request_id = tab.refresh_request_id.wrapping_add(1);
        tab.refreshing = true;
        Some(RefreshRequest {
            tab_index,
            cwd: tab.cwd.clone(),
            request_id: tab.refresh_request_id,
        })
    }

    pub(crate) fn finish_refresh(&mut self, request: &RefreshRequest) -> bool {
        let Some(tab) = self.tabs.get_mut(request.tab_index) else {
            return false;
        };
        if !tab.refreshing
            || tab.refresh_request_id != request.request_id
            || tree_path_key(&tab.cwd) != tree_path_key(&request.cwd)
        {
            return false;
        }
        tab.refreshing = false;
        true
    }

    pub(crate) fn begin_search_request(&self) -> Option<SearchRequest> {
        let tab_index = self.active;
        let tab = self.tabs.get(tab_index)?;
        let search = tab.search.as_ref()?;
        Some(SearchRequest {
            tab_index,
            cwd: search.cwd.clone(),
            query: search.query.clone(),
            generation: search.generation,
        })
    }

    pub(crate) fn apply_search_results(
        &mut self,
        request: &SearchRequest,
        mut results: Vec<FileEntry>,
    ) -> bool {
        let Some(tab) = self.tabs.get_mut(request.tab_index) else {
            return false;
        };
        let valid = tab.search.as_ref().is_some_and(|search| {
            search.generation == request.generation
                && search.cwd == request.cwd
                && search.query == request.query
        });
        if !valid {
            return false;
        }
        sort_files(&mut results, tab.sort);
        let names = results
            .iter()
            .map(|file| file.name.clone())
            .collect::<Vec<_>>();
        if let Some(search) = tab.search.as_mut() {
            search.results = results;
            search.scanning = false;
        }
        reconcile_selection(&mut tab.selected, &mut tab.anchor, &names);
        true
    }

    pub(crate) fn fail_search(&mut self, request: &SearchRequest) -> bool {
        let Some(tab) = self.tabs.get_mut(request.tab_index) else {
            return false;
        };
        let valid = tab.search.as_ref().is_some_and(|search| {
            search.generation == request.generation
                && search.cwd == request.cwd
                && search.query == request.query
        });
        if !valid {
            return false;
        }
        if let Some(search) = tab.search.as_mut() {
            search.results.clear();
            search.scanning = false;
        }
        true
    }

    pub(crate) fn apply_gui_files(
        &mut self,
        cwd: String,
        mut files: Vec<FileEntry>,
    ) -> Option<bool> {
        if self.cur().computer_view || tree_path_key(&self.cur().cwd) != tree_path_key(&cwd) {
            return None;
        }
        let tab = self.cur_mut();
        let refreshed = tab.refreshing && tree_path_key(&tab.cwd) == tree_path_key(&cwd);
        tab.cwd = cwd;
        sort_files(&mut files, tab.sort);
        let names = files
            .iter()
            .map(|file| file.name.clone())
            .collect::<Vec<_>>();
        tab.files = files;
        if refreshed {
            tab.invalidate_refresh();
        }
        if !tab
            .search
            .as_ref()
            .is_some_and(|search| !search.query.is_empty())
        {
            reconcile_selection(&mut tab.selected, &mut tab.anchor, &names);
        }
        Some(refreshed)
    }
}

pub(crate) fn tree_path_key(path: &str) -> String {
    let mut normalized = path.replace('/', "\\");
    while normalized.len() > 3 && normalized.ends_with('\\') {
        normalized.pop();
    }
    normalized.to_ascii_lowercase()
}

pub(crate) fn tree_ancestor_paths(path: &str) -> Vec<String> {
    let mut current = std::path::PathBuf::from(path);
    let mut ancestors = Vec::new();
    loop {
        let text = current.to_string_lossy().into_owned();
        if text.is_empty() {
            return Vec::new();
        }
        ancestors.push(text.clone());
        if crate::fs_ops::is_drive_root(&text) {
            break;
        }
        let Some(parent) = current.parent() else {
            return Vec::new();
        };
        if parent == current {
            return Vec::new();
        }
        current = parent.to_path_buf();
    }
    ancestors.reverse();
    if ancestors
        .first()
        .is_some_and(|ancestor| crate::fs_ops::is_drive_root(ancestor))
    {
        ancestors
    } else {
        Vec::new()
    }
}

pub(crate) fn reconcile_selection(
    selected: &mut Vec<String>,
    anchor: &mut Option<String>,
    available_names: &[String],
) {
    selected.retain(|name| available_names.iter().any(|available| available == name));
    if anchor
        .as_ref()
        .is_some_and(|name| !available_names.iter().any(|available| available == name))
    {
        *anchor = None;
    }
}

pub(crate) fn sort_files(files: &mut [FileEntry], sort: SortState) {
    files.sort_by(|a, b| {
        let directory_order = b.is_dir.cmp(&a.is_dir);
        if directory_order != Ordering::Equal {
            return directory_order;
        }

        let primary = match sort.field {
            SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortField::Modified => a.mtime.partial_cmp(&b.mtime).unwrap_or(Ordering::Equal),
            SortField::Size => a.size.cmp(&b.size),
        };
        let primary = match sort.direction {
            SortDirection::Ascending => primary,
            SortDirection::Descending => primary.reverse(),
        };
        primary
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            is_dir: false,
            size: 1,
            mtime: 0.0,
        }
    }

    #[test]
    fn refresh_request_is_owned_by_workspace_state() {
        let mut workspace = WorkspaceState::new(r"D:\Projects");
        let request = workspace.begin_refresh().unwrap();
        assert!(!workspace.finish_refresh(&RefreshRequest {
            request_id: request.request_id.wrapping_add(1),
            ..request.clone()
        }));
        assert!(workspace.cur().refreshing);
        assert!(workspace.finish_refresh(&request));
        assert!(!workspace.cur().refreshing);
    }

    #[test]
    fn search_results_are_rejected_after_generation_changes() {
        let mut workspace = WorkspaceState::new(r"D:\Projects");
        workspace.cur_mut().search = Some(SearchState {
            cwd: r"D:\Projects".to_string(),
            query: "report".to_string(),
            results: Vec::new(),
            generation: 3,
            scanning: true,
        });
        let request = workspace.begin_search_request().unwrap();
        workspace.cur_mut().search.as_mut().unwrap().generation = 4;
        assert!(!workspace.apply_search_results(&request, vec![file("report.txt")]));
        assert!(workspace.cur().search.as_ref().unwrap().scanning);
    }

    #[test]
    fn navigation_history_starts_at_its_initial_target() {
        let history =
            NavigationHistory::new(NavigationTarget::Directory(r"D:\Projects".to_string()));

        assert_eq!(
            history.current(),
            &NavigationTarget::Directory(r"D:\Projects".to_string())
        );
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn navigation_history_has_back_and_forward_boundaries() {
        let mut history =
            NavigationHistory::new(NavigationTarget::Directory(r"D:\one".to_string()));
        history.commit_new(NavigationTarget::Directory(r"D:\two".to_string()));

        let (cursor, target) = history.back_target().unwrap();
        assert_eq!(cursor, 0);
        assert_eq!(target, NavigationTarget::Directory(r"D:\one".to_string()));
        assert!(history.commit_cursor(cursor));
        assert!(!history.can_go_back());
        assert!(history.can_go_forward());

        let (cursor, target) = history.forward_target().unwrap();
        assert_eq!(cursor, 1);
        assert_eq!(target, NavigationTarget::Directory(r"D:\two".to_string()));
        assert!(history.commit_cursor(cursor));
        assert!(!history.can_go_forward());
    }

    #[test]
    fn navigation_history_collapses_case_and_separator_duplicates() {
        let mut history =
            NavigationHistory::new(NavigationTarget::Directory(r"D:\Projects".to_string()));

        assert!(!history.commit_new(NavigationTarget::Directory(r"d:/projects/".to_string(),)));
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn navigation_history_truncates_forward_entries_after_new_navigation() {
        let mut history =
            NavigationHistory::new(NavigationTarget::Directory(r"D:\one".to_string()));
        history.commit_new(NavigationTarget::Directory(r"D:\two".to_string()));
        history.commit_new(NavigationTarget::Directory(r"D:\three".to_string()));
        let (cursor, _) = history.back_target().unwrap();
        history.commit_cursor(cursor);

        assert!(history.commit_new(NavigationTarget::Directory(r"D:\new".to_string())));
        assert_eq!(history.len(), 3);
        assert_eq!(history.cursor(), 2);
        assert!(!history.can_go_forward());
        assert_eq!(
            history.current(),
            &NavigationTarget::Directory(r"D:\new".to_string())
        );
    }

    #[test]
    fn navigation_history_includes_computer_view_as_a_target() {
        let mut history = NavigationHistory::new(NavigationTarget::ComputerView);
        history.commit_new(NavigationTarget::Directory(r"C:\".to_string()));
        assert!(history.can_go_back());
        let (_, target) = history.back_target().unwrap();
        assert_eq!(target, NavigationTarget::ComputerView);
    }

    #[test]
    fn navigation_history_keeps_at_most_one_hundred_entries() {
        let mut history = NavigationHistory::new(NavigationTarget::Directory(r"D:\0".to_string()));
        for index in 1..=150 {
            history.commit_new(NavigationTarget::Directory(format!(r"D:\{index}")));
        }

        assert_eq!(history.len(), NAVIGATION_HISTORY_LIMIT);
        assert_eq!(history.cursor(), NAVIGATION_HISTORY_LIMIT - 1);
        assert_eq!(
            history.current(),
            &NavigationTarget::Directory(r"D:\150".to_string())
        );
    }
}
