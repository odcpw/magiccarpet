//! Campaign registry, lifecycle, and campaign/native-session ABI surfaces.
//!
//! This family is a pure move from the native_v2 facade. Crate visibility on
//! internal helpers preserves the existing parent and sibling call graph.

use super::*;

#[derive(Default)]
pub(super) struct Registry {
    pub(super) next_handle: u64,
    pub(super) sessions: BTreeMap<u64, RegistryEntry>,
}

#[derive(Clone, Copy)]
enum HandleNamespace {
    SharedSession,
    RetainedCampaign,
}

impl HandleNamespace {
    fn next(self, previous: u64) -> u64 {
        match self {
            // All handles owned by the shared legacy/neutral/runtime registry
            // are odd. Preserve 1 as the first handle and cycle only through
            // the odd half of the nonzero u64 space.
            Self::SharedSession => {
                if previous == 0 || previous == u64::MAX {
                    1
                } else {
                    (previous | 1).wrapping_add(2)
                }
            }
            // Retained campaign handles are even. This makes the two live
            // registries globally disjoint without changing the public u64
            // ABI or coupling their independent lifecycle locks.
            Self::RetainedCampaign => {
                let next = (previous & !1).wrapping_add(2);
                if next == 0 { 2 } else { next }
            }
        }
    }
}

#[derive(Clone)]
pub(super) enum RegistryEntry {
    Legacy(Arc<Mutex<SessionRecord>>),
    Neutral(Arc<Mutex<super::game_session_abi::NeutralSession>>),
}

impl Registry {
    pub(super) fn insert(&mut self, session: SessionRecord) -> AbiResult<u64> {
        let handle = self.reserve_handle()?;
        self.sessions
            .insert(handle, RegistryEntry::Legacy(Arc::new(Mutex::new(session))));
        Ok(handle)
    }

    pub(super) fn reserve_handle(&mut self) -> AbiResult<u64> {
        for _ in 0..u16::MAX {
            self.next_handle = HandleNamespace::SharedSession.next(self.next_handle);
            if !self.sessions.contains_key(&self.next_handle) {
                return Ok(self.next_handle);
            }
        }
        Err(AbiError::new(
            MC_STATUS_LOCK_FAILED,
            "session handle space is unavailable",
        ))
    }

    pub(super) fn insert_neutral(
        &mut self,
        handle: u64,
        session: super::game_session_abi::NeutralSession,
    ) -> AbiResult<()> {
        if handle == 0 || handle & 1 == 0 || self.sessions.contains_key(&handle) {
            return Err(AbiError::new(
                MC_STATUS_LOCK_FAILED,
                format!("invalid or colliding neutral session handle {handle}"),
            ));
        }
        self.sessions.insert(
            handle,
            RegistryEntry::Neutral(Arc::new(Mutex::new(session))),
        );
        Ok(())
    }

    pub(super) fn remove_legacy(&mut self, handle: u64) -> AbiResult<()> {
        match self.sessions.get(&handle) {
            Some(RegistryEntry::Legacy(_)) => {
                self.sessions.remove(&handle);
                Ok(())
            }
            _ => Err(AbiError::new(
                MC_STATUS_UNKNOWN_HANDLE,
                format!("unknown legacy session handle {handle}"),
            )),
        }
    }

    pub(super) fn remove_neutral(&mut self, handle: u64) -> AbiResult<()> {
        match self.sessions.get(&handle) {
            Some(RegistryEntry::Neutral(_)) => {
                self.sessions.remove(&handle);
                Ok(())
            }
            _ => Err(AbiError::new(
                MC_STATUS_UNKNOWN_HANDLE,
                format!("unknown neutral session handle {handle}"),
            )),
        }
    }
}

pub(super) static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

pub(super) fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

pub(super) fn boundary(operation: impl FnOnce() -> AbiResult<()>) -> i32 {
    LAST_ERROR.with(|error| error.borrow_mut().clear());
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => MC_STATUS_OK,
        Ok(Err(error)) => {
            set_last_error(&error.message);
            error.status
        }
        Err(_) => {
            set_last_error("panic contained at mc_native ABI boundary");
            MC_STATUS_INTERNAL_PANIC
        }
    }
}

pub(super) fn boundary_value<T: Copy>(
    output: *mut T,
    output_size: u32,
    operation: impl FnOnce() -> AbiResult<T>,
) -> i32 {
    boundary(|| {
        require_struct_output::<T>(output, output_size, "output")?;
        let value = operation()?;
        // SAFETY: the caller supplied a non-null writable output pointer per
        // the ABI contract; unaligned writes support conservative P/Invoke.
        unsafe { output.write_unaligned(value) };
        Ok(())
    })
}

pub(super) fn set_last_error(message: &str) {
    LAST_ERROR.with(|error| {
        let mut error = error.borrow_mut();
        error.clear();
        error.push_str(message);
    });
}

pub(super) fn with_registry<T>(operation: impl FnOnce(&Registry) -> AbiResult<T>) -> AbiResult<T> {
    let guard = registry()
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session registry lock is poisoned"))?;
    operation(&guard)
}

pub(super) fn with_registry_mut<T>(
    operation: impl FnOnce(&mut Registry) -> AbiResult<T>,
) -> AbiResult<T> {
    let mut guard = registry()
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session registry lock is poisoned"))?;
    operation(&mut guard)
}

fn session_entry(registry: &Registry, handle: u64) -> AbiResult<RegistryEntry> {
    if handle == 0 {
        return Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            "session handle zero is invalid",
        ));
    }
    registry.sessions.get(&handle).cloned().ok_or_else(|| {
        AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            format!("unknown session handle {handle}"),
        )
    })
}

pub(super) fn with_session<T>(
    handle: u64,
    operation: impl FnOnce(&SessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    match with_registry(|registry| session_entry(registry, handle))? {
        RegistryEntry::Legacy(entry) => {
            let guard = entry
                .lock()
                .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session lock is poisoned"))?;
            operation(&guard)
        }
        RegistryEntry::Neutral(entry) => {
            let guard = entry.lock().map_err(|_| {
                AbiError::new(MC_STATUS_LOCK_FAILED, "neutral session lock is poisoned")
            })?;
            operation(guard.legacy_projection()?)
        }
    }
}

pub(super) fn with_session_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut SessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    match with_registry(|registry| session_entry(registry, handle))? {
        RegistryEntry::Legacy(entry) => {
            let mut guard = entry
                .lock()
                .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session lock is poisoned"))?;
            operation(&mut guard)
        }
        RegistryEntry::Neutral(_) => Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "legacy gameplay mutation is unavailable on a neutral session",
        )),
    }
}

/// Mutate only an explicitly classified presentation/audio sidecar on the
/// same SessionRecord owned by the neutral GameSession. Callers must never use
/// this seam for gameplay stepping, reset, injection, or selector authority.
pub(super) fn with_projection_session_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut SessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    match with_registry(|registry| session_entry(registry, handle))? {
        RegistryEntry::Legacy(entry) => {
            let mut guard = entry
                .lock()
                .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session lock is poisoned"))?;
            operation(&mut guard)
        }
        RegistryEntry::Neutral(entry) => {
            let mut guard = entry.lock().map_err(|_| {
                AbiError::new(MC_STATUS_LOCK_FAILED, "neutral session lock is poisoned")
            })?;
            operation(guard.legacy_projection_mut()?)
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum CampaignLevel2Authority {
    Legacy(Level2ResetSession),
    Descriptor(DescriptorCampaignSession),
}

impl CampaignLevel2Authority {
    fn descriptor(&self) -> AbiResult<&DescriptorCampaignSession> {
        match self {
            Self::Descriptor(session) => Ok(session),
            Self::Legacy(_) => Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "generic campaign observation requires a descriptor-backed session",
            )),
        }
    }

    fn reset_authority(&mut self) {
        match self {
            Self::Legacy(session) => session.reset(),
            Self::Descriptor(session) => session.reset(),
        }
    }

    fn exposed_player_count(&self, inspection: &NativeWorldInspection) -> usize {
        match self {
            Self::Legacy(_) => inspection.players().len(),
            Self::Descriptor(session) => {
                usize::from(session.level2_compatibility().runtime().player_count)
            }
        }
    }

    fn active_participant_count(&self) -> usize {
        usize::from(self.runtime().player_count)
    }

    fn exposed_objective_count(&self, inspection: &NativeWorldInspection) -> usize {
        match self {
            Self::Legacy(_) => inspection.runtime().objectives.len(),
            Self::Descriptor(session) => session.definition().level_definition().objectives().len(),
        }
    }

    fn step_authority(&mut self, command: PlayerCommand, speed_index: u8) -> Vec<StepEvidence> {
        match self {
            Self::Legacy(session) => session.step_outer_turn(command, speed_index),
            Self::Descriptor(session) => session.step(command, speed_index),
        }
    }
}

impl std::ops::Deref for CampaignLevel2Authority {
    type Target = Level2ResetSession;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Legacy(session) => session,
            Self::Descriptor(session) => session.level2_compatibility(),
        }
    }
}

impl std::ops::DerefMut for CampaignLevel2Authority {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Legacy(session) => session,
            Self::Descriptor(session) => session.level2_compatibility_mut(),
        }
    }
}

impl CampaignLevel2Authority {
    pub(super) fn definition_identity(&self) -> CampaignSelection {
        match self {
            Self::Legacy(_) => CampaignSelection::LEVEL2,
            Self::Descriptor(session) => session.definition().identity,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum CampaignSessionKind {
    Level1 {
        world: Arc<NativeWorld>,
    },
    Level2 {
        inspection: Arc<NativeWorldInspection>,
        reset: CampaignLevel2Authority,
        day_frame_assets: Option<Arc<DayIndexedFrameAssets>>,
        day_frame_state: DayIndexedFrameState,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CampaignDayMusicLifecycle {
    pub(super) envelope: DynamicMusicEnvelope,
    pub(super) initialized: bool,
    pub(super) loaded: bool,
    pub(super) active: bool,
    pub(super) playing: bool,
    pub(super) track: u8,
    pub(super) volume: u8,
    pub(super) asset_slot: u32,
    pub(super) start_count: u32,
    pub(super) update_count: u32,
    pub(super) fade_callback_count: u32,
    pub(super) stop_count: u32,
    pub(super) end_count: u32,
}

impl CampaignDayMusicLifecycle {
    pub(super) fn start_day() -> Self {
        let mut envelope = DynamicMusicEnvelope::default();
        envelope.start();
        Self {
            envelope,
            initialized: true,
            loaded: true,
            active: true,
            playing: true,
            track: 2,
            volume: 127,
            asset_slot: 0,
            start_count: 1,
            update_count: 0,
            fade_callback_count: 0,
            stop_count: 0,
            end_count: 0,
        }
    }

    pub(super) fn update(&mut self, mode: u8, timer_multiplier: u8) {
        self.update_count = self.update_count.wrapping_add(1);
        let _ = self.envelope.update(mode, timer_multiplier);
    }

    pub(super) fn advance(&mut self, callbacks: u32) {
        for _ in 0..callbacks {
            if !self.envelope.timer_active {
                break;
            }
            self.fade_callback_count = self.fade_callback_count.wrapping_add(1);
            debug_assert!(!matches!(self.envelope.callback(), FadeCallback::Inactive));
        }
    }

    pub(super) fn stop(&mut self) {
        if !self.playing || !self.active {
            return;
        }
        self.stop_count = self.stop_count.wrapping_add(1);
        self.playing = false;
        self.envelope.stop();
    }

    pub(super) fn end(&mut self) {
        if !self.loaded {
            return;
        }
        self.end_count = self.end_count.wrapping_add(1);
        self.playing = false;
        self.loaded = false;
        self.active = false;
        self.envelope.stop();
    }

    pub(super) fn state_hash(self) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(b"level2-day-music-lifecycle-v1");
        hasher.update(self.asset_slot.to_le_bytes());
        hasher.update(self.start_count.to_le_bytes());
        hasher.update(self.update_count.to_le_bytes());
        hasher.update(self.fade_callback_count.to_le_bytes());
        hasher.update(self.stop_count.to_le_bytes());
        hasher.update(self.end_count.to_le_bytes());
        hasher.update([
            u8::from(self.initialized),
            u8::from(self.loaded),
            u8::from(self.active),
            u8::from(self.playing),
            self.track,
            self.volume,
            u8::from(self.envelope.updates_enabled),
            u8::from(self.envelope.timer_active),
            self.envelope.target_mode,
            self.envelope.direction.cast_unsigned(),
            self.envelope.current,
            self.envelope.timer_multiplier,
        ]);
        let digest = hasher.finalize();
        u64::from_le_bytes(digest[..8].try_into().expect("sha256 has eight bytes"))
    }
}

#[derive(Clone, Debug)]
pub(super) struct CampaignSessionRecord {
    pub(super) kind: CampaignSessionKind,
    pub(super) generation: u64,
    pub(super) day_music: Option<CampaignDayMusicLifecycle>,
    /// Transport metadata needed by the frozen `McEventV2.state_hash` field.
    /// Keys are zero-based canonical EffectJournal sequences.
    pub(super) effect_state_hashes: BTreeMap<u64, u64>,
}

#[derive(Default)]
pub(super) struct CampaignRegistry {
    pub(super) next_handle: u64,
    pub(super) sessions: BTreeMap<u64, Arc<Mutex<CampaignSessionRecord>>>,
}

impl CampaignRegistry {
    pub(super) fn insert(&mut self, session: CampaignSessionRecord) -> AbiResult<u64> {
        for _ in 0..u16::MAX {
            self.next_handle = HandleNamespace::RetainedCampaign.next(self.next_handle);
            if !self.sessions.contains_key(&self.next_handle) {
                let handle = self.next_handle;
                self.sessions.insert(handle, Arc::new(Mutex::new(session)));
                return Ok(handle);
            }
        }
        Err(AbiError::new(
            MC_STATUS_LOCK_FAILED,
            "campaign session handle space is unavailable",
        ))
    }
}

pub(super) static CAMPAIGN_REGISTRY: OnceLock<Mutex<CampaignRegistry>> = OnceLock::new();

pub(super) fn campaign_registry() -> &'static Mutex<CampaignRegistry> {
    CAMPAIGN_REGISTRY.get_or_init(|| Mutex::new(CampaignRegistry::default()))
}

pub(super) fn with_campaign_registry<T>(
    operation: impl FnOnce(&CampaignRegistry) -> AbiResult<T>,
) -> AbiResult<T> {
    let guard = campaign_registry().lock().map_err(|_| {
        AbiError::new(
            MC_STATUS_LOCK_FAILED,
            "campaign session registry lock is poisoned",
        )
    })?;
    operation(&guard)
}

pub(super) fn with_campaign_registry_mut<T>(
    operation: impl FnOnce(&mut CampaignRegistry) -> AbiResult<T>,
) -> AbiResult<T> {
    let mut guard = campaign_registry().lock().map_err(|_| {
        AbiError::new(
            MC_STATUS_LOCK_FAILED,
            "campaign session registry lock is poisoned",
        )
    })?;
    operation(&mut guard)
}

pub(super) fn campaign_session_arc(
    registry: &CampaignRegistry,
    handle: u64,
) -> AbiResult<Arc<Mutex<CampaignSessionRecord>>> {
    if handle == 0 {
        return Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            "campaign session handle zero is invalid",
        ));
    }
    registry.sessions.get(&handle).cloned().ok_or_else(|| {
        AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            format!("unknown campaign session handle {handle}"),
        )
    })
}

pub(super) fn with_campaign_session<T>(
    handle: u64,
    operation: impl FnOnce(&CampaignSessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = with_campaign_registry(|registry| Ok(registry.sessions.get(&handle).cloned()))?;
    if let Some(entry) = entry {
        let guard = entry.lock().map_err(|_| {
            AbiError::new(MC_STATUS_LOCK_FAILED, "campaign session lock is poisoned")
        })?;
        return operation(&guard);
    }

    // The neutral game-runtime handle owns the same descriptor campaign that
    // the retained Level 2 presentation ABI knows how to project.  Materialize
    // only a read adapter around a clone while holding the runtime lock: no
    // registry entry, step authority, reset path, or independently advancing
    // state is created.  Mutating campaign exports deliberately continue to
    // reject this handle through `with_campaign_session_mut`.
    let entry = super::game_session_abi::neutral_session_arc(handle)?;
    let mut guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "neutral session lock is poisoned"))?;
    if let super::game_session_abi::NeutralSession::Runtime(runtime) = &mut *guard {
        let record = runtime.take_campaign_presentation_record()?;
        let result = operation(&record);
        runtime.retain_campaign_presentation_record(record);
        return result;
    }
    let (backend, generation) = guard.campaign_backend_and_generation()?;
    let campaign = backend.campaign_presentation_source().clone();
    let record = CampaignSessionRecord {
        kind: CampaignSessionKind::Level2 {
            inspection: Arc::new(campaign.inspection().clone()),
            reset: CampaignLevel2Authority::Descriptor(campaign),
            day_frame_assets: None,
            day_frame_state: DayIndexedFrameState::new(generation),
        },
        generation,
        day_music: None,
        effect_state_hashes: BTreeMap::new(),
    };
    operation(&record)
}

fn with_campaign_presentation_session_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut CampaignSessionRecord, bool) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = with_campaign_registry(|registry| Ok(registry.sessions.get(&handle).cloned()))?;
    if let Some(entry) = entry {
        let mut guard = entry.lock().map_err(|_| {
            AbiError::new(MC_STATUS_LOCK_FAILED, "campaign session lock is poisoned")
        })?;
        return operation(&mut guard, false);
    }

    let entry = super::game_session_abi::neutral_session_arc(handle)?;
    let mut guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "game runtime lock is poisoned"))?;
    let super::game_session_abi::NeutralSession::Runtime(runtime) = &mut *guard else {
        return Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            format!("handle {handle} is not a campaign or game runtime"),
        ));
    };
    let mut record = runtime.take_campaign_presentation_record()?;
    let result = operation(&mut record, true);
    runtime.retain_campaign_presentation_record(record);
    result
}

pub(super) fn with_campaign_session_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut CampaignSessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = with_campaign_registry(|registry| campaign_session_arc(registry, handle))?;
    let mut guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "campaign session lock is poisoned"))?;
    operation(&mut guard)
}

#[unsafe(no_mangle)]
/// Mechanically infallible constant expression: this export touches no memory,
/// performs no allocation and invokes no panic-capable operation.
pub extern "C" fn mc_native_abi_version() -> u32 {
    MC_NATIVE_ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_open(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    out_handle: *mut u64,
) -> i32 {
    boundary_value(out_handle, struct_size::<u64>(), || {
        let package = utf8_input(package_dir, package_dir_len, "package_dir")?;
        let closure = utf8_input(closure_path, closure_path_len, "closure_path")?;
        if package.is_empty() || closure.is_empty() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "package_dir and closure_path must be non-empty",
            ));
        }
        let owner_cache = std::env::var_os("MC_ASSET_CACHE_PATH").map(PathBuf::from);
        let session = CampaignFacade::<SealedLevel1Adapter>::open(
            SealedLevel1Adapter::KEY,
            InternalCampaignOpen {
                package_dir: Path::new(&package),
                closure_path: Some(Path::new(&closure)),
                owner_cache: owner_cache.as_deref(),
            },
        )?;
        with_registry_mut(|registry| registry.insert(session))
    })
}

/// Open a validated common campaign session. Level 1 uses the same loader as
/// the legacy call; Level 2 uses the typed inspection/content seam and never
/// falls through to the Level 1 mission dispatcher.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_open(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    selection: *const McCampaignSelectionV2,
    out_handle: *mut u64,
) -> i32 {
    boundary(|| {
        open_campaign_session(
            package_dir,
            package_dir_len,
            closure_path,
            closure_path_len,
            None,
            selection,
            out_handle,
        )
    })
}

/// Open a campaign session with an explicit qualified owner-cache root.  The
/// additive entry point keeps the legacy environment-bound open intact while
/// letting managed callers bind the Day cache to this handle without relying
/// on process-global state.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_open_with_asset_cache(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    asset_cache_path: *const u8,
    asset_cache_path_len: u32,
    selection: *const McCampaignSelectionV2,
    out_handle: *mut u64,
) -> i32 {
    boundary(|| {
        require_output(out_handle, "out_handle")?;
        write_output(out_handle, 0);
        let asset_cache_path =
            utf8_input(asset_cache_path, asset_cache_path_len, "asset_cache_path")?;
        open_campaign_session(
            package_dir,
            package_dir_len,
            closure_path,
            closure_path_len,
            (!asset_cache_path.is_empty()).then_some(asset_cache_path),
            selection,
            out_handle,
        )
    })
}

pub(super) fn open_campaign_session(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    asset_cache_path: Option<String>,
    selection: *const McCampaignSelectionV2,
    out_handle: *mut u64,
) -> AbiResult<()> {
    require_output(out_handle, "out_handle")?;
    write_output(out_handle, 0);
    let selection = read_versioned_input(selection, "campaign selection")?;
    validate_reserved(&selection.reserved, "campaign selection")?;
    if selection.reserved_0 != 0 {
        return Err(AbiError::new(
            MC_STATUS_RESERVED_NONZERO,
            "campaign selection reserved_0 must be zero",
        ));
    }
    let package = utf8_input(package_dir, package_dir_len, "package_dir")?;
    if package.is_empty() {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "package_dir must be non-empty",
        ));
    }
    let package_root = Path::new(&package);
    let descriptor_backed = package_root.join("campaign-definition.json").is_file();
    let session = if descriptor_backed {
        let campaign_selection = CampaignSelection {
            external_level: selection.external_level,
            internal_level: selection.internal_level,
            map_type_id: selection.map_type,
        };
        let gameplay_cache = asset_cache_path.as_deref().and_then(|cache_root| {
            let root = Path::new(cache_root);
            root.join("components/gameplay/manifest.json")
                .is_file()
                .then(|| root.join("components/gameplay"))
        });
        let mut campaign = match gameplay_cache {
            Some(cache_root) => DescriptorCampaignSession::open_with_owner_cache(
                package_root,
                cache_root,
                campaign_selection,
            ),
            None => DescriptorCampaignSession::open(package_root, campaign_selection),
        }
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
        // The retained Level 2 C-ABI route historically selects its authored
        // entry objective before exposing generation zero.  That adjustment
        // is not part of the neutral descriptor lifecycle: applying it to a
        // different profiled campaign would overwrite the package's reset
        // objective selector with a Level-2 constant.  Keep the compatibility
        // write at its existing Level 2 boundary and leave every other
        // descriptor-backed reset byte-exact.
        if campaign.definition().identity == CampaignSelection::LEVEL2 {
            campaign
                .level2_compatibility_mut()
                .select_level2_campaign_entry_stage();
        }
        let generation = campaign.generation();
        let campaign_package_sha256 = campaign.definition().package_sha256.clone();
        let inspection = Arc::new(campaign.inspection().clone());
        let day_frame_assets = load_optional_day_frame_assets(
            asset_cache_path.as_deref(),
            inspection.as_ref(),
            Some(&campaign_package_sha256),
        )?
        .map(Arc::new);
        let music_root = asset_cache_path
            .as_deref()
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("MC_ASSET_CACHE_PATH").map(PathBuf::from));
        let day_music = super::campaign_audio::load_campaign_music(
            music_root.as_deref(),
            campaign_selection,
            inspection.package_sha256(),
            &campaign_package_sha256,
        )?;
        CampaignSessionRecord {
            kind: CampaignSessionKind::Level2 {
                inspection,
                reset: CampaignLevel2Authority::Descriptor(campaign),
                day_frame_assets,
                day_frame_state: DayIndexedFrameState::new(generation),
            },
            generation,
            day_music,
            effect_state_hashes: BTreeMap::new(),
        }
    } else {
        let expected = sim_core::mission_content::MissionContentProfile::from_metadata(
            selection.external_level,
            selection.internal_level,
            selection.map_type,
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
        if expected.is_level2() {
            let inspection = match asset_cache_path.as_deref() {
                Some(cache_root) => {
                    let root = Path::new(cache_root);
                    if root.join("components/gameplay/manifest.json").is_file() {
                        NativeWorld::inspect_level2_with_owner_cache(
                            &package,
                            root.join("components/gameplay"),
                        )
                    } else {
                        NativeWorld::inspect_level2(&package)
                    }
                }
                None => NativeWorld::inspect_level2(&package),
            }
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
            let mut reset = inspection
                .level2_reset_session()
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
            reset.select_level2_campaign_entry_stage();
            let day_frame_assets =
                load_optional_day_frame_assets(asset_cache_path.as_deref(), &inspection, None)?
                    .map(Arc::new);
            let day_music = load_optional_level2_day_music(asset_cache_path.as_deref())?;
            CampaignSessionRecord {
                kind: CampaignSessionKind::Level2 {
                    inspection: Arc::new(inspection),
                    reset: CampaignLevel2Authority::Legacy(reset),
                    day_frame_assets,
                    day_frame_state: DayIndexedFrameState::new(0),
                },
                generation: 0,
                day_music,
                effect_state_hashes: BTreeMap::new(),
            }
        } else {
            let closure = utf8_input(closure_path, closure_path_len, "closure_path")?;
            if closure.is_empty() {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "Level 1 campaign sessions require closure_path",
                ));
            }
            let world = NativeWorld::load(&package, &closure)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
            CampaignSessionRecord {
                kind: CampaignSessionKind::Level1 {
                    world: Arc::new(world),
                },
                generation: 0,
                day_music: None,
                effect_state_hashes: BTreeMap::new(),
            }
        }
    };
    let handle = with_campaign_registry_mut(|registry| registry.insert(session))?;
    write_output(out_handle, handle);
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_destroy(handle: u64) -> i32 {
    boundary(|| {
        with_campaign_registry_mut(|registry| {
            if registry.sessions.remove(&handle).is_none() {
                return Err(AbiError::new(
                    MC_STATUS_UNKNOWN_HANDLE,
                    format!("unknown campaign session handle {handle}"),
                ));
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_reset(handle: u64) -> i32 {
    boundary(|| {
        with_campaign_session_mut(handle, |session| {
            if let CampaignSessionKind::Level2 {
                reset,
                day_frame_state,
                ..
            } = &mut session.kind
            {
                reset.reset_authority();
                session.generation = session.generation.wrapping_add(1);
                if let CampaignLevel2Authority::Descriptor(campaign) = reset {
                    debug_assert_eq!(campaign.generation(), session.generation);
                }
                day_frame_state.reset(session.generation);
                session.effect_state_hashes.clear();
                if let Some(music) = session.day_music.as_mut() {
                    let track = music.track;
                    *music = CampaignDayMusicLifecycle::start_day();
                    music.track = track;
                }
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_shield_state(
    handle: u64,
    generation: u64,
    out_state: *mut McCampaignShieldSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Shield state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Shield generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            Ok(campaign_shield_snapshot(
                generation,
                reset.shared_spells().shield_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_shield_activate(
    handle: u64,
    generation: u64,
    direction_mask: u16,
    help_enabled: u8,
    out_state: *mut McCampaignShieldSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        if help_enabled > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign Shield help_enabled must be 0 or 1",
            ));
        }
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Shield activation requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Shield generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if !reset
                .shared_spells_mut()
                .activate_shield(direction_mask, help_enabled != 0)
            {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Shield holder is not active",
                ));
            }
            Ok(campaign_shield_snapshot(
                generation,
                reset.shared_spells().shield_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_shield_damage_submit(
    handle: u64,
    generation: u64,
    target_entity_index: u16,
    damage: i32,
    attacker_entity_index: u16,
) -> i32 {
    boundary(|| {
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Shield damage requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Shield generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if !reset.shared_spells_mut().submit_shield_damage(
                target_entity_index,
                damage,
                attacker_entity_index,
            ) {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Shield damage mailbox rejected the hit",
                ));
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_invisible_state(
    handle: u64,
    generation: u64,
    out_state: *mut McCampaignInvisibleSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Invisible state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Invisible generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            Ok(campaign_invisible_snapshot(
                generation,
                reset.shared_spells().invisible_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_invisible_activate(
    handle: u64,
    generation: u64,
    direction_mask: u16,
    help_enabled: u8,
    out_state: *mut McCampaignInvisibleSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        if help_enabled > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign Invisible help_enabled must be 0 or 1",
            ));
        }
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Invisible activation requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Invisible generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if !reset
                .shared_spells_mut()
                .activate_invisible(direction_mask, help_enabled != 0)
            {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Invisible holder is not active",
                ));
            }
            Ok(campaign_invisible_snapshot(
                generation,
                reset.shared_spells().invisible_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_beyond_sight_state(
    handle: u64,
    generation: u64,
    out_state: *mut McCampaignBeyondSightSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Beyond Sight state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Beyond Sight generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            Ok(campaign_beyond_sight_snapshot(
                generation,
                reset.shared_spells().beyond_sight_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_beyond_sight_activate(
    handle: u64,
    generation: u64,
    direction_mask: u16,
    help_enabled: u8,
    out_state: *mut McCampaignBeyondSightSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        if help_enabled > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign Beyond Sight help_enabled must be 0 or 1",
            ));
        }
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Beyond Sight activation requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Beyond Sight generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if !reset
                .shared_spells_mut()
                .activate_beyond_sight(direction_mask, help_enabled != 0)
            {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Beyond Sight holder is not active",
                ));
            }
            Ok(campaign_beyond_sight_snapshot(
                generation,
                reset.shared_spells().beyond_sight_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_opponent_state(
    handle: u64,
    generation: u64,
    out_state: *mut McCampaignOpponentSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign opponent state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign opponent generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let participant = reset.nyphur_participant().map_err(|error| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("campaign opponent reset census is invalid: {error}"),
                )
            })?;
            let visibility = reset.nyphur_visibility().map_err(|error| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("campaign opponent visibility is invalid: {error}"),
                )
            })?;
            Ok(campaign_opponent_snapshot(
                generation,
                participant,
                visibility,
            ))
        })
    })
}

/// Read the authoritative Day HUD/status/hand projection for one campaign
/// generation. The projection is reset-owned and therefore unavailable to
/// the legacy Level 1 handle.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_hud_state(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_state: *mut McCampaignDayHudSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Day HUD state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Day HUD generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if expected_tick != reset.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Day HUD tick {expected_tick} does not match {}",
                        reset.state.tick
                    ),
                ));
            }
            let selection = reset.definition_identity();
            let snapshot = reset.day_hud_for(selection).map_err(|error| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("campaign Day HUD projection is invalid: {error}"),
                )
            })?;
            Ok(campaign_day_hud_snapshot(session.generation, snapshot))
        })
    })
}

pub(super) fn campaign_day_music_snapshot(
    generation: u64,
    tick: u64,
    music: CampaignDayMusicLifecycle,
) -> McCampaignDayMusicSnapshotV2 {
    McCampaignDayMusicSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignDayMusicSnapshotV2>(),
        generation,
        tick,
        asset_slot: music.asset_slot,
        start_count: music.start_count,
        update_count: music.update_count,
        fade_callback_count: music.fade_callback_count,
        stop_count: music.stop_count,
        end_count: music.end_count,
        initialized: u8::from(music.initialized),
        loaded: u8::from(music.loaded),
        active: u8::from(music.active),
        playing: u8::from(music.playing),
        track: music.track,
        volume: music.volume,
        updates_enabled: u8::from(music.envelope.updates_enabled),
        timer_active: u8::from(music.envelope.timer_active),
        target_mode: music.envelope.target_mode,
        direction: music.envelope.direction,
        current_units: music.envelope.current,
        timer_multiplier: music.envelope.timer_multiplier,
        timer_hz: DynamicMusicEnvelope::timer_hz(music.envelope.timer_multiplier),
        reserved_0: 0,
        state_hash: music.state_hash(),
        reserved: [0; 2],
    }
}

pub(super) fn with_level2_campaign_day_music<T>(
    session: &CampaignSessionRecord,
    generation: u64,
    expected_tick: u64,
    operation: impl FnOnce(CampaignDayMusicLifecycle) -> AbiResult<T>,
) -> AbiResult<T> {
    let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "campaign Day music requires a Level 2 campaign session",
        ));
    };
    if generation != session.generation || expected_tick != reset.state.tick {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "campaign Day music generation/tick {generation}/{expected_tick} does not match {}/{}",
                session.generation, reset.state.tick
            ),
        ));
    }
    let music = session.day_music.ok_or_else(|| {
        AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            "campaign Day music requires the qualified Level 2 owner cache",
        )
    })?;
    operation(music)
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_music_state(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_state: *mut McCampaignDayMusicSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_presentation_session_mut(handle, |session, _| {
            with_level2_campaign_day_music(session, generation, expected_tick, |music| {
                Ok(campaign_day_music_snapshot(
                    generation,
                    expected_tick,
                    music,
                ))
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_music_advance(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    callback_count: u32,
    out_state: *mut McCampaignDayMusicSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        if callback_count > 1_024 {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                "campaign Day music callback count exceeds 1024",
            ));
        }
        with_campaign_presentation_session_mut(handle, |session, _| {
            with_level2_campaign_day_music(session, generation, expected_tick, |_| Ok(()))?;
            let music = session.day_music.as_mut().expect("validated Day music");
            music.advance(callback_count);
            Ok(campaign_day_music_snapshot(
                generation,
                expected_tick,
                *music,
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_music_stop(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_state: *mut McCampaignDayMusicSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_presentation_session_mut(handle, |session, _| {
            with_level2_campaign_day_music(session, generation, expected_tick, |_| Ok(()))?;
            let music = session.day_music.as_mut().expect("validated Day music");
            music.stop();
            Ok(campaign_day_music_snapshot(
                generation,
                expected_tick,
                *music,
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_music_end(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_state: *mut McCampaignDayMusicSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_presentation_session_mut(handle, |session, _| {
            with_level2_campaign_day_music(session, generation, expected_tick, |_| Ok(()))?;
            let music = session.day_music.as_mut().expect("validated Day music");
            music.end();
            Ok(campaign_day_music_snapshot(
                generation,
                expected_tick,
                *music,
            ))
        })
    })
}

pub(super) fn campaign_minimap_projection(
    session: &CampaignSessionRecord,
    generation: u64,
    expected_tick: u64,
) -> AbiResult<(Level2MinimapPlan, u16)> {
    let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "campaign minimap projection requires a Level 2 campaign session",
        ));
    };
    if generation != session.generation {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "campaign minimap generation {generation} does not match {}",
                session.generation
            ),
        ));
    }
    if expected_tick != reset.state.tick {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "campaign minimap tick {expected_tick} does not match {}",
                reset.state.tick
            ),
        ));
    }
    let local_player_slot = usize::from(reset.runtime().local_player_slot);
    let local_player_entity_index = reset
        .players()
        .get(local_player_slot)
        .ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign minimap local player slot is unavailable",
            )
        })?
        .entity_index;
    let selection = reset.definition_identity();
    let plan = campaign_minimap_plan(reset, selection).map_err(|error| {
        AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            format!("campaign minimap projection is invalid: {error}"),
        )
    })?;
    Ok((plan, local_player_entity_index))
}

pub(super) fn minimap_hash_update(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(bytes);
}

pub(super) fn level2_minimap_entity_hash(rows: &[Level2MinimapEntity]) -> u64 {
    let mut hasher = Sha256::new();
    for row in rows {
        minimap_hash_update(&mut hasher, &row.source_order.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.subject_entity_index.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.x.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.y.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.selector.to_le_bytes());
        minimap_hash_update(
            &mut hasher,
            &[
                row.palette_index,
                row.kind,
                row.size,
                row.anchor,
                row.help_selector,
                row.visible,
            ],
        );
    }
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[..8].try_into().expect("sha256 has eight bytes"))
}

pub(super) fn level2_minimap_mark_hash(rows: &[Level2MinimapMark]) -> u64 {
    let mut hasher = Sha256::new();
    for row in rows {
        minimap_hash_update(&mut hasher, &row.source_order.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.subject_entity_index.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.x.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.y.to_le_bytes());
        minimap_hash_update(
            &mut hasher,
            &[row.palette_index, row.kind, row.help_selector, row.visible],
        );
    }
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[..8].try_into().expect("sha256 has eight bytes"))
}

pub(super) fn campaign_minimap_state_snapshot(
    generation: u64,
    tick: u64,
    local_player_entity_index: u16,
    plan: &Level2MinimapPlan,
) -> McCampaignMinimapStateV2 {
    McCampaignMinimapStateV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignMinimapStateV2>(),
        generation,
        tick,
        local_player_entity_index,
        entity_count: u32::try_from(plan.entities.len()).expect("minimap entity count fits u32"),
        mark_count: u32::try_from(plan.marks.len()).expect("minimap mark count fits u32"),
        entity_hash: level2_minimap_entity_hash(&plan.entities),
        mark_hash: level2_minimap_mark_hash(&plan.marks),
        visibility_flags: plan.visibility_flags,
        ..McCampaignMinimapStateV2::default()
    }
}

pub(super) fn campaign_minimap_entity_snapshot(
    generation: u64,
    tick: u64,
    row: Level2MinimapEntity,
) -> McCampaignMinimapEntityV2 {
    McCampaignMinimapEntityV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignMinimapEntityV2>(),
        generation,
        tick,
        source_order: row.source_order,
        subject_entity_index: row.subject_entity_index,
        x: row.x,
        y: row.y,
        selector: row.selector,
        palette_index: row.palette_index,
        kind: row.kind,
        size: row.size,
        anchor: row.anchor,
        help_selector: row.help_selector,
        visible: row.visible,
        ..McCampaignMinimapEntityV2::default()
    }
}

pub(super) fn campaign_minimap_mark_snapshot(
    generation: u64,
    tick: u64,
    row: Level2MinimapMark,
) -> McCampaignMinimapMarkV2 {
    McCampaignMinimapMarkV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignMinimapMarkV2>(),
        generation,
        tick,
        source_order: row.source_order,
        subject_entity_index: row.subject_entity_index,
        x: row.x,
        y: row.y,
        palette_index: row.palette_index,
        kind: row.kind,
        help_selector: row.help_selector,
        visible: row.visible,
        ..McCampaignMinimapMarkV2::default()
    }
}

pub(super) fn copy_minimap_rows<T: Copy>(
    rows: &[T],
    start_ordinal: u32,
    buffer: *mut T,
    capacity: u32,
    out_written: *mut u32,
) -> AbiResult<()> {
    let start = usize::try_from(start_ordinal)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "minimap start ordinal exceeds usize"))?;
    if start > rows.len() {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("minimap start ordinal {start} exceeds {}", rows.len()),
        ));
    }
    let capacity = usize::try_from(capacity)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "minimap capacity exceeds usize"))?;
    let count = rows.len().saturating_sub(start).min(capacity);
    if count != 0 && buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            "minimap row output buffer is null",
        ));
    }
    // SAFETY: count is bounded by the caller's capacity and the immutable
    // source slice; the native ABI owns no alias of the destination.
    unsafe { buffer.copy_from_nonoverlapping(rows[start..].as_ptr(), count) };
    write_output(
        out_written,
        u32::try_from(count).expect("minimap row count fits u32"),
    );
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_minimap_state(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_state: *mut McCampaignMinimapStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let (plan, local_player_entity_index) =
                campaign_minimap_projection(session, generation, expected_tick)?;
            Ok(campaign_minimap_state_snapshot(
                generation,
                expected_tick,
                local_player_entity_index,
                &plan,
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_minimap_entities_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McCampaignMinimapEntityV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_element_size::<McCampaignMinimapEntityV2>(element_size, "campaign minimap entity")?;
        require_output(out_written, "minimap entity count")?;
        with_campaign_session(handle, |session| {
            let (plan, _) = campaign_minimap_projection(session, generation, expected_tick)?;
            let rows = plan
                .entities
                .iter()
                .copied()
                .map(|row| campaign_minimap_entity_snapshot(generation, expected_tick, row))
                .collect::<Vec<_>>();
            copy_minimap_rows(&rows, start_ordinal, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_minimap_marks_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McCampaignMinimapMarkV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_element_size::<McCampaignMinimapMarkV2>(element_size, "campaign minimap mark")?;
        require_output(out_written, "minimap mark count")?;
        with_campaign_session(handle, |session| {
            let (plan, _) = campaign_minimap_projection(session, generation, expected_tick)?;
            let rows = plan
                .marks
                .iter()
                .copied()
                .map(|row| campaign_minimap_mark_snapshot(generation, expected_tick, row))
                .collect::<Vec<_>>();
            copy_minimap_rows(&rows, start_ordinal, buffer, capacity, out_written)
        })
    })
}

/// Execute one additive Level 2 outer turn.  This path deliberately owns a
/// separate campaign handle and never enters the legacy Level 1 session
/// dispatcher.  The candidate session is stepped on a clone first so a short
/// event buffer or invalid output cannot partially mutate the live handle.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_step(
    handle: u64,
    input: *const McCampaignStepInputV2,
    out_snapshot: *mut McCampaignStepSnapshotV2,
    out_snapshot_size: u32,
    events: *mut McCampaignEventV2,
    event_capacity: u32,
    event_element_size: u32,
    out_event_count: *mut u32,
) -> i32 {
    boundary(|| {
        require_struct_output::<McCampaignStepSnapshotV2>(
            out_snapshot,
            out_snapshot_size,
            "campaign step snapshot",
        )?;
        require_output(out_event_count, "campaign step event count")?;
        require_element_size::<McCampaignEventV2>(event_element_size, "campaign step event")?;
        let input = read_versioned_input(input, "campaign step input")?;
        validate_reserved(&input.reserved, "campaign step input")?;
        if input.reserved_0 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "campaign step input reserved_0 must be zero",
            ));
        }
        if input.speed_index > 2 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign step speed_index must be 0, 1, or 2",
            ));
        }
        validate_embedded_command(&input.command)?;
        let command = player_command(input.command)?;
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign step requires a Level 2 campaign session",
                ));
            };
            if input.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign step generation {} does not match {}",
                        input.generation, session.generation
                    ),
                ));
            }
            let player = reset
                .players()
                .get(usize::from(input.player_slot))
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!(
                            "campaign player slot {} is out of bounds",
                            input.player_slot
                        ),
                    )
                })?;
            if player.active_player_index < 0 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("campaign player slot {} is inactive", input.player_slot),
                ));
            }
            if input.player_slot != reset.runtime().local_player_slot {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!(
                        "campaign step player slot {} is not the local slot {}",
                        input.player_slot,
                        reset.runtime().local_player_slot
                    ),
                ));
            }
            let player_entity_index = player.entity_index;
            let mut candidate_record = session.clone();
            let snapshot = {
                let CampaignSessionRecord {
                    kind,
                    generation,
                    day_music,
                    effect_state_hashes,
                } = &mut candidate_record;
                let CampaignSessionKind::Level2 { reset, .. } = kind else {
                    unreachable!("candidate preserves the validated Level 2 variant")
                };
                let effect_head_before = reset
                    .descriptor()
                    .ok()
                    .map(|campaign| campaign.effect_cursor().next_sequence);
                let music_event_start = reset.context.events.len();
                reset.state.entity_index = player_entity_index;
                reset.state.position = Position {
                    x: input.player_x,
                    y: input.player_y,
                    z: input.player_z,
                };
                reset.step_authority(command, input.speed_index);

                if let Some(day_music) = day_music.as_mut() {
                    for event in &reset.context.events[music_event_start..] {
                        if let FlightEvent::UpdateMusic {
                            mode,
                            timer_multiplier,
                        } = *event
                        {
                            day_music.update(mode, timer_multiplier);
                        }
                    }
                }

                let state_hash = reset.campaign_state_hash();
                if let (Some(first), Ok(campaign)) = (effect_head_before, reset.descriptor()) {
                    let next = campaign.effect_cursor().next_sequence;
                    for sequence in first..next {
                        effect_state_hashes.insert(sequence, state_hash);
                    }
                    while effect_state_hashes.len() > 16_384 {
                        let _ = effect_state_hashes.pop_first();
                    }
                }

                let event_rows = reset
                    .dispatch_events()
                    .iter()
                    .enumerate()
                    .map(|(ordinal, event)| campaign_event_snapshot(*generation, ordinal, event))
                    .collect::<Vec<_>>();
                copy_slice(&event_rows, events, event_capacity, out_event_count)?;
                let mut event_kind_mask = 0_u32;
                for event in &event_rows {
                    if (1..=32).contains(&event.kind) {
                        event_kind_mask |= 1_u32 << u32::from(event.kind - 1);
                    }
                }
                McCampaignStepSnapshotV2 {
                    abi_version: MC_NATIVE_ABI_VERSION,
                    struct_size: struct_size::<McCampaignStepSnapshotV2>(),
                    generation: *generation,
                    revision: reset.state.tick,
                    state_hash,
                    terrain_hash: reset.terrain_hash(),
                    dispatch_hash: decode_sha256_hex(&reset.dispatch_observability_hash()),
                    event_count: u32::try_from(event_rows.len()).map_err(|_| {
                        AbiError::new(MC_STATUS_BOUNDS, "campaign event count exceeds u32")
                    })?,
                    event_kind_mask,
                    player_slot: input.player_slot,
                    reserved_0: [0; 3],
                    player_entity_index: reset.state.entity_index,
                    reserved_1: 0,
                    x: reset.state.position.x,
                    y: reset.state.position.y,
                    z: reset.state.position.z,
                    reserved: [0; 4],
                }
            };
            *session = candidate_record;
            Ok(snapshot)
        })
        .map(|snapshot| {
            write_output(out_snapshot, snapshot);
            ()
        })
    })
}

/// Page the canonical descriptor-backed campaign EffectJournal through the
/// frozen revisioned event layouts. `cursor` is the number of effects already
/// consumed; projected event sequences remain one-based like the sealed ABI.
pub(super) fn campaign_effect_page_error(error: EffectPageError) -> AbiError {
    match error {
        EffectPageError::StaleGeneration { expected, actual } => AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!("campaign effect generation {actual} does not match {expected}"),
        ),
        EffectPageError::ExpiredCursor {
            first_available,
            requested,
        } => AbiError::new(
            MC_STATUS_CURSOR_EXPIRED,
            format!("campaign effect cursor {requested} predates {first_available}"),
        ),
        EffectPageError::FutureCursor {
            next_sequence,
            requested,
        } => AbiError::new(
            MC_STATUS_BOUNDS,
            format!("campaign effect cursor {requested} is beyond {next_sequence}"),
        ),
        EffectPageError::OutputTooSmall { pending } => AbiError::new(
            MC_STATUS_BUFFER_TOO_SMALL,
            format!("campaign effect page has {pending} pending rows"),
        ),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_events_copy_since(
    handle: u64,
    query: *const McEventQueryV2,
    buffer: *mut McEventV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McEventBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McEventV2>(element_size, "campaign effect event")?;
        let query = read_versioned_input(query, "campaign effect query")?;
        validate_reserved(&query.reserved, "campaign effect query")?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "generic campaign effects require a descriptor-backed session",
                ));
            };
            let campaign = reset.descriptor()?;
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign effect generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            let page = campaign
                .effects_after(
                    EffectCursor {
                        // Runtime pause/resume advances the public session
                        // generation without resetting the campaign journal.
                        generation: campaign.generation(),
                        next_sequence: query.cursor,
                    },
                    usize::try_from(capacity).map_err(|_| {
                        AbiError::new(MC_STATUS_BOUNDS, "campaign effect capacity exceeds usize")
                    })?,
                )
                .map_err(campaign_effect_page_error)?;
            if !page.records.is_empty() && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign effect output buffer is null",
                ));
            }
            let current_state_hash = campaign.state_hash();
            let rows = page
                .records
                .iter()
                .map(|record| {
                    campaign_effect_snapshot(
                        query.generation,
                        session
                            .effect_state_hashes
                            .get(&record.sequence)
                            .copied()
                            .unwrap_or(current_state_hash),
                        record,
                    )
                })
                .collect::<Vec<_>>();
            for (index, row) in rows.iter().enumerate() {
                // SAFETY: non-null was checked for the bounded page and the
                // caller supplied capacity records. Unaligned C storage is an
                // intentional v2 ABI capability.
                unsafe { buffer.add(index).write_unaligned(*row) };
            }
            let head = campaign.effect_cursor().next_sequence;
            Ok(McEventBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McEventBatchV2>(),
                generation: query.generation,
                next_cursor: page.next_cursor.next_sequence,
                event_head: head,
                written: u32::try_from(rows.len()).expect("bounded effect page fits u32"),
                has_more: u8::from(page.next_cursor.next_sequence < head),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

/// Copy the current descriptor-backed campaign presentation through the
/// frozen generic v2 frame layout.
///
/// The qualified Level 2 producer is presently an indexed 320x200 Day
/// terrain frame. It deliberately reports no provenance or compositor-
/// complete flags: this bridge exposes the real shared presentation subset,
/// not a claim that the retained Level 2 client composition is folded yet.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_presentation_frame_copy(
    handle: u64,
    query: *const McPresentationFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    provenance_buffer: *mut u8,
    provenance_capacity: u32,
    out_info: *mut McPresentationFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "campaign presentation frame query")?;
        validate_reserved(&query.reserved, "campaign presentation frame query")?;
        if query.reserved_0 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "campaign presentation frame query reserved_0 must be zero",
            ));
        }
        if query.viewport_width != LEVEL1_VIEW_WIDTH as u16
            || query.viewport_height != LEVEL1_VIEW_HEIGHT as u16
            || query.screen_stride != LEVEL1_VIEW_WIDTH as u16
            || query.vga_resolution != 1
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign presentation frame requires the qualified 320x200 Day profile",
            ));
        }
        for (name, value) in [
            ("configured_blur_mode", query.configured_blur_mode),
            ("blur_capability", query.blur_capability),
            ("high_speed_system", query.high_speed_system),
            ("scratch_present", query.scratch_present),
            ("spatial_smoothing", query.spatial_smoothing),
            ("minimap_enabled", query.minimap_enabled),
            ("top_bar_enabled", query.top_bar_enabled),
            ("menu_overlay", query.menu_overlay),
            ("game_paused", query.game_paused),
            ("level_end", query.level_end),
            ("objective_selector", query.objective_selector),
            ("objective_textbox", query.objective_textbox),
            ("tutorial_or_briefing", query.tutorial_or_briefing),
            ("help_popup", query.help_popup),
            ("debug_text", query.debug_text),
        ] {
            if value != 0 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("campaign presentation frame does not yet support nonzero {name}"),
                ));
            }
        }
        let required =
            u32::try_from(LEVEL1_VIEW_PIXELS).expect("qualified Day frame extent fits u32");
        if index_capacity < required {
            return Err(AbiError::new(
                MC_STATUS_BUFFER_TOO_SMALL,
                format!(
                    "campaign presentation frame requires {required} index bytes, got {index_capacity}"
                ),
            ));
        }
        if index_buffer.is_null() {
            return Err(AbiError::new(
                MC_STATUS_NULL_POINTER,
                "campaign presentation index buffer is null",
            ));
        }
        // The current Day frame has no retained provenance plane. Accept a
        // null, zero-capacity sidecar and report zero bytes instead of
        // fabricating provenance or claiming compositor completeness.
        let _ = (provenance_buffer, provenance_capacity);
        with_campaign_presentation_session_mut(handle, |session, borrowed_runtime| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign presentation generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            let CampaignSessionKind::Level2 {
                inspection,
                reset,
                day_frame_assets,
                day_frame_state,
            } = &mut session.kind
            else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign presentation requires a descriptor-backed session",
                ));
            };
            let campaign = reset.descriptor()?;
            if !borrowed_runtime && campaign.generation() != query.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "campaign presentation generation does not match its Rust authority",
                ));
            }
            if query.tick != campaign.tick() || query.presentation_revision != query.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign presentation revision {}/{} does not match tick {}",
                        query.tick,
                        query.presentation_revision,
                        campaign.tick()
                    ),
                ));
            }
            let assets = day_frame_assets.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "campaign presentation requires a qualified owner cache",
                )
            })?;
            let newly_committed = match day_frame_state.current_frame() {
                Some(frame) if frame.generation == query.generation && frame.tick == query.tick => {
                    false
                }
                Some(_) | None => {
                    day_frame_state
                        .render_current(inspection, reset, assets, query.generation)
                        .map_err(|error| {
                            AbiError::new(
                                MC_STATUS_VALIDATION_FAILED,
                                format!("campaign presentation render failed: {error}"),
                            )
                        })?;
                    true
                }
            };
            let frame = day_frame_state
                .current_frame()
                .expect("successful Day render commits a current frame");
            // SAFETY: the index buffer was checked non-null and large enough
            // for the exact fixed-profile frame before any presentation state
            // was mutated.
            unsafe {
                index_buffer.copy_from_nonoverlapping(frame.indices.as_ptr(), LEVEL1_VIEW_PIXELS)
            };
            Ok(McPresentationFrameInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McPresentationFrameInfoV2>(),
                generation: query.generation,
                tick: query.tick,
                presentation_revision: query.presentation_revision,
                immutable_input_hash: frame.terrain_input_hash,
                width: frame.width,
                height: frame.height,
                stride: frame.width,
                viewport_width: query.viewport_width,
                index_bytes: required,
                provenance_bytes: 0,
                flags: MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN
                    | if frame.profile.map_type_id == 2 {
                        0
                    } else {
                        MC_TERRAIN_FRAME_SKY_SEEDED
                    }
                    | MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED,
                effective_postprocess_mode: 0,
                history_state: 1,
                newly_committed: u8::from(newly_committed),
                reserved_0: 0,
                index_hash: frame.index_hash,
                provenance_hash: 0,
                lighting_hash: 0,
                source_arena_hash: 0,
                overlay_pixels_written: 0,
                overlay_stage_count: 0,
                reserved_1: 0,
                reserved: [0; 6],
            })
        })
    })
}

/// Copy the detailed Level 2 economy sidecars from the most recent outer
/// turn.  Generic `McCampaignEventV2` rows remain the stable 48-byte event
/// stream; callers join these rows by `generation + ordinal`.  Level 1
/// campaign handles and reset/initial states have no kind-10 sidecars.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_economy_events(
    handle: u64,
    events: *mut McCampaignEconomyEventV2,
    event_capacity: u32,
    event_element_size: u32,
    out_event_count: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_event_count, "campaign economy event count")?;
        require_element_size::<McCampaignEconomyEventV2>(
            event_element_size,
            "campaign economy event",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign economy events require a Level 2 campaign session",
                ));
            };
            let rows = reset
                .dispatch_events()
                .iter()
                .enumerate()
                .filter_map(|(ordinal, dispatch)| match dispatch {
                    Level2DispatchEvent::Economy {
                        pass,
                        source_slot,
                        event,
                    } => Some(campaign_economy_event_snapshot(
                        session.generation,
                        ordinal,
                        *pass,
                        *source_slot,
                        *event,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            copy_slice(&rows, events, event_capacity, out_event_count)
        })
    })
}

/// Copy the detailed authored-group activation sidecars from the most recent
/// outer turn.  Generic `McCampaignEventV2` rows remain the stable 48-byte
/// event stream; callers join these rows by `generation + ordinal` and use
/// `event_index` to reconstruct the exact source-ordered authored list.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_authored_group_events(
    handle: u64,
    events: *mut McCampaignAuthoredGroupEventV2,
    event_capacity: u32,
    event_element_size: u32,
    out_event_count: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_event_count, "campaign authored-group event count")?;
        require_element_size::<McCampaignAuthoredGroupEventV2>(
            event_element_size,
            "campaign authored-group event",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign authored-group events require a Level 2 campaign session",
                ));
            };
            let mut rows = Vec::new();
            for (ordinal, dispatch) in reset.dispatch_events().iter().enumerate() {
                let Level2DispatchEvent::AuthoredGroupActivated {
                    pass,
                    source_slot,
                    group,
                    authored_indices,
                } = dispatch
                else {
                    continue;
                };
                let flags = u8::try_from(authored_indices.len()).unwrap_or(u8::MAX);
                for (event_index, authored_index) in authored_indices.iter().copied().enumerate() {
                    rows.push(McCampaignAuthoredGroupEventV2 {
                        abi_version: MC_NATIVE_ABI_VERSION,
                        struct_size: struct_size::<McCampaignAuthoredGroupEventV2>(),
                        generation: session.generation,
                        ordinal: u32::try_from(ordinal)
                            .expect("bounded campaign event ordinal fits u32"),
                        pass: *pass,
                        flags,
                        group: *group,
                        source_slot: *source_slot,
                        event_index: u16::try_from(event_index)
                            .expect("bounded authored-group event index fits u16"),
                        authored_index,
                        reserved_0: 0,
                        reserved: [0; 4],
                    });
                }
            }
            copy_slice(&rows, events, event_capacity, out_event_count)
        })
    })
}

/// Copy the detailed Level 2 Goat sidecars from the most recent outer turn.
/// Generic `McCampaignEventV2` rows remain the stable 48-byte event stream;
/// callers join these rows by `generation + ordinal`, and use `event_index`
/// when one Goat dispatch row emits multiple typed Goat payloads.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_goat_events(
    handle: u64,
    events: *mut McCampaignGoatEventV2,
    event_capacity: u32,
    event_element_size: u32,
    out_event_count: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_event_count, "campaign goat event count")?;
        require_element_size::<McCampaignGoatEventV2>(event_element_size, "campaign goat event")?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign goat events require a Level 2 campaign session",
                ));
            };
            let mut rows = Vec::new();
            for (ordinal, dispatch) in reset.dispatch_events().iter().enumerate() {
                let Level2DispatchEvent::Goat {
                    pass,
                    source_slot,
                    events,
                } = dispatch
                else {
                    continue;
                };
                for (event_index, event) in events.iter().copied().enumerate() {
                    rows.push(campaign_goat_event_snapshot(
                        session.generation,
                        ordinal,
                        event_index,
                        *pass,
                        *source_slot,
                        event,
                    ));
                }
            }
            copy_slice(&rows, events, event_capacity, out_event_count)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_world_info(
    handle: u64,
    out_info: *mut McCampaignWorldInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_campaign_session(handle, |session| {
            let (metadata, entity_count, active_count, player_count, objective_count, hash) =
                match &session.kind {
                    CampaignSessionKind::Level1 { world } => (
                        world.metadata(),
                        world.entities().len(),
                        world
                            .entities()
                            .iter()
                            .filter(|entity| entity.active)
                            .count(),
                        world.players().len(),
                        world.runtime().objectives.len(),
                        state_hash(world.reset_state()),
                    ),
                    CampaignSessionKind::Level2 {
                        inspection, reset, ..
                    } => (
                        inspection.metadata(),
                        inspection.entities().len(),
                        inspection
                            .entities()
                            .iter()
                            .filter(|entity| entity.active)
                            .count(),
                        reset.exposed_player_count(inspection),
                        reset.exposed_objective_count(inspection),
                        reset.campaign_state_hash(),
                    ),
                };
            Ok(McCampaignWorldInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignWorldInfoV2>(),
                external_level: metadata.external_level,
                internal_level: metadata.internal_level,
                map_type: metadata.map_type.id(),
                profile: profile_code(metadata.map_type == sim_core::native_world::MapType::Cave),
                reserved_0: 0,
                entity_count: u32::try_from(entity_count).expect("entity count fits u32"),
                active_entity_count: u32::try_from(active_count).expect("active count fits u32"),
                player_count: u32::try_from(player_count).expect("player count fits u32"),
                objective_count: u32::try_from(objective_count).expect("objective count fits u32"),
                terrain_width: 256,
                terrain_height: 256,
                reset_state_hash: hash,
                reserved: [0; 4],
            })
        })
    })
}

pub(super) fn campaign_day_indexed_frame_info(
    frame: &DayIndexedFrame,
) -> McCampaignDayIndexedFrameInfoV2 {
    McCampaignDayIndexedFrameInfoV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        generation: frame.generation,
        tick: frame.tick,
        width: frame.width,
        height: frame.height,
        stride: frame.width,
        reserved_0: 0,
        index_bytes: u32::try_from(frame.indices.len()).expect("Day frame index extent fits u32"),
        palette_bytes: u32::try_from(frame.palette_rgb8.len())
            .expect("Day frame palette extent fits u32"),
        index_hash: frame.index_hash,
        palette_hash: byte_hash(&frame.palette_rgb8),
        terrain_input_hash: frame.terrain_input_hash,
        terrain_command_hash: frame.terrain_command_hash,
        package_sha256: decode_sha256_hex(&frame.package_sha256),
        cache_content_sha256: decode_sha256_hex(&frame.cache_content_sha256),
        reserved: [0; 4],
    }
}

pub(super) fn with_level2_campaign_day_frame<T>(
    session: &CampaignSessionRecord,
    generation: u64,
    expected_tick: u64,
    operation: impl FnOnce(&DayIndexedFrame) -> AbiResult<T>,
) -> AbiResult<T> {
    if generation != session.generation {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "campaign Day indexed frame generation {generation} does not match {}",
                session.generation
            ),
        ));
    }
    let CampaignSessionKind::Level2 {
        reset,
        day_frame_state,
        ..
    } = &session.kind
    else {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "campaign Day indexed frame requires a Level 2 campaign session",
        ));
    };
    if expected_tick != reset.state.tick {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "campaign Day indexed frame tick {expected_tick} does not match {}",
                reset.state.tick
            ),
        ));
    }
    let frame = day_frame_state.current_frame().ok_or_else(|| {
        AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "campaign Day indexed frame must be rendered before copying",
        )
    })?;
    if frame.generation != generation || frame.tick != expected_tick {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            "campaign Day indexed frame is not bound to the requested generation/tick",
        ));
    }
    operation(frame)
}

/// Render and commit the current qualified Level 2 Day indexed frame.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_indexed_frame_render(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_info: *mut McCampaignDayIndexedFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_campaign_presentation_session_mut(handle, |session, _borrowed_runtime| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Day indexed frame generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let CampaignSessionKind::Level2 {
                inspection,
                reset,
                day_frame_assets,
                day_frame_state,
            } = &mut session.kind
            else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Day indexed frame requires a Level 2 campaign session",
                ));
            };
            if expected_tick != reset.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Day indexed frame tick {expected_tick} does not match {}",
                        reset.state.tick
                    ),
                ));
            }
            if let Some(frame) = day_frame_state.current_frame()
                && frame.generation == generation
                && frame.tick == expected_tick
            {
                return Ok(campaign_day_indexed_frame_info(frame));
            }
            let assets = day_frame_assets.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "campaign Day indexed frame requires a qualified owner cache",
                )
            })?;
            let frame = day_frame_state
                .render(inspection, reset, assets, generation)
                .map_err(|error| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        format!("campaign Day indexed frame render failed: {error}"),
                    )
                })?;
            Ok(campaign_day_indexed_frame_info(&frame))
        })
    })
}

/// Copy the complete committed Day indexed frame.  Rendering and copying are
/// separate so a pre-render or stale-generation request cannot expose an old
/// frame as current.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_indexed_frame_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    index_buffer: *mut u8,
    index_capacity: u32,
    palette_buffer: *mut u8,
    palette_capacity: u32,
    out_index_written: *mut u32,
    out_palette_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_index_written, "Day indexed frame index count")?;
        require_output(out_palette_written, "Day indexed frame palette count")?;
        write_output(out_index_written, 0);
        write_output(out_palette_written, 0);
        with_campaign_presentation_session_mut(handle, |session, _borrowed_runtime| {
            with_level2_campaign_day_frame(session, generation, expected_tick, |frame| {
                let index_required =
                    u32::try_from(frame.indices.len()).expect("Day frame index extent fits u32");
                let palette_required = u32::try_from(frame.palette_rgb8.len())
                    .expect("Day frame palette extent fits u32");
                if index_capacity < index_required || palette_capacity < palette_required {
                    return Err(AbiError::new(
                        MC_STATUS_BUFFER_TOO_SMALL,
                        format!(
                            "Day indexed frame requires {index_required} index and {palette_required} palette bytes, got {index_capacity} and {palette_capacity}"
                        ),
                    ));
                }
                if index_buffer.is_null() || palette_buffer.is_null() {
                    return Err(AbiError::new(
                        MC_STATUS_NULL_POINTER,
                        "Day indexed frame index and palette buffers must be non-null",
                    ));
                }
                // SAFETY: both caller buffers are non-null and have the exact
                // required capacities checked above; frame storage is immutable.
                unsafe {
                    index_buffer
                        .copy_from_nonoverlapping(frame.indices.as_ptr(), frame.indices.len());
                    palette_buffer.copy_from_nonoverlapping(
                        frame.palette_rgb8.as_ptr(),
                        frame.palette_rgb8.len(),
                    );
                }
                write_output(out_index_written, index_required);
                write_output(out_palette_written, palette_required);
                Ok(())
            })
        })
    })
}

pub(super) fn with_level2_campaign_terrain<T>(
    handle: u64,
    generation: u64,
    operation: impl FnOnce(&Level2ResetSession) -> AbiResult<T>,
) -> AbiResult<T> {
    with_campaign_session(handle, |session| {
        if generation != session.generation {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                format!(
                    "campaign terrain generation {} does not match session generation {}",
                    generation, session.generation
                ),
            ));
        }
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign terrain requires a Level 2 campaign session",
            ));
        };
        operation(reset)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_terrain_state(
    handle: u64,
    generation: u64,
    out_state: *mut McCampaignTerrainStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_level2_campaign_terrain(handle, generation, |reset| {
            Ok(McCampaignTerrainStateV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignTerrainStateV2>(),
                generation,
                tick: reset.state.tick,
                mutation_sequence: reset.canonical_terrain().mutation_sequence(),
                terrain_hash: reset.terrain_hash(),
                width: 256,
                height: 256,
                plane_count: 6,
                hash_schema: 1,
                authority: MC_AUTHORITY_SESSION_TERRAIN_LIVE,
                reserved_0: 0,
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_terrain_plane_length(
    handle: u64,
    generation: u64,
    kind: u32,
    out_count: *mut u32,
) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_level2_campaign_terrain(handle, generation, |reset| {
            validate_plane_kind(reset.canonical_terrain().planes(), kind)?;
            Ok(u32::try_from(TERRAIN_CELLS).expect("terrain count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_terrain_plane_copy_u8(
    handle: u64,
    generation: u64,
    kind: u32,
    buffer: *mut u8,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        with_level2_campaign_terrain(handle, generation, |reset| {
            let values = terrain_u8_plane(reset.canonical_terrain().planes(), kind)?;
            copy_slice(values, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_terrain_entity_indices_copy(
    handle: u64,
    generation: u64,
    buffer: *mut i16,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        with_level2_campaign_terrain(handle, generation, |reset| {
            copy_slice(
                &reset.canonical_terrain().planes().entity_indices,
                buffer,
                capacity,
                out_written,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_player_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_campaign_session(handle, |session| {
            let count = match &session.kind {
                CampaignSessionKind::Level1 { world } => world.players().len(),
                CampaignSessionKind::Level2 {
                    inspection, reset, ..
                } => reset.exposed_player_count(inspection),
            };
            u32::try_from(count)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "player count exceeds u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_objective_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_campaign_session(handle, |session| {
            let count = match &session.kind {
                CampaignSessionKind::Level1 { world } => world.runtime().objectives.len(),
                CampaignSessionKind::Level2 {
                    inspection, reset, ..
                } => reset.exposed_objective_count(inspection),
            };
            u32::try_from(count)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "objective count exceeds u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_entity_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_campaign_session(handle, |session| {
            let count = match &session.kind {
                CampaignSessionKind::Level1 { world } => world.entities().len(),
                CampaignSessionKind::Level2 { inspection, .. } => inspection.entities().len(),
            };
            u32::try_from(count)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "entity count exceeds u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_players_copy(
    handle: u64,
    start_slot: u32,
    buffer: *mut McCampaignPlayerSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignPlayerSnapshotV2>(
            element_size,
            "campaign player element",
        )?;
        with_campaign_session(handle, |session| {
            let (players, exposed_count, participant_count) = match &session.kind {
                CampaignSessionKind::Level1 { world } => (
                    world.players(),
                    world.players().len(),
                    usize::from(world.runtime().player_count),
                ),
                CampaignSessionKind::Level2 {
                    inspection, reset, ..
                } => {
                    // Level 2 menu selection, XP unlock projection, and hand
                    // assignment mutate the live shared player rows.  Return
                    // that authoritative session projection rather than the
                    // immutable owner-package inspection snapshot.
                    (
                        reset.shared_spells().players(),
                        reset.exposed_player_count(inspection),
                        reset.active_participant_count(),
                    )
                }
            };
            let start = usize::try_from(start_slot).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign player start exceeds usize")
            })?;
            let player_count = players.len().min(exposed_count);
            if start > player_count {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign player start out of bounds",
                ));
            }
            let count = (player_count - start).min(usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign player capacity exceeds usize")
            })?);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign player buffer is null",
                ));
            }
            for (offset, player) in players.iter().skip(start).take(count).enumerate() {
                let value =
                    campaign_player_snapshot(player, usize::from(player.slot) < participant_count);
                unsafe { buffer.add(offset).write_unaligned(value) };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("player count fits u32"),
            );
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_objectives_copy(
    handle: u64,
    start_slot: u32,
    buffer: *mut McObjectiveSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McObjectiveSnapshotV2>(element_size, "campaign objective element")?;
        with_campaign_session(handle, |session| {
            let objectives = match &session.kind {
                CampaignSessionKind::Level1 { world } => &world.runtime().objectives[..],
                CampaignSessionKind::Level2 {
                    inspection, reset, ..
                } => reset
                    .runtime()
                    .objectives
                    .get(..reset.exposed_objective_count(inspection))
                    .ok_or_else(|| {
                        AbiError::new(
                            MC_STATUS_BOUNDS,
                            "campaign objective definition exceeds runtime capacity",
                        )
                    })?,
            };
            let start = usize::try_from(start_slot).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign objective start exceeds usize")
            })?;
            if start > objectives.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign objective start out of bounds",
                ));
            }
            let count =
                (objectives.len() - start).min(usize::try_from(capacity).map_err(|_| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        "campaign objective capacity exceeds usize",
                    )
                })?);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign objective buffer is null",
                ));
            }
            for (offset, objective) in objectives.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer.add(offset).write_unaligned(objective_snapshot(
                        u8::try_from(start + offset).expect("objective slot fits u8"),
                        objective,
                    ))
                };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("objective count fits u32"),
            );
            Ok(())
        })
    })
}

/// Read one mutable Level 2 objective stage from the generation-bound reset
/// session.  This deliberately does not repurpose the frozen objective-copy
/// contract: Unity needs the authored stage axes and the live selector state
/// that the source model-32 switch consumes.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_objective_stage_state(
    handle: u64,
    generation: u64,
    objective_slot: u8,
    stage_slot: u8,
    out_state: *mut McCampaignObjectiveStageSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign objective stage state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign objective stage generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let runtime = reset.runtime();
            let objective = runtime
                .objectives
                .get(usize::from(objective_slot))
                .ok_or_else(|| {
                    AbiError::new(MC_STATUS_BOUNDS, "campaign objective slot is out of bounds")
                })?;
            let stage = runtime.stages.get(usize::from(stage_slot)).ok_or_else(|| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign stage slot is out of bounds")
            })?;
            let stage_state = objective
                .stage_selectors
                .get(usize::from(stage_slot))
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        "campaign objective stage slot is out of bounds",
                    )
                })?;
            Ok(campaign_objective_stage_snapshot(
                generation,
                objective_slot,
                stage_slot,
                *objective,
                stage,
                stage_state,
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_entities_copy(
    handle: u64,
    start_index: u32,
    buffer: *mut McCampaignEntitySnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignEntitySnapshotV2>(
            element_size,
            "campaign entity element",
        )?;
        with_campaign_session(handle, |session| {
            let mut entities = match &session.kind {
                CampaignSessionKind::Level1 { world } => world
                    .entities()
                    .iter()
                    .map(campaign_entity_snapshot)
                    .collect::<Vec<_>>(),
                CampaignSessionKind::Level2 { reset, .. } => {
                    let mut rows = reset
                        .entities()
                        .iter()
                        .map(campaign_entity_snapshot)
                        .collect::<Vec<_>>();
                    let mut occupied = BTreeSet::new();
                    for source in reset.economy().bee_creatures() {
                        let slot = usize::from(source.source_entity_index);
                        if slot >= rows.len() || !occupied.insert(source.source_entity_index) {
                            return Err(AbiError::new(
                                MC_STATUS_VALIDATION_FAILED,
                                format!(
                                    "campaign live Bee source slot {} is invalid or duplicated",
                                    source.source_entity_index
                                ),
                            ));
                        }
                        rows[slot] = campaign_live_bee_entity_snapshot(source);
                    }
                    for source in reset.economy().authored_class5() {
                        let slot = usize::from(source.source_entity_index);
                        if slot >= rows.len() || !occupied.insert(source.source_entity_index) {
                            return Err(AbiError::new(
                                MC_STATUS_VALIDATION_FAILED,
                                format!(
                                    "campaign live class-5 source slot {} is invalid or duplicated",
                                    source.source_entity_index
                                ),
                            ));
                        }
                        rows[slot] = campaign_live_class5_entity_snapshot(source);
                        if source.model == 4
                            && let Some(archer) = reset
                                .economy()
                                .authored_class5_archer()
                                .iter()
                                .find(|archer| {
                                    archer.source_entity_index == source.source_entity_index
                                })
                        {
                            rows[slot].source_id = archer.entity_id;
                            rows[slot].active &= u8::from(archer.control_flags & 0x421 == 0);
                        }
                    }
                    for source in reset.economy().castle_defenders() {
                        let slot = usize::from(source.source_entity_index);
                        if slot >= rows.len() || !occupied.insert(source.source_entity_index) {
                            return Err(AbiError::new(
                                MC_STATUS_VALIDATION_FAILED,
                                format!(
                                    "campaign Castle defender source slot {} is invalid or duplicated",
                                    source.source_entity_index
                                ),
                            ));
                        }
                        rows[slot] = McCampaignEntitySnapshotV2 {
                            abi_version: MC_NATIVE_ABI_VERSION,
                            struct_size: struct_size::<McCampaignEntitySnapshotV2>(),
                            entity_index: source.source_entity_index,
                            source_id: source.id_entity_index,
                            next_relation: u16::MAX,
                            parameter_relation: 83,
                            authority: MC_AUTHORITY_CAMPAIGN_LIVE,
                            active: u8::from(source.active && !source.drawing_disabled()),
                            class_id: 5,
                            model: 15,
                            action: source.action,
                            phase: source.source_byte_3e,
                            x: source.position.x,
                            y: source.position.y,
                            z: source.position.z,
                            yaw: source.yaw.cast_signed(),
                            pitch: source.pitch,
                            fov: source.orientation_fov,
                            actual_speed: source.speed,
                            reserved: [0; 4],
                        };
                    }
                    // Authored pickup constructors can reuse an imported
                    // slot. The shared-spell record then owns its live pose
                    // and lifetime, just as in campaign_entity_presentations.
                    let arena_slots = reset
                        .economy()
                        .live_arena_slots()
                        .into_iter()
                        .map(|(slot, _)| slot)
                        .collect::<BTreeSet<_>>();
                    for source in reset
                        .shared_spells()
                        .dynamic_pickups()
                        .iter()
                        .filter(|row| {
                            row.active && row.class == 15 && arena_slots.contains(&row.index)
                        })
                    {
                        let slot = usize::from(source.index);
                        if slot >= rows.len() || !occupied.insert(source.index) {
                            return Err(AbiError::new(
                                MC_STATUS_VALIDATION_FAILED,
                                format!(
                                    "campaign live pickup source slot {} is invalid or duplicated",
                                    source.index
                                ),
                            ));
                        }
                        rows[slot] = campaign_entity_snapshot(source);
                        rows[slot].authority = MC_AUTHORITY_CAMPAIGN_LIVE;
                        rows[slot].active = u8::from(
                            !source.drawing_disabled() && source.control_flags & 0x21 == 0,
                        );
                    }
                    for source in reset.field_source_rows() {
                        let slot = usize::from(source.index);
                        if slot >= rows.len() || !occupied.insert(source.index) {
                            return Err(AbiError::new(
                                MC_STATUS_VALIDATION_FAILED,
                                format!(
                                    "campaign live field source slot {} is invalid or duplicated",
                                    source.index
                                ),
                            ));
                        }
                        rows[slot] = campaign_entity_snapshot(&source);
                        rows[slot].authority = MC_AUTHORITY_CAMPAIGN_LIVE;
                        rows[slot].active =
                            u8::from(source.active && source.control_flags & 0x421 == 0);
                    }
                    let live_rows =
                        reset
                            .economy()
                            .fireball_projectiles()
                            .iter()
                            .map(|source| {
                                (
                                    source.source_entity_index,
                                    source.owner_entity_index,
                                    source.active && !source.drawing_disabled,
                                    source.source_class,
                                    source.source_model,
                                    source.action,
                                    source.phase,
                                    source.position,
                                    source.yaw,
                                    source.pitch,
                                    source.fov,
                                    source.actual_speed,
                                )
                            })
                            .chain(
                                reset
                                    .economy()
                                    .authored_class5_archer_children()
                                    .iter()
                                    .map(|source| {
                                        (
                                            source.source_entity_index,
                                            source.entity_id,
                                            source.active
                                                && source.drawing_enabled
                                                && source.control_flags & 0x421 == 0,
                                            source.source_class,
                                            source.model,
                                            source.action,
                                            source.source_byte_3e,
                                            source.position,
                                            source.yaw,
                                            source.pitch,
                                            source.fov,
                                            source.actual_speed,
                                        )
                                    }),
                            )
                            .chain(reset.economy().house_residents().iter().map(|source| {
                                (
                                    source.source_entity_index,
                                    source.source_id,
                                    // Hidden residents retain their arena identity
                                    // until the following source boundary reclaims
                                    // them, just like other live class-5 actors.
                                    source.active
                                        && source.drawing_enabled
                                        && source.control_flags & 0x21 == 0,
                                    5,
                                    source.model,
                                    source.action,
                                    source.phase,
                                    source.position,
                                    source.yaw,
                                    0,
                                    0,
                                    source.actual_speed,
                                )
                            }))
                            .chain(
                                reset
                                    .ordinary()
                                    .houses()
                                    .iter()
                                    .filter(|source| source.active)
                                    .map(|source| {
                                        (
                                            source.entity_index,
                                            source.source_id,
                                            // Inactive HouseFamily entries may outlive
                                            // their arena slot; only current owners
                                            // can shadow the raw entity projection.
                                            source.control_flags & 0x421 == 0,
                                            10,
                                            45,
                                            source.action,
                                            source.phase,
                                            source.position,
                                            0,
                                            0,
                                            0,
                                            0,
                                        )
                                    }),
                            )
                            .chain(reset.economy().castle_projectiles().iter().map(|source| {
                                (
                                    source.source_entity_index,
                                    source.owner_entity_index,
                                    source.active
                                        && !source.drawing_disabled
                                        && source.control_flags & (0x0000_0400 | 0x21) == 0,
                                    source.source_class,
                                    source.source_model,
                                    // The source Castle flyer dispatch selector is
                                    // class 9/action 10. Its carrier has no public
                                    // FOV field. Phase comes from the existing
                                    // Rust source scheduler carrier.
                                    10,
                                    reset
                                        .cf10_live_source_phase(source.source_entity_index)
                                        .unwrap_or_default(),
                                    source.position,
                                    source.yaw,
                                    source.pitch,
                                    0,
                                    source.actual_speed,
                                )
                            }))
                            .chain(
                                reset
                                    .economy()
                                    .possession_projectiles()
                                    .iter()
                                    .map(|source| {
                                        (
                                            source.source_entity_index,
                                            source.source_id_entity_index,
                                            source.active,
                                            source.source_class,
                                            source.source_model,
                                            source.action,
                                            source.phase,
                                            source.position,
                                            source.yaw,
                                            source.pitch,
                                            source.fov,
                                            source.actual_speed,
                                        )
                                    }),
                            )
                            .chain(reset.economy().model17_spawned_children().iter().map(
                                |source| {
                                    (
                                        source.source_entity_index,
                                        source.parent_entity_id,
                                        source.active,
                                        source.source_class,
                                        source.model,
                                        source.action,
                                        source.phase,
                                        source.position,
                                        source.yaw,
                                        source.pitch,
                                        0,
                                        0,
                                    )
                                },
                            ))
                            .chain(
                                reset
                                    .economy()
                                    .model59_smoke_children()
                                    .iter()
                                    // Reclaimed smoke records can outlive their
                                    // slot. Only the active carrier may shadow
                                    // the imported row, as in effects::sprite_sources.
                                    .filter(|source| source.active)
                                    .map(|source| {
                                        (
                                            source.source_entity_index,
                                            source.source_entity_index,
                                            source.control_flags & 0x421 == 0,
                                            source.source_class,
                                            source.model,
                                            source.action,
                                            source.phase,
                                            source.position,
                                            source.yaw,
                                            0,
                                            0,
                                            source.actual_speed,
                                        )
                                    }),
                            )
                            .chain(reset.economy().castle().into_iter().map(|source| {
                                (
                                    source.source_entity_index,
                                    source.source_id_entity_index,
                                    source.active
                                        && source.control_flags & (0x0000_0400 | 0x21) == 0,
                                    source.source_class,
                                    source.source_model,
                                    source.action,
                                    source.phase,
                                    source.position,
                                    source.shift_yaw,
                                    source.shift_pitch,
                                    source.shift_fov,
                                    0,
                                )
                            }))
                            .chain(reset.economy().spheres().iter().map(|source| {
                                (
                                    source.source_entity_index,
                                    source.source_entity_index,
                                    source.active
                                        && !source.pending_delete
                                        && source.control_flags & (0x0000_0400 | 0x21) == 0,
                                    10,
                                    39,
                                    41,
                                    source.phase,
                                    source.position,
                                    source.yaw,
                                    0,
                                    0,
                                    // This carrier stores component velocities,
                                    // not a live actSpeed word. Pose is authoritative.
                                    0,
                                )
                            }))
                            .chain(
                                reset
                                    .economy()
                                    .castle_children()
                                    .iter()
                                    .filter(|source| {
                                        (source.source_class, source.model, source.action)
                                            == (3, 3, 9)
                                    })
                                    .map(|source| {
                                        (
                                            source.source_entity_index,
                                            source.id_entity_index,
                                            source.active
                                                && source.control_flags & (0x0000_0400 | 0x21) == 0,
                                            source.source_class,
                                            source.model,
                                            source.action,
                                            source.source_byte_3e,
                                            source.position,
                                            source.yaw.cast_signed(),
                                            0,
                                            0,
                                            source.speed,
                                        )
                                    }),
                            );
                    for (
                        index,
                        source_id,
                        active,
                        class_id,
                        model,
                        action,
                        phase,
                        position,
                        yaw,
                        pitch,
                        fov,
                        actual_speed,
                    ) in live_rows
                    {
                        let slot = usize::from(index);
                        if slot >= rows.len() || !occupied.insert(index) {
                            return Err(AbiError::new(
                                MC_STATUS_VALIDATION_FAILED,
                                format!(
                                    "campaign live source slot {index} is invalid or duplicated"
                                ),
                            ));
                        }
                        rows[slot] = McCampaignEntitySnapshotV2 {
                            abi_version: MC_NATIVE_ABI_VERSION,
                            struct_size: struct_size::<McCampaignEntitySnapshotV2>(),
                            entity_index: index,
                            source_id,
                            next_relation: u16::MAX,
                            parameter_relation: u16::MAX,
                            authority: MC_AUTHORITY_CAMPAIGN_LIVE,
                            active: u8::from(active),
                            class_id,
                            model,
                            action,
                            phase,
                            x: position.x,
                            y: position.y,
                            z: position.z,
                            yaw,
                            pitch,
                            fov,
                            actual_speed,
                            reserved: [0; 4],
                        };
                    }
                    rows
                }
            };
            let start = usize::try_from(start_index).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign entity start exceeds usize")
            })?;
            if start > entities.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign entity start out of bounds",
                ));
            }
            let count = (entities.len() - start).min(usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign entity capacity exceeds usize")
            })?);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign entity buffer is null",
                ));
            }
            for (offset, entity) in entities.drain(start..start + count).enumerate() {
                unsafe { buffer.add(offset).write_unaligned(entity) };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("entity count fits u32"),
            );
            Ok(())
        })
    })
}

/// Copy live sprite selectors derived by Rust from the qualified campaign
/// package. The caller contributes only camera yaw; class/model mapping never
/// crosses the ABI.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_entity_presentations_copy(
    handle: u64,
    generation: u64,
    tick: u64,
    camera_yaw: i16,
    buffer: *mut McCampaignEntityPresentationV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignEntityPresentationV2>(
            element_size,
            "campaign entity presentation element",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign entity presentation requires a descriptor campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign entity presentation generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if tick != reset.campaign_tick() {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign entity presentation tick {tick} does not match {}",
                        reset.campaign_tick()
                    ),
                ));
            }
            let rows = reset
                .campaign_visible_entity_presentations(camera_yaw)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign presentation capacity exceeds usize",
                )
            })?;
            if rows.len() > capacity {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!(
                        "campaign entity presentation needs {} rows, capacity is {capacity}",
                        rows.len()
                    ),
                ));
            }
            if !rows.is_empty() && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign entity presentation buffer is null",
                ));
            }
            for (ordinal, row) in rows.iter().enumerate() {
                let value = McCampaignEntityPresentationV2 {
                    abi_version: MC_NATIVE_ABI_VERSION,
                    struct_size: struct_size::<McCampaignEntityPresentationV2>(),
                    generation,
                    tick,
                    entity_index: row.entity_index,
                    source_id: row.source_id,
                    render_parameter_index: row.render_parameter_index,
                    base_tmap: row.base_tmap,
                    selected_tmap: row.selected_tmap,
                    logical_height: row.logical_height,
                    render_mode: row.render_mode,
                    orientation_sector: row.orientation_sector,
                    mirrored: u8::from(row.mirrored),
                    reserved_0: 0,
                    x: row.position.x,
                    y: row.position.y,
                    z: row.position.z,
                    yaw: row.yaw,
                    reserved: [0; 4],
                };
                unsafe { buffer.add(ordinal).write_unaligned(value) };
            }
            write_output(
                out_written,
                u32::try_from(rows.len()).expect("campaign presentation count fits u32"),
            );
            Ok(())
        })
    })
}

/// Copy the live Level 2 Goat family from the campaign reset session.  The
/// legacy `mc_native_goats_copy` remains Level 1-only; this additive route
/// exposes the authored Level 2 rows after each production campaign step.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_goats_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McCampaignGoatSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McGoatBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McCampaignGoatSnapshotV2>(element_size, "campaign Goat element")?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Goat snapshots require a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Goat generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let goats = reset.goats();
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign Goat start exceeds usize")
            })?;
            if start > goats.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign Goat start is beyond family count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign Goat capacity exceeds usize")
            })?;
            let count = (goats.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign Goat output buffer is null",
                ));
            }
            for (offset, goat) in goats.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(campaign_goat_snapshot(goat));
                }
            }
            Ok(McGoatBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McGoatBatchV2>(),
                generation: session.generation,
                tick: u64::from(reset.goat_outer_tick()),
                live_world_hash: reset.campaign_state_hash(),
                start_ordinal,
                written: u32::try_from(count).expect("Level 2 Goat count fits u32"),
                total_count: u32::try_from(goats.len()).expect("campaign Goat count fits u32"),
                has_more: u8::from(start + count < goats.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

/// Queue one source-authoritative Level 2 Goat hit.  The normal campaign
/// step consumes the pending mailbox and owns the resulting life/death and
/// economy transitions; callers cannot mutate Goat state directly.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_goat_damage_submit(
    handle: u64,
    generation: u64,
    target_entity_index: u16,
    damage: i32,
    attacker_entity_index: u16,
) -> i32 {
    boundary(|| {
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Goat damage requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Goat damage generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            reset
                .submit_goat_damage(target_entity_index, damage, attacker_entity_index)
                .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error.to_string()))
        })
    })
}

/// Copy the typed Level 2 authored spell-pickup arena. The rows are created
/// by the native authored-group route and remain bound to the shared source
/// allocator; clients use the returned positions to feed the normal campaign
/// step rather than synthesizing a pickup or mutating simulation state.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_spell_pickups_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McCampaignSpellPickupSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McCampaignManaBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McCampaignSpellPickupSnapshotV2>(
            element_size,
            "campaign spell pickup element",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign spell pickups require a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign spell pickup generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let pickups = reset.economy().spell_pickups();
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign spell pickup start exceeds usize",
                )
            })?;
            if start > pickups.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign spell pickup start is beyond arena count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign spell pickup capacity exceeds usize",
                )
            })?;
            let count = (pickups.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign spell pickup output buffer is null",
                ));
            }
            for (offset, pickup) in pickups.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(campaign_spell_pickup_snapshot(pickup));
                }
            }
            Ok(McCampaignManaBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignManaBatchV2>(),
                generation: session.generation,
                tick: reset.state.tick,
                live_world_hash: reset.campaign_state_hash(),
                start_ordinal,
                written: u32::try_from(count).expect("Level 2 spell pickup count fits u32"),
                total_count: u32::try_from(pickups.len())
                    .expect("Level 2 spell pickup count fits u32"),
                has_more: u8::from(start + count < pickups.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

/// Copy the live Level 2 mana-sphere arena.  This is deliberately separate
/// from the legacy Level 1 mission endpoint: the rows come from the mutable
/// `Level2EconomyState` arena after the source `sub_5FD00`/`TransformEntityToManaSphere`
/// paths, including retained inactive rows awaiting the reclaim pass.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_mana_spheres_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McManaSphereSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McCampaignManaBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McManaSphereSnapshotV2>(
            element_size,
            "campaign mana sphere element",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign mana spheres require a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign mana sphere generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let spheres = reset.economy().spheres();
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign mana sphere start exceeds usize")
            })?;
            if start > spheres.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana sphere start is beyond arena count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana sphere capacity exceeds usize",
                )
            })?;
            let count = (spheres.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign mana sphere output buffer is null",
                ));
            }
            for (offset, sphere) in spheres.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(mana_sphere_snapshot(sphere));
                }
            }
            Ok(McCampaignManaBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignManaBatchV2>(),
                generation: session.generation,
                tick: reset.state.tick,
                live_world_hash: reset.campaign_state_hash(),
                start_ordinal,
                written: u32::try_from(count).expect("Level 2 mana sphere count fits u32"),
                total_count: u32::try_from(spheres.len())
                    .expect("Level 2 mana sphere count fits u32"),
                has_more: u8::from(start + count < spheres.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

/// Copy every retained Level 2 mana-carrier row in source allocation order.
/// Do not collapse this to `economy().carrier()`: remc2 can retain multiple
/// active/inactive carriers in the shared arena until the reclaim pass.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_mana_carriers_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McManaCarrierSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McCampaignManaBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McManaCarrierSnapshotV2>(
            element_size,
            "campaign mana carrier element",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign mana carriers require a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign mana carrier generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let carriers = reset.economy().carriers();
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana carrier start exceeds usize",
                )
            })?;
            if start > carriers.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana carrier start is beyond arena count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana carrier capacity exceeds usize",
                )
            })?;
            let count = (carriers.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign mana carrier output buffer is null",
                ));
            }
            for (offset, carrier) in carriers.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(mana_carrier_snapshot_value(
                            carrier,
                            session.generation,
                            reset.state.tick,
                        ));
                }
            }
            Ok(McCampaignManaBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignManaBatchV2>(),
                generation: session.generation,
                tick: reset.state.tick,
                live_world_hash: reset.campaign_state_hash(),
                start_ordinal,
                written: u32::try_from(count).expect("Level 2 mana carrier count fits u32"),
                total_count: u32::try_from(carriers.len())
                    .expect("Level 2 mana carrier count fits u32"),
                has_more: u8::from(start + count < carriers.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_destroy(handle: u64) -> i32 {
    boundary(|| with_registry_mut(|registry| registry.remove_legacy(handle)))
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_reset(
    handle: u64,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session_mut(handle, CampaignFacade::<SealedLevel1Adapter>::reset)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_snapshot(
    handle: u64,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(session.snapshot()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_source_projection_hash(handle: u64, out_hash: *mut u64) -> i32 {
    boundary_value(out_hash, struct_size::<u64>(), || {
        with_session(handle, |session| Ok(source_projection_hash(&session.state)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_source_arena_hash(handle: u64, out_hash: *mut u64) -> i32 {
    boundary_value(out_hash, struct_size::<u64>(), || {
        with_session(handle, |session| Ok(session.mission.source_arena_hash_v1()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_action_51_fallback_state_hash_v1(
    handle: u64,
    out_hash: *mut u64,
) -> i32 {
    boundary_value(out_hash, struct_size::<u64>(), || {
        with_session(handle, |session| {
            Ok(session.mission.action_51_fallback_state_hash_v1())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_resident_observation_v1(
    handle: u64,
    out_observation: *mut McLevel1ResidentObservationV1,
    out_observation_size: u32,
) -> i32 {
    boundary_value(out_observation, out_observation_size, || {
        with_session(handle, |session| {
            Ok(McLevel1ResidentObservationV1 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1ResidentObservationV1>(),
                required_mask: session.mission.required_resident_behavior_mask(),
                observed_mask: session.mission.observed_resident_behavior_mask(),
                remaining_mask: session.mission.remaining_resident_behavior_mask(),
                reserved_0: 0,
                digest: session.mission.resident_behavior_observation_digest_v1(),
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_presentation_state(
    handle: u64,
    out_state: *mut McPresentationStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(session.presentation_state()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_render_camera_state(
    handle: u64,
    out_state: *mut McRenderCameraStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(session.render_camera_state()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_pnt5_vector_count() -> u32 {
    TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_pnt5_vector(
    index: u32,
    out_vector: *mut McTerrainPnt5VectorV2,
    out_vector_size: u32,
) -> i32 {
    boundary_value(out_vector, out_vector_size, || {
        let (turn, tile_x, tile_y, shade, angle, view_y, distance_squared) = match index {
            0 => (0, 4, 4, 0, 8, 128, 0),
            1 => (0, 12, 4, 40, 8, 128, 0),
            2 => (1, 0, 0, 40, 8, 128, 0),
            3 => (1, 1, 1, 40, 8, 128, 14_745_600),
            4 => (1, 2, 3, 40, 8, 128, 14_745_601),
            5 => (1, 5, 7, 40, 8, 128, 19_202_048),
            6 => (1, 255, 255, 40, 8, 128, 23_658_495),
            7 => (1, 255, 255, 40, 8, 128, 23_658_496),
            8 => (1, 255, 255, 40, 8, 128, 26_214_399),
            9 => (1, 255, 255, 40, 8, 128, 26_214_400),
            10 => (1, 42, 99, 56, 8, 128, 0),
            11 => (65_535, 256, -1, 55, 8, 128, 14_757_945),
            12 => (1, 0, 0, 40, 8, -256, 0),
            _ => {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("terrain pnt5 qualification vector {index} is out of bounds"),
                ));
            }
        };
        let value =
            outdoor_terrain_pnt5(turn, tile_x, tile_y, shade, angle, view_y, distance_squared);
        Ok(McTerrainPnt5VectorV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McTerrainPnt5VectorV2>(),
            turn,
            tile_x,
            tile_y,
            distance_squared,
            shade,
            angle,
            visible: u8::from(value.visible),
            tablesn_row: value.tablesn_row,
            wave: value.wave,
            land_displacement: value.land_displacement,
            pnt5: value.pnt5,
            view_y,
            lighting_wave: value.lighting_wave,
            reserved: [0; 2],
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_grid_vertex_count() -> u32 {
    u32::try_from(OUTDOOR_GRID_VERTICES).expect("terrain grid vertex count fits u32")
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_terrain_grid_copy(
    handle: u64,
    buffer: *mut McTerrainGridVertexV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McTerrainGridVertexV2>(element_size, "terrain grid vertex")?;
        with_session(handle, |session| {
            let grid = session.terrain_grid();
            copy_slice(&grid, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_step(
    handle: u64,
    command: *const McCommandV2,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        let command = read_versioned_input(command, "command")?;
        validate_command(&command)?;
        let command = player_command(command)?;
        with_session_mut(handle, |session| {
            CampaignFacade::<SealedLevel1Adapter>::step(session, command)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_step_with_camera(
    handle: u64,
    command: *const McCommandV2,
    camera_input: *const McCameraInputV2,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        let command = read_versioned_input(command, "command")?;
        validate_command(&command)?;
        let camera_input = read_versioned_input(camera_input, "camera input")?;
        validate_camera_input(&camera_input)?;
        let mut command = player_command(command)?;
        command.argument_6 = camera_input.look_yaw;
        command.argument_8 = camera_input.look_pitch;
        with_session_mut(handle, |session| {
            session.step(command);
            Ok(session.snapshot())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_terrain_frame_copy(
    handle: u64,
    query: *const McTerrainFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    coverage_buffer: *mut u8,
    coverage_capacity: u32,
    out_info: *mut McTerrainFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "terrain frame query")?;
        validate_reserved(&query.reserved, "terrain frame query")?;
        with_session(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "terrain frame generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.tick != session.camera_history.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "terrain frame presentation tick {} does not match {}",
                        query.tick, session.camera_history.tick
                    ),
                ));
            }
            let required =
                u32::try_from(LEVEL1_VIEW_PIXELS).expect("qualified terrain frame extent fits u32");
            if index_capacity < required || coverage_capacity < required {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    format!(
                        "terrain frame requires {required} index and coverage bytes, got {index_capacity} and {coverage_capacity}"
                    ),
                ));
            }
            if index_buffer.is_null() || coverage_buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "terrain frame index and coverage buffers must be non-null",
                ));
            }
            let frame = session.terrain_frame()?;
            // SAFETY: both caller buffers were checked non-null and have at
            // least the exact 64,000-byte frame capacity.
            unsafe {
                index_buffer.copy_from_nonoverlapping(frame.indices.as_ptr(), LEVEL1_VIEW_PIXELS);
                coverage_buffer
                    .copy_from_nonoverlapping(frame.terrain_coverage.as_ptr(), LEVEL1_VIEW_PIXELS);
            }
            Ok(McTerrainFrameInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McTerrainFrameInfoV2>(),
                generation: session.generation,
                tick: session.camera_history.tick,
                width: LEVEL1_VIEW_WIDTH as u16,
                height: LEVEL1_VIEW_HEIGHT as u16,
                stride: LEVEL1_VIEW_WIDTH as u16,
                viewport_x: 0,
                viewport_y: 0,
                viewport_width: LEVEL1_VIEW_WIDTH as u16,
                viewport_height: LEVEL1_VIEW_HEIGHT as u16,
                reserved_0: 0,
                index_bytes: required,
                coverage_bytes: required,
                flags: MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN
                    | MC_TERRAIN_FRAME_SKY_SEEDED
                    | MC_TERRAIN_FRAME_HAS_COVERAGE
                    | MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED,
                reflected_triangles: frame.stats.reflected_triangles,
                primary_mode5_triangles: frame.stats.primary_mode5_triangles,
                primary_mode26_triangles: frame.stats.primary_mode26_triangles,
                index_hash: frame.index_hash,
                coverage_hash: frame.coverage_hash,
                terrain_pixel_writes: frame.stats.terrain_pixel_writes,
                reserved: [0; 2],
            })
        })
    })
}

/// Copy the bounded source-ordered Level 1 world frame.
///
/// This preserves the stable terrain-only endpoint while adding the currently
/// qualified mode-20 Goat family. Other non-terrain writers remain explicitly
/// omitted in the returned flags.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_world_frame_copy(
    handle: u64,
    query: *const McTerrainFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    coverage_buffer: *mut u8,
    coverage_capacity: u32,
    out_info: *mut McTerrainFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "world frame query")?;
        validate_reserved(&query.reserved, "world frame query")?;
        with_session_mut(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "world frame generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.tick != session.camera_history.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "world frame presentation tick {} does not match {}",
                        query.tick, session.camera_history.tick
                    ),
                ));
            }
            let required =
                u32::try_from(LEVEL1_VIEW_PIXELS).expect("qualified world frame extent fits u32");
            if index_capacity < required || coverage_capacity < required {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    format!(
                        "world frame requires {required} index and coverage bytes, got {index_capacity} and {coverage_capacity}"
                    ),
                ));
            }
            if index_buffer.is_null() || coverage_buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "world frame index and coverage buffers must be non-null",
                ));
            }
            let frame = session.world_frame()?;
            // SAFETY: both caller buffers were checked non-null and have at
            // least the exact 64,000-byte frame capacity.
            unsafe {
                index_buffer.copy_from_nonoverlapping(frame.indices.as_ptr(), LEVEL1_VIEW_PIXELS);
                coverage_buffer
                    .copy_from_nonoverlapping(frame.terrain_coverage.as_ptr(), LEVEL1_VIEW_PIXELS);
            }
            Ok(McTerrainFrameInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McTerrainFrameInfoV2>(),
                generation: session.generation,
                tick: session.camera_history.tick,
                width: LEVEL1_VIEW_WIDTH as u16,
                height: LEVEL1_VIEW_HEIGHT as u16,
                stride: LEVEL1_VIEW_WIDTH as u16,
                viewport_x: 0,
                viewport_y: 0,
                viewport_width: LEVEL1_VIEW_WIDTH as u16,
                viewport_height: LEVEL1_VIEW_HEIGHT as u16,
                reserved_0: 0,
                index_bytes: required,
                coverage_bytes: required,
                flags: world_frame_flags(
                    frame.reflected_sprites_composited,
                    frame.sprite_shadows_composited,
                ),
                reflected_triangles: frame.stats.reflected_triangles,
                primary_mode5_triangles: frame.stats.primary_mode5_triangles,
                primary_mode26_triangles: frame.stats.primary_mode26_triangles,
                index_hash: frame.index_hash,
                coverage_hash: frame.coverage_hash,
                terrain_pixel_writes: frame.stats.terrain_pixel_writes,
                reserved: [0; 2],
            })
        })
    })
}

/// Copy the revisioned compositor-complete fixed-profile FINAL MAIN.
///
/// Unlike the stable 96-byte terrain/world endpoints, this additive endpoint
/// owns temporal MAIN history, indexed overlays, and per-byte provenance.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_presentation_frame_copy(
    handle: u64,
    query: *const McPresentationFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    provenance_buffer: *mut u8,
    provenance_capacity: u32,
    out_info: *mut McPresentationFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "presentation frame query")?;
        validate_reserved(&query.reserved, "presentation frame query")?;
        if query.reserved_0 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "presentation frame query reserved_0 must be zero",
            ));
        }
        for (name, value) in [
            ("blur_capability", query.blur_capability),
            ("high_speed_system", query.high_speed_system),
            ("scratch_present", query.scratch_present),
            ("spatial_smoothing", query.spatial_smoothing),
            ("minimap_enabled", query.minimap_enabled),
            ("top_bar_enabled", query.top_bar_enabled),
            ("game_paused", query.game_paused),
            ("level_end", query.level_end),
            ("objective_textbox", query.objective_textbox),
            ("tutorial_or_briefing", query.tutorial_or_briefing),
            ("help_popup", query.help_popup),
        ] {
            if value > 1 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("presentation frame {name} must be 0 or 1"),
                ));
            }
        }
        if query.debug_text & !0x48 != 0 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "presentation frame debug_text accepts source flags 0x08 and 0x40 only",
            ));
        }
        if query.configured_blur_mode > 2
            || query.viewport_width > LEVEL1_VIEW_WIDTH as u16
            || query.viewport_height > LEVEL1_VIEW_HEIGHT as u16
            || query.screen_stride < query.viewport_width
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "presentation blur mode or viewport/stride is invalid",
            ));
        }
        let required =
            u32::try_from(LEVEL1_VIEW_PIXELS).expect("qualified presentation extent fits u32");
        if index_capacity < required || provenance_capacity < required {
            return Err(AbiError::new(
                MC_STATUS_BUFFER_TOO_SMALL,
                format!(
                    "presentation frame requires {required} index and provenance bytes, got {index_capacity} and {provenance_capacity}"
                ),
            ));
        }
        if index_buffer.is_null() || provenance_buffer.is_null() {
            return Err(AbiError::new(
                MC_STATUS_NULL_POINTER,
                "presentation index and provenance buffers must be non-null",
            ));
        }
        with_projection_session_mut(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "presentation generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.tick != session.camera_history.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "presentation tick {} does not match camera history {}",
                        query.tick, session.camera_history.tick
                    ),
                ));
            }
            if (query.game_paused != 0) != session.game_paused {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "presentation paused query does not match authoritative session state",
                ));
            }
            let result = session.presentation_frame(query)?;
            let frame = &result.frame;
            // SAFETY: both caller buffers were checked non-null with the
            // exact fixed-profile capacity.
            unsafe {
                index_buffer.copy_from_nonoverlapping(frame.indices.as_ptr(), LEVEL1_VIEW_PIXELS);
                provenance_buffer
                    .copy_from_nonoverlapping(frame.provenance.as_ptr(), LEVEL1_VIEW_PIXELS);
            }
            let effective_postprocess_mode = match frame.postprocess_branch {
                IndexedPostprocessBranch::Direct { .. } => 0,
                IndexedPostprocessBranch::Temporal { mode } => mode,
            };
            let source = frame.source_receipt;
            let family = source.family_mask;
            let source_flags = if source.sky_seeded {
                MC_TERRAIN_FRAME_SKY_SEEDED
            } else {
                0
            } | if source.has_coverage {
                MC_TERRAIN_FRAME_HAS_COVERAGE
            } else {
                0
            } | if family & 1 != 0 {
                MC_TERRAIN_FRAME_GOATS_COMPOSITED
            } else {
                0
            } | if family & (1 << 1) != 0 {
                MC_TERRAIN_FRAME_FROZEN_ACTORS_COMPOSITED
            } else {
                0
            } | if family & (1 << 3) != 0 {
                MC_TERRAIN_FRAME_ARCHERS_COMPOSITED
            } else {
                0
            } | if family & (1 << 4) != 0 {
                MC_TERRAIN_FRAME_FIREFLIES_COMPOSITED
            } else {
                0
            } | if family & (1 << 5) != 0 {
                MC_TERRAIN_FRAME_COMBAT_COMPOSITED
            } else {
                0
            } | if family & (1 << 6) != 0 {
                MC_TERRAIN_FRAME_OBJECTIVE_PROPS_COMPOSITED
            } else {
                0
            } | if family & (1 << 7) != 0 {
                MC_TERRAIN_FRAME_ECONOMY_COMPOSITED
            } else {
                0
            } | if family & (1 << 8) != 0 {
                MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED
            } else {
                0
            } | if source.reflected_sprites_composited {
                MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED
            } else {
                0
            };
            let presentation_flags = if frame.has_provenance() {
                MC_PRESENTATION_FRAME_HAS_PROVENANCE
            } else {
                0
            } | if frame.compositor_complete() {
                MC_PRESENTATION_FRAME_COMPOSITOR_COMPLETE
            } else {
                0
            } | if frame.overlays_complete() {
                MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED
            } else {
                0
            } | if frame.postprocess_complete() {
                MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE
            } else {
                0
            } | if frame.lighting_complete() {
                MC_PRESENTATION_FRAME_LIGHTING_COMPLETE
            } else {
                0
            };
            Ok(McPresentationFrameInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McPresentationFrameInfoV2>(),
                generation: frame.generation,
                tick: frame.tick,
                presentation_revision: frame.presentation_revision,
                immutable_input_hash: frame.immutable_input_hash,
                width: LEVEL1_VIEW_WIDTH as u16,
                height: LEVEL1_VIEW_HEIGHT as u16,
                stride: LEVEL1_VIEW_WIDTH as u16,
                viewport_width: query.viewport_width,
                index_bytes: required,
                provenance_bytes: required,
                flags: source_flags | presentation_flags,
                effective_postprocess_mode,
                history_state: match session.presentation_frame.history() {
                    MainHistory::Unprimed => 0,
                    MainHistory::Primed => 1,
                },
                newly_committed: u8::from(result.newly_committed),
                reserved_0: 0,
                index_hash: frame.index_hash,
                provenance_hash: frame.provenance_hash,
                lighting_hash: session.mission.dynamic_lighting_hash_v1(),
                source_arena_hash: session.mission.source_arena_hash_v2(),
                overlay_pixels_written: frame.overlay_pixels_written,
                overlay_stage_count: u16::try_from(frame.overlay_receipts.len())
                    .expect("fixed overlay stage count fits u16"),
                reserved_1: 0,
                reserved: [0; 6],
            })
        })
    })
}

/// Copy one owner-bound, profile-sized Classic HD frame.
///
/// This additive endpoint never calls the fixed 320x200 presentation
/// compositor. The Rust Classic HD graph rerenders terrain and supported
/// typed sprites into its own profile-sized MAIN/history planes, then copies
/// those planes to the caller. Unsupported source families fail closed.
pub(super) fn validate_classic_hd_frame_query(query: &McPresentationFrameQueryV2) -> AbiResult<()> {
    validate_reserved(&query.reserved, "Classic HD frame query")?;
    if query.reserved_0 != 0 {
        return Err(AbiError::new(
            MC_STATUS_RESERVED_NONZERO,
            "Classic HD frame query reserved_0 must be zero",
        ));
    }
    for (name, value) in [
        ("blur_capability", query.blur_capability),
        ("high_speed_system", query.high_speed_system),
        ("scratch_present", query.scratch_present),
        ("spatial_smoothing", query.spatial_smoothing),
        ("minimap_enabled", query.minimap_enabled),
        ("top_bar_enabled", query.top_bar_enabled),
        ("game_paused", query.game_paused),
        ("level_end", query.level_end),
        ("objective_textbox", query.objective_textbox),
        ("tutorial_or_briefing", query.tutorial_or_briefing),
        ("help_popup", query.help_popup),
    ] {
        if value > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!("Classic HD frame {name} must be 0 or 1"),
            ));
        }
    }
    if query.debug_text & !0x48 != 0 {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "Classic HD frame debug_text accepts source flags 0x08 and 0x40 only",
        ));
    }
    if query.configured_blur_mode > 2 {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "Classic HD configured blur mode must be zero, one, or two",
        ));
    }
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_classic_hd_frame_copy(
    handle: u64,
    profile_id: u32,
    query: *const McPresentationFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    provenance_buffer: *mut u8,
    provenance_capacity: u32,
    out_info: *mut McPresentationFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "Classic HD frame query")?;
        validate_classic_hd_frame_query(&query)?;
        let profile = classic_hd_profile_for_id(profile_id)?;
        let required = profile.extent();
        let required_u32 = u32::try_from(required).expect("Classic HD extent fits u32");
        if index_capacity < required_u32 || provenance_capacity < required_u32 {
            return Err(AbiError::new(
                MC_STATUS_BUFFER_TOO_SMALL,
                format!(
                    "Classic HD profile {} requires {required_u32} index and provenance bytes, got {index_capacity} and {provenance_capacity}",
                    profile.id.as_str()
                ),
            ));
        }
        if index_buffer.is_null() || provenance_buffer.is_null() {
            return Err(AbiError::new(
                MC_STATUS_NULL_POINTER,
                "Classic HD index and provenance buffers must be non-null",
            ));
        }
        // SAFETY: the capacities were checked against the validated profile
        // extent and the pointers were checked non-null above.
        let indices = unsafe { slice::from_raw_parts_mut(index_buffer, required) };
        let provenance = unsafe { slice::from_raw_parts_mut(provenance_buffer, required) };
        with_session_mut(handle, |session| {
            session.classic_hd_frame_copy(profile_id, query, indices, provenance)
        })
    })
}

/// Copy only the profile-sized owner HUD overlay for Unity Enhanced.
///
/// This endpoint shares the exact owner compositor and profile scaling with
/// Classic HD, but deliberately does not render the Classic HD world graph.
/// The destination is transparent except for provenance-marked HUD pixels;
/// Unity's Enhanced world remains the only world image on that route.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_classic_hd_owner_overlay_copy(
    handle: u64,
    profile_id: u32,
    query: *const McPresentationFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    provenance_buffer: *mut u8,
    provenance_capacity: u32,
    out_info: *mut McPresentationFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "Classic HD owner overlay query")?;
        validate_classic_hd_frame_query(&query)?;
        let profile = classic_hd_profile_for_id(profile_id)?;
        let required = profile.extent();
        let required_u32 = u32::try_from(required).expect("Classic HD extent fits u32");
        if index_capacity < required_u32 || provenance_capacity < required_u32 {
            return Err(AbiError::new(
                MC_STATUS_BUFFER_TOO_SMALL,
                format!(
                    "Classic HD profile {} requires {required_u32} owner overlay bytes per plane, got {index_capacity} and {provenance_capacity}",
                    profile.id.as_str()
                ),
            ));
        }
        if index_buffer.is_null() || provenance_buffer.is_null() {
            return Err(AbiError::new(
                MC_STATUS_NULL_POINTER,
                "Classic HD owner overlay index and provenance buffers must be non-null",
            ));
        }
        // SAFETY: the capacities were checked against the validated profile
        // extent and the pointers were checked non-null above.
        let indices = unsafe { slice::from_raw_parts_mut(index_buffer, required) };
        let provenance = unsafe { slice::from_raw_parts_mut(provenance_buffer, required) };
        with_session_mut(handle, |session| {
            session.classic_hd_owner_overlay_copy(profile_id, query, indices, provenance)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_world_info(
    handle: u64,
    out_info: *mut McWorldInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_session(handle, |session| {
            let world = &session.world;
            let metadata = world.metadata();
            Ok(McWorldInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McWorldInfoV2>(),
                external_level: metadata.external_level,
                internal_level: metadata.internal_level,
                map_type: metadata.map_type.id(),
                profile: profile_code(world.profile().cave_mode),
                reserved_0: 0,
                entity_count: u32::try_from(world.entities().len())
                    .expect("validated entity count fits u32"),
                active_entity_count: u32::try_from(
                    world
                        .entities()
                        .iter()
                        .filter(|entity| entity.active)
                        .count(),
                )
                .expect("validated active count fits u32"),
                player_count: u32::try_from(world.players().len())
                    .expect("validated player count fits u32"),
                asset_ref_count: u32::try_from(world.asset_refs().len())
                    .expect("validated asset count fits u32"),
                terrain_width: 256,
                terrain_height: 256,
                reset_state_hash: state_hash(world.reset_state()),
                reset_conformance_hash: conformance_hash(
                    world.reset_state(),
                    world.reset_context(),
                ),
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_world_string_length(
    handle: u64,
    kind: u32,
    out_length: *mut u32,
) -> i32 {
    boundary_value(out_length, struct_size::<u32>(), || {
        with_session(handle, |session| {
            let value = world_string(&session.world, kind)?;
            string_length(value)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_world_string_copy(
    handle: u64,
    kind: u32,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_length, "out_length")?;
        with_session(handle, |session| {
            let value = world_string(&session.world, kind)?;
            copy_string(value, buffer, capacity, out_length)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_plane_length(
    handle: u64,
    kind: u32,
    out_count: *mut u32,
) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            validate_plane_kind(session.terrain.planes(), kind)?;
            Ok(u32::try_from(TERRAIN_CELLS).expect("terrain count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_terrain_state(
    handle: u64,
    out_state: *mut McTerrainStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(terrain_state(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_plane_copy_u8(
    handle: u64,
    kind: u32,
    buffer: *mut u8,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        with_session(handle, |session| {
            let values = terrain_u8_plane(session.terrain.planes(), kind)?;
            copy_slice(values, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_entity_indices_copy(
    handle: u64,
    buffer: *mut i16,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        with_session(handle, |session| {
            let values = &session.terrain.planes().entity_indices;
            copy_slice(values, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_entity_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(u32::try_from(session.world.entities().len())
                .expect("validated entity count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_objective_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(u32::try_from(session.world.runtime().objectives.len())
                .expect("validated objective count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_objectives_copy(
    handle: u64,
    start_slot: u32,
    buffer: *mut McObjectiveSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McObjectiveSnapshotV2>(element_size, "objective element")?;
        with_session(handle, |session| {
            let objectives = &session.world.runtime().objectives;
            let start = usize::try_from(start_slot)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "objective start exceeds usize"))?;
            if start > objectives.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "objective start is beyond objective count",
                ));
            }
            let remaining = &objectives[start..];
            let count = remaining.len().min(
                usize::try_from(capacity)
                    .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "capacity exceeds usize"))?,
            );
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "objective output buffer is null",
                ));
            }
            for (offset, objective) in remaining.iter().take(count).enumerate() {
                let slot = start + offset;
                let snapshot = objective_snapshot(
                    u8::try_from(slot).expect("validated objective slot fits u8"),
                    objective,
                );
                // SAFETY: buffer is non-null for count records and the caller
                // promises capacity writable records of the exact ABI stride.
                unsafe { buffer.add(offset).write_unaligned(snapshot) };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("objective count fits u32"),
            );
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_entities_copy(
    handle: u64,
    start_index: u32,
    buffer: *mut McEntitySnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McEntitySnapshotV2>(element_size, "entity element")?;
        with_session(handle, |session| {
            let entities = session.world.entities();
            let start = usize::try_from(start_index)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "entity start does not fit usize"))?;
            if start > entities.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "entity start is beyond entity count",
                ));
            }
            let remaining = &entities[start..];
            let count = remaining.len().min(
                usize::try_from(capacity)
                    .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "capacity does not fit usize"))?,
            );
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "entity output buffer is null",
                ));
            }
            for (index, entity) in remaining.iter().take(count).enumerate() {
                // SAFETY: buffer is non-null for count records and the caller
                // promises capacity writable records.
                unsafe {
                    buffer.add(index).write_unaligned(entity_snapshot(
                        entity,
                        &session.state,
                        &session.live_world,
                        &session.mission,
                    ));
                };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("entity count fits u32"),
            );
            Ok::<(), AbiError>(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_goats_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McGoatSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McGoatBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McGoatSnapshotV2>(element_size, "Goat element")?;
        with_session(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let goats = session.live_world.goats();
            let start = usize::try_from(start_ordinal)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "Goat start exceeds usize"))?;
            if start > goats.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "Goat start is beyond family count",
                ));
            }
            let capacity = usize::try_from(capacity)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "Goat capacity exceeds usize"))?;
            let count = (goats.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "Goat output buffer is null",
                ));
            }
            for (offset, goat) in goats.iter().skip(start).take(count).enumerate() {
                // SAFETY: the caller supplied exact element stride and
                // writable capacity for `count` records.
                unsafe {
                    buffer.add(offset).write_unaligned(goat_snapshot(goat));
                }
            }
            Ok(McGoatBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McGoatBatchV2>(),
                generation: session.generation,
                tick: session.live_world.outer_tick().into(),
                live_world_hash: session.live_world_hash_v2(),
                start_ordinal,
                written: u32::try_from(count).expect("33 Goats fit u32"),
                total_count: u32::try_from(LEVEL_ONE_GOAT_INDICES.len()).expect("33 Goats fit u32"),
                has_more: u8::from(start + count < goats.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

#[cfg(test)]
mod castle_worldfeed_tests {
    use super::super::{game_runtime_abi::*, game_session_abi::*};
    use super::*;

    fn snapshot(handle: u64) -> serde_json::Value {
        let mut bytes = vec![0_u8; 1024 * 1024];
        let mut written = 0;
        assert_eq!(
            mc_game_session_snapshot_json_v1(
                handle,
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                &raw mut written,
            ),
            MC_STATUS_OK,
        );
        serde_json::from_slice(&bytes[..written as usize]).unwrap()
    }

    fn public_step(handle: u64, command: &[u8]) {
        let before = snapshot(handle);
        let identity = &before["identity"];
        assert_eq!(
            mc_game_session_step_json_v1(
                handle,
                identity["generation"].as_str().unwrap().parse().unwrap(),
                identity["tick"].as_str().unwrap().parse().unwrap(),
                command.as_ptr(),
                command.len() as u32,
            ),
            MC_STATUS_OK,
        );
    }

    fn world_rows(handle: u64) -> Vec<McCampaignEntitySnapshotV2> {
        let before = snapshot(handle);
        let mut rows = vec![McCampaignEntitySnapshotV2::default(); 1000];
        let mut written = 0;
        assert_eq!(
            mc_native_campaign_entities_copy(
                handle,
                0,
                rows.as_mut_ptr(),
                rows.len() as u32,
                struct_size::<McCampaignEntitySnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK,
        );
        assert_eq!(written, 1000);
        assert_eq!(snapshot(handle), before, "world polling is observational");
        rows
    }

    #[test]
    fn real_level_two_public_castle_flyer_worldfeed_hides_after_landing_and_reset() {
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let replay =
            fs::read(generated.join("opened-l2-block32-castle-boot-f1fd3b5-20261005.json"))
                .expect("fresh exact owner Level 2 entry replay");
        let packages = serde_json::to_vec(&serde_json::json!({
            "request_contract": "carpet-flight-installed-package-set-bootstrap/v1",
            "packages": [
                {"package_dir": generated.join("mc-cf15-level1-package-v3-current")},
                {"package_dir": generated.join("level2-day-block32-campaign-v2")},
                {"package_dir": generated.join("level3-day-block32-campaign-recipe-v2")}
            ]
        }))
        .unwrap();
        let mut handle = 0;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        assert_eq!(
            mc_game_runtime_load_replay_json_v1(
                handle,
                1,
                packages.as_ptr(),
                packages.len() as u32,
                replay.as_ptr(),
                replay.len() as u32,
            ),
            MC_STATUS_OK,
        );
        for attempt in 0..2 {
            let rows = world_rows(handle);
            assert!(
                !rows
                    .iter()
                    .any(|row| row.active != 0 && row.class_id == 9 && row.model == 10)
            );
            let player = rows
                .iter()
                .find(|row| row.active != 0 && row.class_id == 3 && row.model == 0)
                .expect("real local player")
                .entity_index;
            public_step(handle, br#"{"select_spell":"spell.castle"}"#);
            public_step(handle, br#"{"cast_selected":true}"#);
            let rows = world_rows(handle);
            let flyer = rows
                .iter()
                .find(|row| row.active != 0 && row.class_id == 9 && row.model == 10)
                .expect("the real public Castle birth is visible before landing");
            assert_eq!(flyer.authority, MC_AUTHORITY_CAMPAIGN_LIVE);
            assert_eq!(flyer.source_id, player);
            assert_eq!(flyer.action, 10);
            assert_ne!(
                flyer.phase, 0,
                "the real source phase is published, not a placeholder"
            );
            assert!(flyer.actual_speed > 0);
            let flyer_slot = flyer.entity_index;
            public_step(handle, b"{}");
            assert_eq!(snapshot(handle)["value"]["player_vitals"]["mana"], 0);
            let mut reached_root = false;
            for _ in 0..64 {
                let rows = world_rows(handle);
                if rows.iter().any(|row| {
                    row.active != 0 && row.class_id == 3 && row.model == 2 && row.action == 4
                }) {
                    assert!(
                        !rows.iter().any(|row| row.active != 0
                            && row.entity_index == flyer_slot
                            && row.class_id == 9
                            && row.model == 10),
                        "landed flyer is hidden/reclaimed"
                    );
                    reached_root = true;
                    break;
                }
                public_step(handle, b"{}");
            }
            assert!(
                reached_root,
                "public Castle construction remains within 64 turns"
            );
            if attempt == 0 {
                let before = snapshot(handle);
                let identity = &before["identity"];
                assert_eq!(
                    mc_game_session_reset_v1(
                        handle,
                        identity["generation"].as_str().unwrap().parse().unwrap(),
                        identity["tick"].as_str().unwrap().parse().unwrap(),
                    ),
                    MC_STATUS_OK
                );
            }
        }
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
    }
}
