#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use chrono::{Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock as ARwLock;

use crate::file_filter::KNOWLEDGE_FOLDER_NAME;
use crate::files_correction::get_project_dirs;
use crate::global_context::GlobalContext;
use crate::memories::{
    create_frontmatter, get_global_knowledge_dir, memories_add, normalize_memory_tags,
    update_memory_document_frontmatter,
};

const HIGH_CONFIDENCE_APPROVAL_THRESHOLD: f32 = 0.85;
const PAYLOAD_CONTENT_MAX_CHARS: usize = 12000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemorySource {
    Buddy,
    Trajectory,
    Git,
    Manual,
    BehaviorLearner,
    MemoryGarden,
    KnowledgeConflictResolver,
}

impl Default for MemorySource {
    fn default() -> Self {
        Self::Buddy
    }
}

impl MemorySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buddy => "buddy",
            Self::Trajectory => "trajectory",
            Self::Git => "git",
            Self::Manual => "manual",
            Self::BehaviorLearner => "behavior_learner",
            Self::MemoryGarden => "memory_garden",
            Self::KnowledgeConflictResolver => "knowledge_conflict_resolver",
        }
    }

    pub fn is_autonomous(self) -> bool {
        !matches!(self, Self::Manual)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOpType {
    CreateMemory,
    UpdateMemory,
    Retag,
    RepairLinks,
    Refresh,
    ArchiveCandidate,
    Archive,
    MergeArchive,
    DeleteCandidate,
    PromoteDigest,
    MarkReviewNeeded,
    MarkStale,
}

impl Default for MemoryOpType {
    fn default() -> Self {
        Self::CreateMemory
    }
}

impl MemoryOpType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreateMemory => "create_memory",
            Self::UpdateMemory => "update_memory",
            Self::Retag => "retag",
            Self::RepairLinks => "repair_links",
            Self::Refresh => "refresh",
            Self::ArchiveCandidate => "archive_candidate",
            Self::Archive => "archive",
            Self::MergeArchive => "merge_archive",
            Self::DeleteCandidate => "delete_candidate",
            Self::PromoteDigest => "promote_digest",
            Self::MarkReviewNeeded => "mark_review_needed",
            Self::MarkStale => "mark_stale",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOpStatus {
    Pending,
    Approved,
    Applied,
    Rejected,
    Failed,
    Skipped,
}

impl Default for MemoryOpStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl MemoryOpStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Applied => "applied",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCandidateStatus {
    Proposed,
    Approved,
    Promoted,
    Rejected,
    Skipped,
}

impl Default for MemoryCandidateStatus {
    fn default() -> Self {
        Self::Proposed
    }
}

impl MemoryCandidateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Approved => "approved",
            Self::Promoted => "promoted",
            Self::Rejected => "rejected",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryCreatePayload {
    pub title: Option<String>,
    pub content: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub filenames: Vec<String>,
    pub related_files: Vec<String>,
    pub links: Vec<String>,
    pub review_after: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryLifecyclePayload {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
    pub kind: Option<String>,
    pub filenames: Option<Vec<String>>,
    pub related_files: Option<Vec<String>>,
    pub links: Option<Vec<String>>,
    pub review_after: Option<String>,
    pub superseded_by: Option<String>,
    pub superseded_paths: Vec<String>,
    pub canonical: Option<MemoryCreatePayload>,
}

impl MemoryCreatePayload {
    pub fn normalized(mut self) -> Self {
        self.title = normalize_optional_text(self.title.as_deref());
        self.content = redact_and_cap_payload_text(&self.content, PAYLOAD_CONTENT_MAX_CHARS);
        self.tags = normalize_tags(&self.tags);
        self.kind = normalize_kind(&self.kind);
        self.filenames = normalize_paths(&self.filenames);
        self.related_files = normalize_paths(&self.related_files);
        self.links = normalize_strings(&self.links);
        self.review_after = normalize_review_after(self.review_after.as_deref());
        self
    }
}

impl MemoryLifecyclePayload {
    pub fn normalized(mut self) -> Self {
        self.title = normalize_optional_text(self.title.as_deref());
        self.content = self
            .content
            .as_deref()
            .map(|content| redact_and_cap_payload_text(content, PAYLOAD_CONTENT_MAX_CHARS))
            .filter(|content| !content.is_empty());
        self.tags = self.tags.map(|tags| normalize_tags(&tags));
        self.kind = self.kind.as_deref().map(normalize_kind);
        self.filenames = self.filenames.map(|paths| normalize_paths(&paths));
        self.related_files = self.related_files.map(|paths| normalize_paths(&paths));
        self.links = self.links.map(|links| normalize_strings(&links));
        self.review_after = normalize_review_after(self.review_after.as_deref());
        self.superseded_by = normalize_optional_string(self.superseded_by.as_deref());
        self.superseded_paths = normalize_paths(&self.superseded_paths);
        self.canonical = self.canonical.map(|canonical| canonical.normalized());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryLifecycleOp {
    pub op_id: String,
    pub source: MemorySource,
    pub op_type: MemoryOpType,
    pub payload: MemoryLifecyclePayload,
    pub target_paths: Vec<String>,
    pub evidence: String,
    pub confidence: f32,
    pub requires_approval: bool,
    pub status: MemoryOpStatus,
    pub created_at: String,
    pub applied_at: Option<String>,
    pub idempotency_key: String,
    pub error: Option<String>,
}

impl Default for MemoryLifecycleOp {
    fn default() -> Self {
        Self {
            op_id: String::new(),
            source: MemorySource::default(),
            op_type: MemoryOpType::default(),
            payload: MemoryLifecyclePayload::default(),
            target_paths: Vec::new(),
            evidence: String::new(),
            confidence: 0.0,
            requires_approval: true,
            status: MemoryOpStatus::default(),
            created_at: String::new(),
            applied_at: None,
            idempotency_key: String::new(),
            error: None,
        }
    }
}

impl MemoryLifecycleOp {
    pub fn pending(
        op_id: impl Into<String>,
        source: MemorySource,
        op_type: MemoryOpType,
        target_paths: Vec<String>,
        evidence: impl Into<String>,
        confidence: f32,
        created_at: impl Into<String>,
    ) -> Self {
        let target_paths = normalize_paths(&target_paths);
        let evidence = evidence.into();
        let idempotency_key = compute_idempotency_key(&MemoryOpIdempotencyInput {
            source,
            op_type,
            target_paths: target_paths.clone(),
            tags: Vec::new(),
            kind: None,
            source_id: None,
            title: None,
            content: None,
            evidence: Some(evidence.clone()),
        });
        Self {
            op_id: op_id.into(),
            source,
            op_type,
            payload: MemoryLifecyclePayload::default(),
            target_paths,
            evidence,
            confidence,
            requires_approval: default_requires_approval(op_type, confidence),
            status: MemoryOpStatus::Pending,
            created_at: created_at.into(),
            applied_at: None,
            idempotency_key,
            error: None,
        }
    }

    pub fn normalized(mut self) -> Self {
        self.op_id = self.op_id.trim().to_string();
        self.created_at = self.created_at.trim().to_string();
        self.idempotency_key = self.idempotency_key.trim().to_string();
        self.target_paths = normalize_paths(&self.target_paths);
        self.payload = self.payload.normalized();
        self.applied_at = normalize_optional_string(self.applied_at.as_deref());
        self.error = normalize_optional_string(self.error.as_deref());
        if self.idempotency_key.trim().is_empty() {
            self.idempotency_key = compute_idempotency_key(&MemoryOpIdempotencyInput {
                source: self.source,
                op_type: self.op_type,
                target_paths: self.target_paths.clone(),
                tags: Vec::new(),
                kind: None,
                source_id: None,
                title: None,
                content: None,
                evidence: Some(self.evidence.clone()),
            });
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemoryOpsRecord {
    Op { op: MemoryLifecycleOp },
}

impl MemoryOpsRecord {
    pub fn into_op(self) -> MemoryLifecycleOp {
        match self {
            Self::Op { op } => op,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryOpsState {
    pub ops: Vec<MemoryLifecycleOp>,
    pub total_records: u32,
    pub malformed_lines: u32,
    pub pending_count: u32,
    pub approved_count: u32,
    pub applied_count: u32,
    pub rejected_count: u32,
    pub failed_count: u32,
    pub skipped_count: u32,
}

impl MemoryOpsState {
    pub fn from_records(records: impl IntoIterator<Item = MemoryOpsRecord>) -> Self {
        Self::from_records_with_malformed(records, 0)
    }

    pub fn from_records_with_malformed(
        records: impl IntoIterator<Item = MemoryOpsRecord>,
        malformed_lines: u32,
    ) -> Self {
        let mut ops: Vec<MemoryLifecycleOp> = Vec::new();
        let mut op_id_index: HashMap<String, usize> = HashMap::new();
        let mut idempotency_index: HashMap<String, usize> = HashMap::new();
        let mut total_records = 0u32;

        for record in records {
            total_records = total_records.saturating_add(1);
            let op = record.into_op().normalized();
            let existing_index = nonempty_key(&op.idempotency_key)
                .and_then(|key| idempotency_index.get(key).copied())
                .or_else(|| nonempty_key(&op.op_id).and_then(|key| op_id_index.get(key).copied()));

            match existing_index {
                Some(index) => {
                    if let Some(old) = ops.get(index).cloned() {
                        remove_indexed_key(&mut op_id_index, &old.op_id, index);
                        remove_indexed_key(&mut idempotency_index, &old.idempotency_key, index);
                    }
                    ops[index] = op.clone();
                    insert_op_indexes(&op, index, &mut op_id_index, &mut idempotency_index);
                }
                None => {
                    let index = ops.len();
                    insert_op_indexes(&op, index, &mut op_id_index, &mut idempotency_index);
                    ops.push(op);
                }
            }
        }

        let mut state = Self {
            ops,
            total_records,
            malformed_lines,
            ..Self::default()
        };
        state.recount();
        state
    }

    pub fn canonical_records(&self) -> Vec<MemoryOpsRecord> {
        self.ops
            .iter()
            .cloned()
            .map(|op| MemoryOpsRecord::Op { op })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    fn recount(&mut self) {
        self.pending_count = 0;
        self.approved_count = 0;
        self.applied_count = 0;
        self.rejected_count = 0;
        self.failed_count = 0;
        self.skipped_count = 0;

        for op in &self.ops {
            match op.status {
                MemoryOpStatus::Pending => self.pending_count += 1,
                MemoryOpStatus::Approved => self.approved_count += 1,
                MemoryOpStatus::Applied => self.applied_count += 1,
                MemoryOpStatus::Rejected => self.rejected_count += 1,
                MemoryOpStatus::Failed => self.failed_count += 1,
                MemoryOpStatus::Skipped => self.skipped_count += 1,
            }
        }
    }
}

fn insert_op_indexes(
    op: &MemoryLifecycleOp,
    index: usize,
    op_id_index: &mut HashMap<String, usize>,
    idempotency_index: &mut HashMap<String, usize>,
) {
    if let Some(key) = nonempty_key(&op.op_id) {
        op_id_index.insert(key.to_string(), index);
    }
    if let Some(key) = nonempty_key(&op.idempotency_key) {
        idempotency_index.insert(key.to_string(), index);
    }
}

fn remove_indexed_key(index: &mut HashMap<String, usize>, key: &str, expected_index: usize) {
    let Some(key) = nonempty_key(key) else {
        return;
    };
    if index.get(key) == Some(&expected_index) {
        index.remove(key);
    }
}

fn nonempty_key(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryCandidate {
    pub candidate_id: String,
    pub source: MemorySource,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub filenames: Vec<String>,
    pub related_files: Vec<String>,
    pub source_id: Option<String>,
    pub confidence: f32,
    pub status: MemoryCandidateStatus,
    pub content_hash: String,
    pub review_after_days: u32,
}

impl Default for MemoryCandidate {
    fn default() -> Self {
        Self {
            candidate_id: String::new(),
            source: MemorySource::default(),
            title: String::new(),
            content: String::new(),
            tags: Vec::new(),
            kind: "domain".to_string(),
            filenames: Vec::new(),
            related_files: Vec::new(),
            source_id: None,
            confidence: 0.0,
            status: MemoryCandidateStatus::Proposed,
            content_hash: String::new(),
            review_after_days: 0,
        }
    }
}

impl MemoryCandidate {
    pub fn normalized(mut self) -> Self {
        self.tags = normalize_tags(&self.tags);
        self.filenames = normalize_paths(&self.filenames);
        self.related_files = normalize_paths(&self.related_files);
        self.kind = normalize_kind(&self.kind);
        self.source_id = normalize_optional_string(self.source_id.as_deref());
        if self.content_hash.trim().is_empty() {
            self.content_hash = compute_content_hash(&self.content);
        }
        if self.review_after_days == 0 {
            self.review_after_days =
                default_review_after_days(&self.kind, self.source, self.status);
        }
        self
    }

    pub fn idempotency_input(&self, op_type: MemoryOpType) -> MemoryOpIdempotencyInput {
        MemoryOpIdempotencyInput {
            source: self.source,
            op_type,
            target_paths: self.filenames.clone(),
            tags: self.tags.clone(),
            kind: Some(self.kind.clone()),
            source_id: self.source_id.clone(),
            title: Some(self.title.clone()),
            content: Some(self.content.clone()),
            evidence: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryOpIdempotencyInput {
    pub source: MemorySource,
    pub op_type: MemoryOpType,
    pub target_paths: Vec<String>,
    pub tags: Vec<String>,
    pub kind: Option<String>,
    pub source_id: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub evidence: Option<String>,
}

impl Default for MemoryOpIdempotencyInput {
    fn default() -> Self {
        Self {
            source: MemorySource::default(),
            op_type: MemoryOpType::default(),
            target_paths: Vec::new(),
            tags: Vec::new(),
            kind: None,
            source_id: None,
            title: None,
            content: None,
            evidence: None,
        }
    }
}

impl MemoryOpIdempotencyInput {
    pub fn normalized(&self) -> Self {
        Self {
            source: self.source,
            op_type: self.op_type,
            target_paths: normalize_paths(&self.target_paths),
            tags: normalize_tags(&self.tags),
            kind: self.kind.as_deref().map(normalize_kind),
            source_id: normalize_optional_string(self.source_id.as_deref()),
            title: normalize_optional_text(self.title.as_deref()),
            content: normalize_optional_hash_text(self.content.as_deref()),
            evidence: normalize_optional_text(self.evidence.as_deref()),
        }
    }
}

pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = tags
        .iter()
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

pub fn normalize_paths(paths: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = paths
        .iter()
        .filter_map(|path| normalize_path(path))
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

pub fn normalize_path(path: &str) -> Option<String> {
    let path = path.trim().replace('\\', "/");
    if path.is_empty() {
        return None;
    }

    let bytes = path.as_bytes();
    let drive = if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        Some((bytes[0] as char).to_ascii_uppercase())
    } else {
        None
    };

    if let Some(drive) = drive {
        let rest = &path[2..];
        let absolute = rest.starts_with('/');
        let parts = normalize_path_parts(rest);
        return Some(match (absolute, parts.is_empty()) {
            (true, true) => format!("{}:/", drive),
            (true, false) => format!("{}:/{}", drive, parts.join("/")),
            (false, true) => format!("{}:", drive),
            (false, false) => format!("{}:{}", drive, parts.join("/")),
        });
    }

    let unc = path.starts_with("//");
    let absolute = path.starts_with('/') && !unc;
    let parts = normalize_path_parts(if unc { &path[2..] } else { &path });

    if unc {
        if parts.is_empty() {
            Some("//".to_string())
        } else {
            Some(format!("//{}", parts.join("/")))
        }
    } else if absolute {
        if parts.is_empty() {
            Some("/".to_string())
        } else {
            Some(format!("/{}", parts.join("/")))
        }
    } else if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

pub fn normalize_kind(kind: &str) -> String {
    let normalized = kind
        .trim()
        .to_lowercase()
        .replace('-', "_")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_");
    if normalized.is_empty() {
        "domain".to_string()
    } else {
        normalized
    }
}

pub fn normalize_idempotency_input(input: &MemoryOpIdempotencyInput) -> MemoryOpIdempotencyInput {
    input.normalized()
}

pub fn compute_content_hash(content: &str) -> String {
    let mut h = Sha256::new();
    h.update(normalize_hash_text(content).as_bytes());
    hex::encode(h.finalize())
}

pub fn compute_idempotency_key(input: &MemoryOpIdempotencyInput) -> String {
    let normalized = input.normalized();
    let content_hash = normalized
        .content
        .as_deref()
        .map(compute_content_hash)
        .unwrap_or_default();
    let evidence_hash = normalized
        .evidence
        .as_deref()
        .map(compute_content_hash)
        .unwrap_or_default();
    let mut h = Sha256::new();
    hash_field(&mut h, "source", normalized.source.as_str());
    hash_field(&mut h, "op_type", normalized.op_type.as_str());
    hash_list(&mut h, "target_path", &normalized.target_paths);
    hash_list(&mut h, "tag", &normalized.tags);
    hash_field(&mut h, "kind", normalized.kind.as_deref().unwrap_or(""));
    hash_field(
        &mut h,
        "source_id",
        normalized.source_id.as_deref().unwrap_or(""),
    );
    hash_field(&mut h, "title", normalized.title.as_deref().unwrap_or(""));
    hash_field(&mut h, "content_hash", &content_hash);
    hash_field(&mut h, "evidence_hash", &evidence_hash);
    format!("memop_{}", hex::encode(h.finalize()))
}

pub fn default_requires_approval(op_type: MemoryOpType, confidence: f32) -> bool {
    match op_type {
        MemoryOpType::ArchiveCandidate
        | MemoryOpType::Archive
        | MemoryOpType::MergeArchive
        | MemoryOpType::DeleteCandidate => true,
        MemoryOpType::CreateMemory | MemoryOpType::Retag | MemoryOpType::RepairLinks => {
            confidence < HIGH_CONFIDENCE_APPROVAL_THRESHOLD
        }
        _ => true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryApplyOutcome {
    pub status: MemoryOpStatus,
    pub applied_paths: Vec<PathBuf>,
    pub message: Option<String>,
}

impl MemoryApplyOutcome {
    fn applied(paths: Vec<PathBuf>) -> Self {
        Self {
            status: MemoryOpStatus::Applied,
            applied_paths: paths,
            message: None,
        }
    }

    fn skipped(message: impl Into<String>) -> Self {
        Self {
            status: MemoryOpStatus::Skipped,
            applied_paths: Vec::new(),
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone)]
struct KnowledgeRoots {
    local: Vec<PathBuf>,
    global: PathBuf,
}

impl KnowledgeRoots {
    fn all(&self) -> Vec<PathBuf> {
        let mut roots = self.local.clone();
        roots.push(self.global.clone());
        roots
    }
}

pub async fn apply_memory_lifecycle_op(
    gcx: Arc<ARwLock<GlobalContext>>,
    op: &MemoryLifecycleOp,
) -> Result<MemoryApplyOutcome, String> {
    let op = op.clone().normalized();
    if matches!(
        op.status,
        MemoryOpStatus::Applied | MemoryOpStatus::Skipped | MemoryOpStatus::Rejected
    ) {
        return Ok(MemoryApplyOutcome::skipped("operation already finalized"));
    }
    if op.requires_approval && op.status != MemoryOpStatus::Approved {
        return Err("operation requires approval".to_string());
    }
    if destructive_memory_op(op.op_type) && op.status != MemoryOpStatus::Approved {
        return Err("archive, delete, and merge operations require approval".to_string());
    }

    match op.op_type {
        MemoryOpType::CreateMemory => apply_create_memory(gcx, &op).await,
        MemoryOpType::Retag => apply_retag(gcx, &op).await,
        MemoryOpType::RepairLinks => apply_repair_links(gcx, &op).await,
        MemoryOpType::MarkReviewNeeded => apply_review_status(gcx, &op, "review_needed").await,
        MemoryOpType::MarkStale => apply_review_status(gcx, &op, "stale").await,
        MemoryOpType::Archive | MemoryOpType::ArchiveCandidate => {
            apply_archive(gcx, &op, None).await
        }
        MemoryOpType::MergeArchive => apply_merge_archive(gcx, &op).await,
        MemoryOpType::DeleteCandidate => {
            Err("hard delete is not supported by memory lifecycle applier".to_string())
        }
        _ => Err(format!(
            "unsupported memory lifecycle operation: {}",
            op.op_type.as_str()
        )),
    }
}

pub async fn apply_memory_lifecycle_op_status(
    gcx: Arc<ARwLock<GlobalContext>>,
    op: &MemoryLifecycleOp,
) -> MemoryLifecycleOp {
    let mut updated = op.clone().normalized();
    match apply_memory_lifecycle_op(gcx, &updated).await {
        Ok(outcome) => {
            updated.status = outcome.status;
            updated.error = outcome.message;
            if updated.status == MemoryOpStatus::Applied {
                updated.applied_at = Some(Utc::now().to_rfc3339());
            }
        }
        Err(err) => {
            updated.status = MemoryOpStatus::Failed;
            updated.error = Some(err);
        }
    }
    updated
}

fn destructive_memory_op(op_type: MemoryOpType) -> bool {
    matches!(
        op_type,
        MemoryOpType::ArchiveCandidate
            | MemoryOpType::Archive
            | MemoryOpType::MergeArchive
            | MemoryOpType::DeleteCandidate
    )
}

async fn apply_create_memory(
    gcx: Arc<ARwLock<GlobalContext>>,
    op: &MemoryLifecycleOp,
) -> Result<MemoryApplyOutcome, String> {
    let payload = op
        .payload
        .canonical
        .clone()
        .unwrap_or_else(|| MemoryCreatePayload {
            title: op.payload.title.clone(),
            content: op
                .payload
                .content
                .clone()
                .unwrap_or_else(|| op.evidence.clone()),
            tags: op.payload.tags.clone().unwrap_or_default(),
            kind: op
                .payload
                .kind
                .clone()
                .unwrap_or_else(|| "domain".to_string()),
            filenames: op.payload.filenames.clone().unwrap_or_default(),
            related_files: op.payload.related_files.clone().unwrap_or_default(),
            links: op.payload.links.clone().unwrap_or_default(),
            review_after: op.payload.review_after.clone(),
        })
        .normalized();

    let content = if payload.content.trim().is_empty() {
        return Err("create_memory payload content is empty".to_string());
    } else {
        payload.content.trim().to_string()
    };

    let mut tags = payload.tags.clone();
    if tags.is_empty() {
        tags.push("memory".to_string());
    }
    let links = payload.links.clone();
    let mut frontmatter = create_frontmatter(
        payload.title.as_deref(),
        &tags,
        &payload.filenames,
        &links,
        &payload.kind,
    );
    frontmatter.related_files = payload.related_files;
    frontmatter.status = Some(
        if op.source.is_autonomous()
            && !(op.status == MemoryOpStatus::Approved
                || (!op.requires_approval && op.confidence >= HIGH_CONFIDENCE_APPROVAL_THRESHOLD))
        {
            "proposed".to_string()
        } else {
            "active".to_string()
        },
    );
    if let Some(review_after) = payload.review_after {
        frontmatter.review_after = Some(review_after);
    }
    frontmatter.source_tool = Some(format!("buddy_memory_lifecycle:{}", op.source.as_str()));
    frontmatter.content_hash = Some(compute_content_hash(&content));

    let path = memories_add(gcx, &frontmatter, &content).await?;
    Ok(MemoryApplyOutcome::applied(vec![path]))
}

async fn apply_retag(
    gcx: Arc<ARwLock<GlobalContext>>,
    op: &MemoryLifecycleOp,
) -> Result<MemoryApplyOutcome, String> {
    let tags = op
        .payload
        .tags
        .clone()
        .ok_or_else(|| "retag payload missing tags".to_string())?;
    let roots = knowledge_roots(gcx.clone()).await;
    let mut paths = Vec::new();
    for target in &op.target_paths {
        let path = validate_existing_memory_path(target, &roots).await?;
        let changed = update_memory_document_frontmatter(gcx.clone(), &path, |frontmatter| {
            let new_tags = normalize_memory_tags(&tags, 16);
            if frontmatter.tags == new_tags {
                return Ok(false);
            }
            frontmatter.tags = new_tags;
            frontmatter.updated = Some(today_string());
            Ok(true)
        })
        .await?;
        if changed {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        Ok(MemoryApplyOutcome::skipped("retag already applied"))
    } else {
        Ok(MemoryApplyOutcome::applied(paths))
    }
}

async fn apply_repair_links(
    gcx: Arc<ARwLock<GlobalContext>>,
    op: &MemoryLifecycleOp,
) -> Result<MemoryApplyOutcome, String> {
    if op.payload.filenames.is_none()
        && op.payload.related_files.is_none()
        && op.payload.links.is_none()
    {
        return Err("repair_links payload has no link fields".to_string());
    }
    let roots = knowledge_roots(gcx.clone()).await;
    let mut paths = Vec::new();
    for target in &op.target_paths {
        let path = validate_existing_memory_path(target, &roots).await?;
        let changed = update_memory_document_frontmatter(gcx.clone(), &path, |frontmatter| {
            let old = (
                frontmatter.filenames.clone(),
                frontmatter.related_files.clone(),
                frontmatter.links.clone(),
            );
            if let Some(filenames) = &op.payload.filenames {
                frontmatter.filenames = filenames.clone();
            }
            if let Some(related_files) = &op.payload.related_files {
                frontmatter.related_files = related_files.clone();
            }
            if let Some(links) = &op.payload.links {
                frontmatter.links = links.clone();
            }
            let new = (
                frontmatter.filenames.clone(),
                frontmatter.related_files.clone(),
                frontmatter.links.clone(),
            );
            if old == new {
                return Ok(false);
            }
            frontmatter.updated = Some(today_string());
            Ok(true)
        })
        .await?;
        if changed {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        Ok(MemoryApplyOutcome::skipped("links already repaired"))
    } else {
        Ok(MemoryApplyOutcome::applied(paths))
    }
}

async fn apply_review_status(
    gcx: Arc<ARwLock<GlobalContext>>,
    op: &MemoryLifecycleOp,
    status: &str,
) -> Result<MemoryApplyOutcome, String> {
    let roots = knowledge_roots(gcx.clone()).await;
    let review_after = op.payload.review_after.clone().unwrap_or_else(today_string);
    let mut paths = Vec::new();
    for target in &op.target_paths {
        let path = validate_existing_memory_path(target, &roots).await?;
        let changed = update_memory_document_frontmatter(gcx.clone(), &path, |frontmatter| {
            if frontmatter.status.as_deref() == Some(status)
                && frontmatter.review_after.as_deref() == Some(review_after.as_str())
            {
                return Ok(false);
            }
            frontmatter.status = Some(status.to_string());
            frontmatter.review_after = Some(review_after.clone());
            frontmatter.updated = Some(today_string());
            Ok(true)
        })
        .await?;
        if changed {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        Ok(MemoryApplyOutcome::skipped("review status already applied"))
    } else {
        Ok(MemoryApplyOutcome::applied(paths))
    }
}

async fn apply_archive(
    gcx: Arc<ARwLock<GlobalContext>>,
    op: &MemoryLifecycleOp,
    superseded_by: Option<&str>,
) -> Result<MemoryApplyOutcome, String> {
    let roots = knowledge_roots(gcx.clone()).await;
    let mut paths = Vec::new();
    for target in &op.target_paths {
        let path = validate_existing_memory_path(target, &roots).await?;
        let changed = archive_memory_file(
            gcx.clone(),
            &path,
            superseded_by.or(op.payload.superseded_by.as_deref()),
        )
        .await?;
        if changed {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        Ok(MemoryApplyOutcome::skipped("memory already archived"))
    } else {
        Ok(MemoryApplyOutcome::applied(paths))
    }
}

async fn apply_merge_archive(
    gcx: Arc<ARwLock<GlobalContext>>,
    op: &MemoryLifecycleOp,
) -> Result<MemoryApplyOutcome, String> {
    if op.status != MemoryOpStatus::Approved {
        return Err("merge_archive requires approval".to_string());
    }

    let canonical = op
        .payload
        .canonical
        .clone()
        .ok_or_else(|| "merge_archive payload missing canonical memory".to_string())?
        .normalized();
    if canonical.content.trim().is_empty() {
        return Err("merge_archive canonical content is empty".to_string());
    }

    let roots = knowledge_roots(gcx.clone()).await;
    let superseded_targets = if op.payload.superseded_paths.is_empty() {
        op.target_paths.clone()
    } else {
        op.payload.superseded_paths.clone()
    };
    let mut superseded_paths = Vec::new();
    for target in &superseded_targets {
        superseded_paths.push(validate_existing_memory_path(target, &roots).await?);
    }

    let mut tags = canonical.tags.clone();
    if tags.is_empty() {
        tags.push("memory".to_string());
    }
    let mut frontmatter = create_frontmatter(
        canonical.title.as_deref(),
        &tags,
        &canonical.filenames,
        &canonical.links,
        &canonical.kind,
    );
    frontmatter.related_files = canonical.related_files;
    frontmatter.content_hash = Some(compute_content_hash(&canonical.content));
    if let Some(review_after) = canonical.review_after {
        frontmatter.review_after = Some(review_after);
    }
    frontmatter.source_tool = Some(format!("buddy_memory_lifecycle:{}", op.source.as_str()));

    let canonical_path = memories_add(gcx.clone(), &frontmatter, canonical.content.trim()).await?;
    let canonical_id = frontmatter
        .id
        .clone()
        .unwrap_or_else(|| canonical_path.to_string_lossy().to_string());

    let mut paths = vec![canonical_path];
    for path in superseded_paths {
        let changed = archive_memory_file(gcx.clone(), &path, Some(&canonical_id)).await?;
        if changed {
            paths.push(path);
        }
    }

    Ok(MemoryApplyOutcome::applied(paths))
}

async fn archive_memory_file(
    gcx: Arc<ARwLock<GlobalContext>>,
    path: &Path,
    superseded_by: Option<&str>,
) -> Result<bool, String> {
    update_memory_document_frontmatter(gcx, path, |frontmatter| {
        if frontmatter.is_archived() {
            return Ok(false);
        }
        frontmatter.status = Some("archived".to_string());
        frontmatter.deprecated_at = Some(today_string());
        frontmatter.updated = Some(today_string());
        if let Some(superseded_by) = superseded_by {
            frontmatter.superseded_by = Some(superseded_by.to_string());
        }
        Ok(true)
    })
    .await
}

async fn knowledge_roots(gcx: Arc<ARwLock<GlobalContext>>) -> KnowledgeRoots {
    let local = get_project_dirs(gcx.clone())
        .await
        .into_iter()
        .map(|dir| dir.join(KNOWLEDGE_FOLDER_NAME))
        .collect();
    let global = get_global_knowledge_dir(gcx).await;
    KnowledgeRoots { local, global }
}

async fn validate_existing_memory_path(
    path: &str,
    roots: &KnowledgeRoots,
) -> Result<PathBuf, String> {
    let normalized = normalize_path(path).ok_or_else(|| "empty memory path".to_string())?;
    reject_unsafe_path(&normalized)?;
    let candidate = PathBuf::from(&normalized);
    validate_memory_extension(&candidate)?;

    let absolute_candidate = if candidate.is_absolute() {
        candidate
    } else {
        let roots_all = roots.all();
        let mut resolved = None;
        for root in &roots_all {
            if normalized.starts_with(&format!("{}/", KNOWLEDGE_FOLDER_NAME)) {
                if let Some(parent) = root.parent() {
                    let candidate = parent.join(&normalized);
                    if candidate.exists() {
                        resolved = Some(candidate);
                        break;
                    }
                }
            }
            let candidate = root.join(&normalized);
            if candidate.exists() {
                resolved = Some(candidate);
                break;
            }
        }
        resolved.ok_or_else(|| format!("memory path not found: {}", normalized))?
    };

    let canonical = canonical_existing_file_no_symlink(&absolute_candidate).await?;
    let canonical_roots = canonicalize_existing_roots(roots).await?;
    if !canonical_roots
        .iter()
        .any(|root| canonical.starts_with(root))
    {
        return Err("memory path outside knowledge directories".to_string());
    }
    Ok(canonical)
}

fn reject_unsafe_path(path: &str) -> Result<(), String> {
    if path.contains('\0') {
        return Err("memory path contains nul byte".to_string());
    }
    let parsed = Path::new(path);
    for component in parsed.components() {
        match component {
            Component::ParentDir => return Err("memory path cannot contain '..'".to_string()),
            Component::Prefix(_) => {
                return Err("windows drive prefixes are not allowed".to_string())
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_memory_extension(path: &Path) -> Result<(), String> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("md") | Some("mdx") => Ok(()),
        _ => Err("memory path must be .md or .mdx".to_string()),
    }
}

async fn canonical_existing_file_no_symlink(path: &Path) -> Result<PathBuf, String> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(|e| format!("memory path not accessible: {}", e))?;
    if metadata.file_type().is_symlink() {
        return Err("memory path cannot be a symlink".to_string());
    }
    if !metadata.is_file() {
        return Err("memory path must be a file".to_string());
    }
    tokio::fs::canonicalize(path)
        .await
        .map(|path| dunce::simplified(&path).to_path_buf())
        .map_err(|e| format!("failed to canonicalize memory path: {}", e))
}

async fn canonicalize_existing_roots(roots: &KnowledgeRoots) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for root in roots.all() {
        if !root.exists() {
            continue;
        }
        let metadata = tokio::fs::symlink_metadata(&root)
            .await
            .map_err(|e| format!("knowledge root inaccessible: {}", e))?;
        if metadata.file_type().is_symlink() {
            return Err("knowledge root cannot be a symlink".to_string());
        }
        let canonical = tokio::fs::canonicalize(&root)
            .await
            .map(|path| dunce::simplified(&path).to_path_buf())
            .map_err(|e| format!("failed to canonicalize knowledge root: {}", e))?;
        out.push(canonical);
    }
    if out.is_empty() {
        return Err("no knowledge directories available".to_string());
    }
    Ok(out)
}

fn normalize_strings(values: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = values
        .iter()
        .filter_map(|value| normalize_optional_string(Some(value)))
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalize_review_after(value: Option<&str>) -> Option<String> {
    let value = normalize_optional_string(value)?;
    NaiveDate::parse_from_str(&value, "%Y-%m-%d")
        .ok()
        .map(|date| date.format("%Y-%m-%d").to_string())
}

fn today_string() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn redact_and_cap_payload_text(text: &str, max_chars: usize) -> String {
    let redacted = crate::buddy::actor::redact_sensitive(text);
    crate::llm::safe_truncate(&redacted, max_chars)
        .trim()
        .to_string()
}

pub fn default_review_after_days(
    kind: &str,
    source: MemorySource,
    status: MemoryCandidateStatus,
) -> u32 {
    let base = match normalize_kind(kind).as_str() {
        "decision" | "decisions" | "architecture" | "code" => 180,
        "preference" => 365,
        "task" | "task_report" | "task_summary" | "task_report_summary" => 30,
        "research" | "research_note" | "domain" | "trajectory" => 90,
        "digest" | "summary" => 60,
        _ => 90,
    };
    let source_adjusted = match source {
        MemorySource::Manual => base,
        MemorySource::Git => base.min(120),
        MemorySource::Trajectory => base.min(90),
        MemorySource::BehaviorLearner => base.min(60),
        MemorySource::MemoryGarden
        | MemorySource::KnowledgeConflictResolver
        | MemorySource::Buddy => base.min(75),
    };
    if status == MemoryCandidateStatus::Proposed && source.is_autonomous() {
        source_adjusted.min(30)
    } else {
        source_adjusted
    }
}

pub fn default_review_after_date(
    created: chrono::NaiveDate,
    kind: &str,
    source: MemorySource,
    status: MemoryCandidateStatus,
) -> String {
    let days = default_review_after_days(kind, source, status) as i64;
    (created + chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

fn normalize_path_parts(path: &str) -> Vec<String> {
    let mut parts = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            parts.push(part.to_string());
            continue;
        }
        parts.push(part.to_string());
    }
    parts
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|value| !value.is_empty())
}

fn normalize_optional_hash_text(value: Option<&str>) -> Option<String> {
    value
        .map(normalize_hash_text)
        .filter(|value| !value.is_empty())
}

fn normalize_hash_text(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn hash_field(h: &mut Sha256, name: &str, value: &str) {
    h.update(name.as_bytes());
    h.update([0]);
    h.update(value.as_bytes());
    h.update([0]);
}

fn hash_list(h: &mut Sha256, name: &str, values: &[String]) {
    for value in values {
        hash_field(h, name, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_graph::kg_structs::KnowledgeFrontmatter;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn test_op(op_id: &str, evidence: &str, status: MemoryOpStatus) -> MemoryLifecycleOp {
        let mut op = MemoryLifecycleOp::pending(
            op_id,
            MemorySource::MemoryGarden,
            MemoryOpType::CreateMemory,
            strings(&[".refact/knowledge/item.md"]),
            evidence,
            0.91,
            "2026-05-02T00:00:00Z",
        );
        op.status = status;
        op
    }

    async fn test_gcx_with_workspace(dir: &Path) -> Arc<ARwLock<GlobalContext>> {
        let gcx = crate::global_context::tests::make_test_gcx().await;
        {
            let gcx_lock = gcx.read().await;
            *gcx_lock.documents_state.workspace_folders.lock().unwrap() = vec![dir.to_path_buf()];
        }
        gcx
    }

    fn frontmatter_and_body(text: &str) -> (KnowledgeFrontmatter, String) {
        let (frontmatter, content_start) = KnowledgeFrontmatter::parse(text);
        (frontmatter, text[content_start..].trim().to_string())
    }

    async fn write_memory_file(path: &Path, frontmatter: KnowledgeFrontmatter, body: &str) {
        tokio::fs::write(path, format!("{}\n\n{}", frontmatter.to_yaml(), body))
            .await
            .unwrap();
    }

    fn active_frontmatter(title: &str, tags: &[&str]) -> KnowledgeFrontmatter {
        KnowledgeFrontmatter {
            id: Some(title.to_string()),
            title: Some(title.to_string()),
            status: Some("active".to_string()),
            tags: strings(tags),
            created: Some("2026-05-02".to_string()),
            updated: Some("2026-05-02".to_string()),
            kind: Some("domain".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn serde_roundtrip_every_source_op_and_status_variant() {
        let sources = [
            MemorySource::Buddy,
            MemorySource::Trajectory,
            MemorySource::Git,
            MemorySource::Manual,
            MemorySource::BehaviorLearner,
            MemorySource::MemoryGarden,
            MemorySource::KnowledgeConflictResolver,
        ];
        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            assert_eq!(serde_json::from_str::<MemorySource>(&json).unwrap(), source);
        }

        let op_types = [
            MemoryOpType::CreateMemory,
            MemoryOpType::UpdateMemory,
            MemoryOpType::Retag,
            MemoryOpType::RepairLinks,
            MemoryOpType::Refresh,
            MemoryOpType::ArchiveCandidate,
            MemoryOpType::Archive,
            MemoryOpType::MergeArchive,
            MemoryOpType::DeleteCandidate,
            MemoryOpType::PromoteDigest,
            MemoryOpType::MarkReviewNeeded,
            MemoryOpType::MarkStale,
        ];
        for op_type in op_types {
            let json = serde_json::to_string(&op_type).unwrap();
            assert_eq!(
                serde_json::from_str::<MemoryOpType>(&json).unwrap(),
                op_type
            );
        }

        let statuses = [
            MemoryOpStatus::Pending,
            MemoryOpStatus::Approved,
            MemoryOpStatus::Applied,
            MemoryOpStatus::Rejected,
            MemoryOpStatus::Failed,
            MemoryOpStatus::Skipped,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(
                serde_json::from_str::<MemoryOpStatus>(&json).unwrap(),
                status
            );
        }

        let op = MemoryLifecycleOp::pending(
            "op-1",
            MemorySource::MemoryGarden,
            MemoryOpType::Retag,
            strings(&["src//lib.rs", "src/lib.rs"]),
            "Memory tags were stale",
            0.91,
            "2026-05-02T00:00:00Z",
        );
        let json = serde_json::to_string(&op).unwrap();
        assert_eq!(
            serde_json::from_str::<MemoryLifecycleOp>(&json).unwrap(),
            op
        );
    }

    #[test]
    fn idempotency_key_is_stable_for_tag_and_path_order() {
        let first = MemoryOpIdempotencyInput {
            source: MemorySource::Trajectory,
            op_type: MemoryOpType::CreateMemory,
            target_paths: strings(&["src//buddy/memory_lifecycle.rs", "README.md"]),
            tags: strings(&[" Buddy ", "Memory", "buddy"]),
            kind: Some(" Research Note ".to_string()),
            source_id: Some(" trajectory-1 ".to_string()),
            title: Some("  Useful discovery  ".to_string()),
            content: Some("Line one\r\nLine two\n".to_string()),
            evidence: Some(" observed in trajectory ".to_string()),
        };
        let second = MemoryOpIdempotencyInput {
            source: MemorySource::Trajectory,
            op_type: MemoryOpType::CreateMemory,
            target_paths: strings(&["README.md", "src/buddy/memory_lifecycle.rs"]),
            tags: strings(&["memory", "buddy"]),
            kind: Some("research_note".to_string()),
            source_id: Some("trajectory-1".to_string()),
            title: Some("Useful discovery".to_string()),
            content: Some("Line one\nLine two".to_string()),
            evidence: Some("observed in trajectory".to_string()),
        };

        assert_eq!(
            compute_idempotency_key(&first),
            compute_idempotency_key(&second)
        );
    }

    #[test]
    fn path_normalization_handles_unix_relative_and_windows_inputs() {
        assert_eq!(
            normalize_path("/tmp//project/./src/lib.rs"),
            Some("/tmp/project/src/lib.rs".to_string())
        );
        assert_eq!(
            normalize_path(" ./relative//path/ "),
            Some("relative/path".to_string())
        );
        assert_eq!(
            normalize_path("../outside.md"),
            Some("../outside.md".to_string())
        );
        assert_eq!(
            normalize_path("src\\buddy//memory_lifecycle.rs"),
            Some("src/buddy/memory_lifecycle.rs".to_string())
        );
        assert_eq!(
            normalize_path("c:\\Users\\Ada\\project\\file.md"),
            Some("C:/Users/Ada/project/file.md".to_string())
        );
        assert_eq!(
            normalize_paths(&strings(&["b//c", "a\\d", "b/c", ""])),
            strings(&["a/d", "b/c"])
        );
    }

    #[test]
    fn tag_normalization_trims_lowercases_sorts_and_dedupes() {
        assert_eq!(
            normalize_tags(&strings(&[" Buddy ", "memory", "", "MEMORY", "alpha"])),
            strings(&["alpha", "buddy", "memory"])
        );
    }

    #[test]
    fn default_approval_policy_requires_destructive_and_allows_high_confidence_safe_ops() {
        assert!(default_requires_approval(MemoryOpType::Archive, 0.99));
        assert!(default_requires_approval(
            MemoryOpType::ArchiveCandidate,
            0.99
        ));
        assert!(default_requires_approval(MemoryOpType::MergeArchive, 0.99));
        assert!(default_requires_approval(
            MemoryOpType::DeleteCandidate,
            0.99
        ));

        assert!(!default_requires_approval(MemoryOpType::CreateMemory, 0.90));
        assert!(!default_requires_approval(MemoryOpType::Retag, 0.90));
        assert!(!default_requires_approval(MemoryOpType::RepairLinks, 0.90));
        assert!(default_requires_approval(MemoryOpType::CreateMemory, 0.70));
        assert!(default_requires_approval(MemoryOpType::UpdateMemory, 0.95));
    }

    #[test]
    fn review_after_policy_varies_by_kind_source_and_status() {
        let manual_code = default_review_after_days(
            "code",
            MemorySource::Manual,
            MemoryCandidateStatus::Promoted,
        );
        let manual_research = default_review_after_days(
            "research",
            MemorySource::Manual,
            MemoryCandidateStatus::Promoted,
        );
        let manual_task = default_review_after_days(
            "task_report",
            MemorySource::Manual,
            MemoryCandidateStatus::Promoted,
        );
        let proposed_auto_code = default_review_after_days(
            "code",
            MemorySource::BehaviorLearner,
            MemoryCandidateStatus::Proposed,
        );

        assert!(manual_code > manual_research);
        assert!(manual_research > manual_task);
        assert!(proposed_auto_code < manual_code);
        assert_eq!(proposed_auto_code, 30);
        assert_eq!(
            default_review_after_date(
                chrono::NaiveDate::from_ymd_opt(2026, 5, 2).unwrap(),
                "task_report",
                MemorySource::Manual,
                MemoryCandidateStatus::Promoted,
            ),
            "2026-06-01"
        );
    }

    #[test]
    fn memory_ops_state_preserves_first_seen_order() {
        let first = test_op("op-1", "first", MemoryOpStatus::Pending);
        let second = test_op("op-2", "second", MemoryOpStatus::Approved);
        let state = MemoryOpsState::from_records(vec![
            MemoryOpsRecord::Op { op: first.clone() },
            MemoryOpsRecord::Op { op: second.clone() },
        ]);

        assert_eq!(state.ops, vec![first.normalized(), second.normalized()]);
        assert_eq!(state.pending_count, 1);
        assert_eq!(state.approved_count, 1);
    }

    #[test]
    fn memory_ops_state_duplicate_idempotency_key_uses_latest_slot() {
        let first = test_op("op-1", "same", MemoryOpStatus::Pending);
        let mut second = test_op("op-2", "same", MemoryOpStatus::Applied);
        second.idempotency_key = first.idempotency_key.clone();

        let state = MemoryOpsState::from_records(vec![
            MemoryOpsRecord::Op { op: first },
            MemoryOpsRecord::Op { op: second.clone() },
        ]);

        assert_eq!(state.ops.len(), 1);
        assert_eq!(state.ops[0].op_id, "op-2");
        assert_eq!(state.ops[0].status, MemoryOpStatus::Applied);
        assert_eq!(state.applied_count, 1);
    }

    #[test]
    fn memory_ops_state_compaction_records_latest_per_op_and_key() {
        let first = test_op("op-1", "first", MemoryOpStatus::Pending);
        let mut second = first.clone();
        second.status = MemoryOpStatus::Failed;
        second.error = Some("write failed".to_string());
        let third = test_op("op-2", "second", MemoryOpStatus::Applied);

        let state = MemoryOpsState::from_records(vec![
            MemoryOpsRecord::Op { op: first },
            MemoryOpsRecord::Op { op: second.clone() },
            MemoryOpsRecord::Op { op: third.clone() },
        ]);
        let compacted = MemoryOpsState::from_records(state.canonical_records());

        assert_eq!(compacted.ops, vec![second.normalized(), third.normalized()]);
        assert_eq!(compacted.failed_count, 1);
        assert_eq!(compacted.applied_count, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_memory_op_writes_frontmatter_body_with_normalized_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let gcx = test_gcx_with_workspace(dir.path()).await;
        let mut op = MemoryLifecycleOp::pending(
            "op-create",
            MemorySource::BehaviorLearner,
            MemoryOpType::CreateMemory,
            Vec::new(),
            "evidence",
            0.91,
            "2026-05-02T00:00:00Z",
        );
        op.payload = MemoryLifecyclePayload {
            title: Some(" Useful Memory ".to_string()),
            content: Some("# Useful Memory\n\nBody".to_string()),
            tags: Some(strings(&[" Buddy ", "MEMORY", "buddy"])),
            kind: Some("Preference".to_string()),
            filenames: Some(strings(&["src//lib.rs"])),
            related_files: Some(strings(&["src/main.rs"])),
            ..Default::default()
        };

        let outcome = apply_memory_lifecycle_op(gcx, &op).await.unwrap();

        assert_eq!(outcome.status, MemoryOpStatus::Applied);
        assert_eq!(outcome.applied_paths.len(), 1);
        let text = tokio::fs::read_to_string(&outcome.applied_paths[0])
            .await
            .unwrap();
        let (frontmatter, body) = frontmatter_and_body(&text);
        assert_eq!(frontmatter.title.as_deref(), Some("Useful Memory"));
        assert_eq!(frontmatter.tags, strings(&["buddy", "memory"]));
        assert_eq!(frontmatter.kind.as_deref(), Some("preference"));
        assert_eq!(frontmatter.status.as_deref(), Some("active"));
        assert_eq!(frontmatter.filenames, strings(&["src/lib.rs"]));
        assert_eq!(frontmatter.related_files, strings(&["src/main.rs"]));
        assert_eq!(body, "# Useful Memory\n\nBody");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn autonomous_create_memory_defaults_to_proposed_without_approval() {
        let dir = tempfile::tempdir().unwrap();
        let gcx = test_gcx_with_workspace(dir.path()).await;
        let mut op = MemoryLifecycleOp::pending(
            "op-create-proposed",
            MemorySource::MemoryGarden,
            MemoryOpType::CreateMemory,
            Vec::new(),
            "Unapproved autonomous evidence",
            0.50,
            "2026-05-02T00:00:00Z",
        );
        op.requires_approval = false;
        op.payload.content = Some("Autonomous body".to_string());

        let outcome = apply_memory_lifecycle_op(gcx, &op).await.unwrap();
        let text = tokio::fs::read_to_string(&outcome.applied_paths[0])
            .await
            .unwrap();
        let (frontmatter, _) = frontmatter_and_body(&text);

        assert_eq!(frontmatter.status.as_deref(), Some("proposed"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn retag_and_repair_links_preserve_body_and_parseable_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let gcx = test_gcx_with_workspace(dir.path()).await;
        let knowledge_dir = dir.path().join(KNOWLEDGE_FOLDER_NAME);
        tokio::fs::create_dir_all(&knowledge_dir).await.unwrap();
        let path = knowledge_dir.join("memory.md");
        let mut frontmatter = active_frontmatter("memory", &["old"]);
        frontmatter.filenames = strings(&["old.rs"]);
        frontmatter.related_files = strings(&["old-related.rs"]);
        frontmatter.links = strings(&["old-link"]);
        write_memory_file(&path, frontmatter, "# Heading\n\nOriginal body").await;

        let mut retag = MemoryLifecycleOp::pending(
            "op-retag",
            MemorySource::MemoryGarden,
            MemoryOpType::Retag,
            vec![path.to_string_lossy().to_string()],
            "retag",
            0.91,
            "2026-05-02T00:00:00Z",
        );
        retag.payload.tags = Some(strings(&["New", "buddy"]));
        apply_memory_lifecycle_op(gcx.clone(), &retag)
            .await
            .unwrap();

        let mut repair = MemoryLifecycleOp::pending(
            "op-repair",
            MemorySource::MemoryGarden,
            MemoryOpType::RepairLinks,
            vec![path.to_string_lossy().to_string()],
            "repair",
            0.91,
            "2026-05-02T00:00:00Z",
        );
        repair.payload.filenames = Some(strings(&["src/lib.rs"]));
        repair.payload.related_files = Some(strings(&["src/main.rs"]));
        repair.payload.links = Some(strings(&["new-link"]));
        apply_memory_lifecycle_op(gcx, &repair).await.unwrap();

        let text = tokio::fs::read_to_string(&path).await.unwrap();
        let (frontmatter, body) = frontmatter_and_body(&text);
        assert_eq!(frontmatter.tags, strings(&["buddy", "new"]));
        assert_eq!(frontmatter.filenames, strings(&["src/lib.rs"]));
        assert_eq!(frontmatter.related_files, strings(&["src/main.rs"]));
        assert_eq!(frontmatter.links, strings(&["new-link"]));
        assert_eq!(body, "# Heading\n\nOriginal body");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn archive_op_preserves_content_and_makes_doc_inactive() {
        let dir = tempfile::tempdir().unwrap();
        let gcx = test_gcx_with_workspace(dir.path()).await;
        let knowledge_dir = dir.path().join(KNOWLEDGE_FOLDER_NAME);
        tokio::fs::create_dir_all(&knowledge_dir).await.unwrap();
        let path = knowledge_dir.join("memory.md");
        write_memory_file(
            &path,
            active_frontmatter("memory", &["old"]),
            "Archive body",
        )
        .await;

        let mut op = MemoryLifecycleOp::pending(
            "op-archive",
            MemorySource::MemoryGarden,
            MemoryOpType::Archive,
            vec![path.to_string_lossy().to_string()],
            "archive",
            0.91,
            "2026-05-02T00:00:00Z",
        );
        op.status = MemoryOpStatus::Approved;

        apply_memory_lifecycle_op(gcx.clone(), &op).await.unwrap();

        assert!(path.exists());
        let text = tokio::fs::read_to_string(&path).await.unwrap();
        let (frontmatter, body) = frontmatter_and_body(&text);
        assert_eq!(frontmatter.status.as_deref(), Some("archived"));
        assert_eq!(body, "Archive body");
        let kg = crate::knowledge_graph::build_knowledge_graph(gcx.clone()).await;
        assert!(kg.active_docs().all(|doc| doc.path != path));
        let found = crate::memories::load_memories_by_tags(gcx, &["old"], 10)
            .await
            .unwrap();
        assert!(found.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn invalid_path_traversal_and_symlink_escape_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let gcx = test_gcx_with_workspace(dir.path()).await;
        let knowledge_dir = dir.path().join(KNOWLEDGE_FOLDER_NAME);
        tokio::fs::create_dir_all(&knowledge_dir).await.unwrap();
        let outside = dir.path().join("outside.md");
        write_memory_file(&outside, active_frontmatter("outside", &["old"]), "Outside").await;
        let link = knowledge_dir.join("link.md");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&outside, &link).unwrap();

        let mut traversal = MemoryLifecycleOp::pending(
            "op-traversal",
            MemorySource::MemoryGarden,
            MemoryOpType::Retag,
            strings(&["../outside.md"]),
            "retag",
            0.91,
            "2026-05-02T00:00:00Z",
        );
        traversal.payload.tags = Some(strings(&["new"]));
        let traversal_err = apply_memory_lifecycle_op(gcx.clone(), &traversal)
            .await
            .unwrap_err();
        assert!(traversal_err.contains(".."));

        let mut symlink = MemoryLifecycleOp::pending(
            "op-symlink",
            MemorySource::MemoryGarden,
            MemoryOpType::Retag,
            vec![link.to_string_lossy().to_string()],
            "retag",
            0.91,
            "2026-05-02T00:00:00Z",
        );
        symlink.payload.tags = Some(strings(&["new"]));
        let symlink_err = apply_memory_lifecycle_op(gcx, &symlink).await.unwrap_err();
        assert!(symlink_err.contains("symlink"));

        let text = tokio::fs::read_to_string(&outside).await.unwrap();
        let (frontmatter, body) = frontmatter_and_body(&text);
        assert_eq!(frontmatter.tags, strings(&["old"]));
        assert_eq!(body, "Outside");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn merge_archive_requires_approval_and_archives_after_canonical_write() {
        let dir = tempfile::tempdir().unwrap();
        let gcx = test_gcx_with_workspace(dir.path()).await;
        let knowledge_dir = dir.path().join(KNOWLEDGE_FOLDER_NAME);
        tokio::fs::create_dir_all(&knowledge_dir).await.unwrap();
        let old_path = knowledge_dir.join("old.md");
        write_memory_file(&old_path, active_frontmatter("old", &["old"]), "Old body").await;

        let mut op = MemoryLifecycleOp::pending(
            "op-merge",
            MemorySource::MemoryGarden,
            MemoryOpType::MergeArchive,
            vec![old_path.to_string_lossy().to_string()],
            "merge",
            0.91,
            "2026-05-02T00:00:00Z",
        );
        op.payload.canonical = Some(MemoryCreatePayload {
            title: Some("Canonical".to_string()),
            content: "Canonical body".to_string(),
            tags: strings(&["canonical"]),
            kind: "domain".to_string(),
            ..Default::default()
        });

        let err = apply_memory_lifecycle_op(gcx.clone(), &op)
            .await
            .unwrap_err();
        assert!(err.contains("approval"));
        let old_text = tokio::fs::read_to_string(&old_path).await.unwrap();
        assert_eq!(
            frontmatter_and_body(&old_text).0.status.as_deref(),
            Some("active")
        );

        op.status = MemoryOpStatus::Approved;
        let outcome = apply_memory_lifecycle_op(gcx, &op).await.unwrap();
        assert_eq!(outcome.status, MemoryOpStatus::Applied);
        assert_eq!(outcome.applied_paths.len(), 2);
        let old_text = tokio::fs::read_to_string(&old_path).await.unwrap();
        let (old_frontmatter, old_body) = frontmatter_and_body(&old_text);
        assert_eq!(old_frontmatter.status.as_deref(), Some("archived"));
        assert!(old_frontmatter.superseded_by.is_some());
        assert_eq!(old_body, "Old body");
        let canonical_text = tokio::fs::read_to_string(&outcome.applied_paths[0])
            .await
            .unwrap();
        let (canonical_frontmatter, canonical_body) = frontmatter_and_body(&canonical_text);
        assert_eq!(canonical_frontmatter.title.as_deref(), Some("Canonical"));
        assert_eq!(canonical_body, "Canonical body");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn failed_apply_leaves_original_file_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let gcx = test_gcx_with_workspace(dir.path()).await;
        let knowledge_dir = dir.path().join(KNOWLEDGE_FOLDER_NAME);
        tokio::fs::create_dir_all(&knowledge_dir).await.unwrap();
        let path = knowledge_dir.join("memory.md");
        write_memory_file(&path, active_frontmatter("memory", &["old"]), "Body").await;
        let before = tokio::fs::read_to_string(&path).await.unwrap();

        let op = MemoryLifecycleOp::pending(
            "op-fail",
            MemorySource::MemoryGarden,
            MemoryOpType::Retag,
            vec![path.to_string_lossy().to_string()],
            "retag",
            0.91,
            "2026-05-02T00:00:00Z",
        );

        let err = apply_memory_lifecycle_op(gcx, &op).await.unwrap_err();
        assert!(err.contains("missing tags"));
        let after = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(after, before);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn replay_of_applied_op_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let gcx = test_gcx_with_workspace(dir.path()).await;
        let mut op = MemoryLifecycleOp::pending(
            "op-applied",
            MemorySource::BehaviorLearner,
            MemoryOpType::CreateMemory,
            Vec::new(),
            "evidence",
            0.91,
            "2026-05-02T00:00:00Z",
        );
        op.status = MemoryOpStatus::Applied;
        op.payload.content = Some("Should not be written".to_string());

        let outcome = apply_memory_lifecycle_op(gcx, &op).await.unwrap();

        assert_eq!(outcome.status, MemoryOpStatus::Skipped);
        assert!(dir.path().join(KNOWLEDGE_FOLDER_NAME).read_dir().is_err());
    }
}
