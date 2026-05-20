use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};

use async_trait::async_trait;
use axum::extract::FromRef;
use tokenizers::Tokenizer;
use tokio::sync::{Mutex as AMutex, RwLock as ARwLock, Semaphore};

use crate::ast::ast_indexer_thread::AstIndexService;
use crate::buddy::actor::BuddyService;
use crate::buddy::events::BuddyEvent;
use crate::buddy::user_activity::UserActivityRing;
use crate::caps::CodeAssistantCaps;
use crate::chat::trajectories::{self, TrajectoryEvent};
use crate::chat::{self, process_command_queue, SessionsMap};
use crate::completion_cache::CompletionCache;
use crate::files_blocklist::IndexingEverywhere;
use crate::files_in_workspace::DocumentsState;
use crate::global_context::{AtCommandsPreviewCache, CommandLine, SharedGlobalContext};
use crate::http::routers::v1::code_lens::CodeLensCache;
use crate::http::routers::v1::sidebar::NotificationEvent;
use crate::integrations::browser_runtime::BrowserRuntime;
use crate::integrations::sessions::IntegrationSession;
use crate::knowledge_index::KnowledgeIndex;
use crate::privacy::PrivacySettings;
use crate::providers::ProviderRegistry;
use crate::stats::event::LlmCallEvent;
use crate::tasks::events::TaskEventEnvelope;
use crate::voice::SharedVoiceService;
use crate::yaml_configs::customization_registry::RegistryCacheManager;
use refact_core::vecdb_types::VecdbSearch;
use refact_runtime_api::{
    ChatSessionFacade, ChatSessionSnapshot, ChatSessionUpdate, CreateSessionRequest,
};

#[derive(Clone)]
pub struct RuntimeServices {
    pub shutdown_flag: Arc<AtomicBool>,
    pub cmdline: Arc<CommandLine>,
    pub http_client: reqwest::Client,
    pub http_client_slowdown: Arc<Semaphore>,
    pub ask_shutdown_sender: Arc<StdMutex<std::sync::mpsc::Sender<String>>>,
}

#[derive(Clone)]
pub struct PathServices {
    pub cache_dir: PathBuf,
    pub config_dir: PathBuf,
    pub app_searchable_id: String,
}

#[derive(Clone)]
pub struct ModelServices {
    pub caps: Arc<ARwLock<CapsState>>,
    pub tokenizers: Arc<StdRwLock<TokenizerState>>,
    pub providers: Arc<ARwLock<ProviderRegistry>>,
    pub llm_stats_sender: Option<tokio::sync::mpsc::Sender<LlmCallEvent>>,
}

#[derive(Clone)]
pub struct CapsState {
    pub caps: Option<Arc<CodeAssistantCaps>>,
    pub reading_lock: Arc<AMutex<bool>>,
    pub last_error: String,
    pub last_attempted_ts: u64,
    pub models_dev_startup_refresh_attempted: bool,
}

#[derive(Clone)]
pub struct TokenizerState {
    pub map: HashMap<String, Option<Arc<Tokenizer>>>,
    pub download_lock: Arc<AMutex<bool>>,
}

#[derive(Clone)]
pub struct WorkspaceServices {
    pub documents_state: DocumentsState,
    pub privacy_settings: Arc<PrivacySettings>,
    pub indexing_everywhere: Arc<IndexingEverywhere>,
    pub completions_cache: Arc<StdRwLock<CompletionCache>>,
    pub vec_db: Arc<AMutex<Option<Arc<dyn VecdbSearch>>>>,
    pub vec_db_error: Arc<StdMutex<String>>,
    pub ast_service: Option<Arc<AMutex<AstIndexService>>>,
    pub knowledge_index: Arc<AMutex<KnowledgeIndex>>,
    pub at_commands_preview_cache: Arc<AMutex<AtCommandsPreviewCache>>,
}

#[derive(Clone)]
pub struct ChatServices {
    pub sessions: SessionsMap,
    pub facade: Arc<dyn ChatSessionFacade>,
    pub trajectory_events_tx: tokio::sync::broadcast::Sender<TrajectoryEvent>,
    pub workspace_changed_tx: tokio::sync::broadcast::Sender<()>,
    pub task_events_tx: tokio::sync::broadcast::Sender<TaskEventEnvelope>,
    pub task_events_seq: Arc<AtomicU64>,
    pub notification_events_tx: tokio::sync::broadcast::Sender<NotificationEvent>,
    pub voice_service: SharedVoiceService,
}

#[derive(Clone)]
pub struct BuddyServices {
    pub buddy: Arc<AMutex<Option<BuddyService>>>,
    pub buddy_events_tx: tokio::sync::broadcast::Sender<BuddyEvent>,
    pub user_activity: Arc<AMutex<UserActivityRing>>,
}

#[derive(Clone)]
pub struct IntegrationServices {
    pub integration_sessions: Arc<AMutex<HashMap<String, Arc<AMutex<Box<dyn IntegrationSession>>>>>>,
    pub browser_runtimes: Arc<AMutex<HashMap<String, Arc<AMutex<BrowserRuntime>>>>>,
    pub ext_cache_generation: Arc<AtomicU64>,
    pub project_registry_cache: Arc<StdRwLock<RegistryCacheManager>>,
    pub codelens_cache: Arc<AMutex<CodeLensCache>>,
    pub init_shadow_repos_lock: Arc<AMutex<bool>>,
    pub git_operations_abort_flag: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct EngineChatSessionFacade {
    gcx: SharedGlobalContext,
}

impl EngineChatSessionFacade {
    pub fn new(gcx: SharedGlobalContext) -> Self {
        Self { gcx }
    }
}

#[async_trait]
impl ChatSessionFacade for EngineChatSessionFacade {
    async fn session_snapshot(&self, chat_id: &str) -> Result<ChatSessionSnapshot, String> {
        let app = AppState::from_gcx(self.gcx.clone()).await;
        let session_arc =
            chat::get_or_create_session_with_trajectory(app, &self.gcx.chat_sessions, chat_id).await;
        let session = session_arc.lock().await;
        Ok(ChatSessionSnapshot {
            messages: session.messages.clone(),
            thread: session.thread.clone(),
            session_state: session.runtime.state,
        })
    }

    async fn update_session(&self, chat_id: &str, update: ChatSessionUpdate) -> Result<(), String> {
        let app = AppState::from_gcx(self.gcx.clone()).await;
        let session_arc =
            chat::get_or_create_session_with_trajectory(app, &self.gcx.chat_sessions, chat_id).await;
        let mut session = session_arc.lock().await;
        session.messages = update.messages;
        session.increment_version();
        let snapshot = session.snapshot();
        session.emit(snapshot);
        Ok(())
    }

    async fn create_session(&self, request: CreateSessionRequest) -> Result<(), String> {
        let app = AppState::from_gcx(self.gcx.clone()).await;
        let session_arc = chat::get_or_create_session_with_trajectory(
            app,
            &self.gcx.chat_sessions,
            &request.chat_id,
        )
        .await;
        let mut session = session_arc.lock().await;
        session.thread = request.thread;
        for message in request.messages {
            session.add_message(message);
        }
        session.increment_version();
        Ok(())
    }

    async fn push_command(
        &self,
        chat_id: &str,
        command: refact_chat_api::ChatCommand,
    ) -> Result<(), String> {
        let app = AppState::from_gcx(self.gcx.clone()).await;
        let session_arc =
            chat::get_or_create_session_with_trajectory(app.clone(), &self.gcx.chat_sessions, chat_id)
                .await;
        let mut session = session_arc.lock().await;
        session.command_queue.push_back(refact_chat_api::CommandRequest {
            client_request_id: uuid::Uuid::new_v4().to_string(),
            priority: false,
            command,
        });
        session.touch();
        let processor_running = session.queue_processor_running.clone();
        let queue_notify = session.queue_notify.clone();
        drop(session);
        if !processor_running.swap(true, Ordering::SeqCst) {
            tokio::spawn(process_command_queue(app, session_arc, processor_running));
        } else {
            queue_notify.notify_one();
        }
        Ok(())
    }

    async fn session_state(
        &self,
        chat_id: &str,
    ) -> Result<Option<refact_runtime_api::SessionState>, String> {
        let session_arc = {
            let sessions = self.gcx.chat_sessions.read().await;
            sessions.get(chat_id).cloned()
        };
        match session_arc {
            Some(session_arc) => Ok(Some(session_arc.lock().await.runtime.state)),
            None => Ok(None),
        }
    }

    async fn maybe_save_session(&self, chat_id: &str) -> Result<(), String> {
        let session_arc = {
            let sessions = self.gcx.chat_sessions.read().await;
            sessions.get(chat_id).cloned()
        };
        if let Some(session_arc) = session_arc {
            trajectories::maybe_save_trajectory(AppState::from_gcx(self.gcx.clone()).await, session_arc).await;
        }
        Ok(())
    }

    async fn save_trajectory_snapshot(
        &self,
        snapshot: refact_runtime_api::RuntimeTrajectorySnapshot,
    ) -> Result<(), String> {
        trajectories::save_trajectory_snapshot(self.gcx.clone(), snapshot).await
    }
}

#[derive(Clone)]
pub struct AppState {
    pub gcx: SharedGlobalContext,
    pub runtime: RuntimeServices,
    pub paths: PathServices,
    pub model: ModelServices,
    pub workspace: WorkspaceServices,
    pub chat: ChatServices,
    pub buddy: BuddyServices,
    pub integrations: IntegrationServices,
}

impl AppState {
    pub async fn from_gcx(gcx: SharedGlobalContext) -> Self {
        gcx.app_state(gcx.clone())
    }
}

impl FromRef<AppState> for SharedGlobalContext {
    fn from_ref(app: &AppState) -> Self {
        app.gcx.clone()
    }
}

impl From<AppState> for SharedGlobalContext {
    fn from(app: AppState) -> Self {
        app.gcx.clone()
    }
}

impl From<&AppState> for SharedGlobalContext {
    fn from(app: &AppState) -> Self {
        app.gcx.clone()
    }
}

impl FromRef<AppState> for RuntimeServices {
    fn from_ref(app: &AppState) -> Self {
        app.runtime.clone()
    }
}

impl FromRef<AppState> for PathServices {
    fn from_ref(app: &AppState) -> Self {
        app.paths.clone()
    }
}

impl FromRef<AppState> for ModelServices {
    fn from_ref(app: &AppState) -> Self {
        app.model.clone()
    }
}

impl FromRef<AppState> for WorkspaceServices {
    fn from_ref(app: &AppState) -> Self {
        app.workspace.clone()
    }
}

impl FromRef<AppState> for ChatServices {
    fn from_ref(app: &AppState) -> Self {
        app.chat.clone()
    }
}

impl FromRef<AppState> for BuddyServices {
    fn from_ref(app: &AppState) -> Self {
        app.buddy.clone()
    }
}

impl FromRef<AppState> for IntegrationServices {
    fn from_ref(app: &AppState) -> Self {
        app.integrations.clone()
    }
}
