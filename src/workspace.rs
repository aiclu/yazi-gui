use crate::fs_ops::FolderEntry;
use crate::yazi::FileEntry;
use gpui::HighlightStyle;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io;
use std::ops::Range;

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
