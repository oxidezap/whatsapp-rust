//! Portable, durable registration driver. No HTTP operation is retried implicitly.
//!
//! The host supplies an artifact-qualified profile and registration material. Its
//! bounded snapshot and fingerprint are persisted alongside the keys, so restore
//! never regenerates installation IDs. Profile/key drift is rejected. Cancellation
//! retains the exchange inside this driver.
//! Challenge state is checkpointed; images/audio remain caller-owned. Hosts must
//! retain their UI payload securely when solving a challenge across a restart.
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use futures::future::{Either, select};
use wacore::mobile::{
    AttemptToken, MobileEnvironment, RegistrationCheckpoint, RegistrationError,
    RegistrationIdentity, RegistrationOperation, RegistrationOutcome, RegistrationPolicy,
    RegistrationRunId, RegistrationStage, RegistrationWorkflow,
    registration_wire::{
        AttestationPolicy, BodyAttestation, CodeRequestPlan, EphemeralRegistrationKey,
        RegistrationDeliveryHints, RegistrationEndpointPath, RegistrationForm,
        RegistrationMaterial, RegistrationMaterialPersistence, RegistrationRequest,
        RegistrationRequestBuilder, RegistrationResponse, RegistrationResponseDecision,
        RegistrationWireError, VerificationMethod,
    },
};
use wacore::net::{HttpClient, HttpRequest, HttpResponse};
use wacore::runtime::{BoxFuture, Runtime};
use wacore::store::{MobileDeviceKeys, MobileSessionBinding, StoredMobilePlatform};
use wacore::time::Clock;
use wacore_binary::{Jid, Server};
use zeroize::{Zeroize, Zeroizing};

use crate::store::{Backend, error::StoreError, persistence_manager::PersistenceManager};

/// The host signs the exact encrypted body using its legitimate platform port.
/// No default attestation provider or successful empty fallback is supplied.
pub trait RegistrationAttestation: wacore::sync_marker::MaybeSendSync {
    fn attest<'a>(
        &'a self,
        encrypted_body: &'a [u8],
    ) -> BoxFuture<'a, Result<RegistrationAttestationValue, MobileRegistrationError>>;
}

/// Sensitive values deliberately have no Debug implementation.
pub struct RegistrationAttestationValue {
    pub body_attestation: Option<Zeroizing<String>>,
    pub authorization: Option<Zeroizing<String>>,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MobileRegistrationError {
    #[error("{0}")]
    Workflow(#[from] RegistrationError),
    #[error("{0}")]
    Wire(#[from] RegistrationWireError),
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("registration profile, keys or environment differs from the stored binding")]
    BindingMismatch,
    #[error("registration checkpoint encoding is invalid")]
    InvalidCheckpoint,
    #[error("registration requires a nonnegative host clock")]
    Clock,
    #[error(
        "registration delivery is uncertain; resume the retained attempt before starting another"
    )]
    DeliveryUnknown,
    #[error("registration HTTP response was unsuccessful")]
    HttpStatus,
    #[error("registration response does not establish a known transition")]
    UnknownResponse,
    #[error("registration verification omitted a valid canonical account identity")]
    MissingIdentity,
    #[error("registration attestation provider failed")]
    Attestation,
}

/// A response is exposed only after its resulting state is durable. It may carry
/// sensitive challenge/login data and must not be logged indiscriminately.
pub struct MobileRegistrationResult {
    pub response: RegistrationResponse,
    pub decision: RegistrationResponseDecision,
    pub stage: RegistrationStage,
}

const REGISTRATION_CONTEXT_LIMIT: usize = 64 * 1024;
// Maximum bounded delivery hints plus five growing timestamps fit this reserve.
const DYNAMIC_CONTEXT_HEADROOM: usize = 1024;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DriverContext {
    schema_version: u32,
    profile_fingerprint: [u8; 32],
    material: String,
    code_not_before: [u64; 5],
    #[serde(default)]
    delivery: Option<RegistrationDeliveryContext>,
}
impl Drop for DriverContext {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.material.zeroize();
    }
}
impl DriverContext {
    fn decode(binding: &MobileSessionBinding) -> Result<Self, MobileRegistrationError> {
        let value: Self = serde_json::from_slice(&binding.registration_context)
            .map_err(|_| MobileRegistrationError::BindingMismatch)?;
        if value.schema_version != 1 {
            return Err(MobileRegistrationError::BindingMismatch);
        }
        Ok(value)
    }
    fn encode(&self) -> Result<Vec<u8>, MobileRegistrationError> {
        let mut bytes =
            serde_json::to_vec(self).map_err(|_| MobileRegistrationError::InvalidCheckpoint)?;
        if bytes.len() > REGISTRATION_CONTEXT_LIMIT {
            bytes.zeroize();
            return Err(RegistrationWireError::SizeLimit.into());
        }
        Ok(bytes)
    }
}

/// The successful delivery request's selected route and bounded server hints.
/// This acknowledges a request; it does not prove handset delivery. External
/// callers can explicitly declare equivalent metadata on the same host timebase.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationDeliveryContext {
    method: VerificationMethod,
    observed_at_ms: u64,
    hints: RegistrationDeliveryHints,
}
impl RegistrationDeliveryContext {
    pub fn new(
        method: VerificationMethod,
        observed_at_ms: u64,
        hints: RegistrationDeliveryHints,
    ) -> Self {
        Self {
            method,
            observed_at_ms,
            hints,
        }
    }
    pub fn method(&self) -> VerificationMethod {
        self.method
    }
    pub fn observed_at_ms(&self) -> u64 {
        self.observed_at_ms
    }
    pub fn hints(&self) -> &RegistrationDeliveryHints {
        &self.hints
    }
    pub fn flash_expires_at_ms(&self) -> Option<u64> {
        self.hints
            .flash_timeout_seconds()
            .and_then(|seconds| self.observed_at_ms.checked_add(u64::from(seconds) * 1000))
    }
}

/// Proven local handoff after durable registration. No Debug implementation:
/// account number and stable installation material are sensitive.
/// Connection-time Device validation still detects subsequent credential drift.
pub struct RegisteredMobileAccount {
    environment: MobileEnvironment,
    identity: RegistrationIdentity,
    canonical_phone_number: Zeroizing<String>,
    material: RegistrationMaterial,
}
impl RegisteredMobileAccount {
    pub fn environment(&self) -> &MobileEnvironment {
        &self.environment
    }
    pub fn identity(&self) -> RegistrationIdentity {
        self.identity
    }
    pub fn canonical_phone_number(&self) -> &str {
        &self.canonical_phone_number
    }
    pub fn material(&self) -> &RegistrationMaterial {
        &self.material
    }
}

struct PendingAttempt {
    token: AttemptToken,
    endpoint: RegistrationEndpointPath,
    method: Option<VerificationMethod>,
    deadline_ms: u64,
    exchange: Option<BoxFuture<'static, anyhow::Result<HttpResponse>>>,
    response: Option<RegistrationResponse>,
    // This candidate exists only while dispatch has provably never begun.
    unsent_rollback: Option<RegistrationCheckpoint>,
}

/// One writer per namespace, like PersistenceManager. Retaining this object lets
/// cancellation resume the original exchange without sending another SMS/code.
/// Dropping it and restoring an in-flight checkpoint yields OutcomeUnknown.
pub struct MobileRegistrationClient {
    environment: MobileEnvironment,
    identity: RegistrationIdentity,
    store: Arc<PersistenceManager>,
    http: Arc<dyn HttpClient>,
    clock: Arc<dyn Clock>,
    runtime: Arc<dyn Runtime>,
    binding: MobileSessionBinding,
    context: DriverContext,
    workflow: RegistrationWorkflow,
    pending: Option<PendingAttempt>,
    recovering: bool,
    publication: Option<MobileSessionBinding>,
}

impl MobileRegistrationClient {
    /// Atomically install supplied keys and the initial checkpoint in an empty
    /// namespace. The caller owns entropy and retains the exact profile/material.
    #[allow(clippy::too_many_arguments)]
    pub async fn initialize(
        backend: Arc<dyn Backend>,
        keys: MobileDeviceKeys,
        builder: &RegistrationRequestBuilder<'_>,
        environment: MobileEnvironment,
        platform: StoredMobilePlatform,
        run: RegistrationRunId,
        policy: RegistrationPolicy,
        http: Arc<dyn HttpClient>,
        clock: Arc<dyn Clock>,
        runtime: Arc<dyn Runtime>,
    ) -> Result<Self, MobileRegistrationError> {
        Self::initialize_with_material_persistence(
            backend,
            keys,
            builder,
            environment,
            platform,
            run,
            policy,
            http,
            clock,
            runtime,
            RegistrationMaterialPersistence::StableOnly,
        )
        .await
    }

    /// Opt in explicitly when the profile's opaque platform fields/headers are
    /// safe and necessary to retain in protected device storage.
    #[allow(clippy::too_many_arguments)]
    pub async fn initialize_with_material_persistence(
        backend: Arc<dyn Backend>,
        keys: MobileDeviceKeys,
        builder: &RegistrationRequestBuilder<'_>,
        environment: MobileEnvironment,
        platform: StoredMobilePlatform,
        run: RegistrationRunId,
        policy: RegistrationPolicy,
        http: Arc<dyn HttpClient>,
        clock: Arc<dyn Clock>,
        runtime: Arc<dyn Runtime>,
        material_persistence: RegistrationMaterialPersistence,
    ) -> Result<Self, MobileRegistrationError> {
        if platform != stored_platform(builder.platform()) {
            return Err(MobileRegistrationError::BindingMismatch);
        }
        let identity = builder.registration_identity();
        let workflow = RegistrationWorkflow::new(environment.binding(), identity, run, policy);
        let material = RegistrationMaterial::capture_with_policy(builder, material_persistence)?;
        let encoded_material = material.encode()?;
        let context = DriverContext {
            schema_version: 1,
            material: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(encoded_material.as_slice()),
            profile_fingerprint: builder.context_fingerprint()?,
            code_not_before: [0; 5],
            delivery: None,
        };
        let mut initial_context = context.encode()?;
        if initial_context.len() > REGISTRATION_CONTEXT_LIMIT - DYNAMIC_CONTEXT_HEADROOM {
            initial_context.zeroize();
            return Err(RegistrationWireError::SizeLimit.into());
        }
        let binding = MobileSessionBinding::new(
            *environment.binding().as_bytes(),
            *identity.as_bytes(),
            platform,
            initial_context,
            Some(encode(&workflow)?),
        )?;
        let device = keys.into_device(binding.clone())?;
        if !builder.matches_device(&device) {
            return Err(MobileRegistrationError::BindingMismatch);
        }
        let store = Arc::new(PersistenceManager::initialize_mobile(backend, device).await?);
        Ok(Self {
            environment,
            identity,
            store,
            http,
            clock,
            runtime,
            binding,
            context,
            workflow,
            pending: None,
            recovering: false,
            publication: None,
        })
    }

    /// Load through the existing manager; this never regenerates keys or retries.
    pub fn restore(
        store: Arc<PersistenceManager>,
        builder: &RegistrationRequestBuilder<'_>,
        environment: MobileEnvironment,
        http: Arc<dyn HttpClient>,
        clock: Arc<dyn Clock>,
        runtime: Arc<dyn Runtime>,
    ) -> Result<Self, MobileRegistrationError> {
        let snapshot = store.get_device_snapshot();
        snapshot.validate_mobile_session()?;
        if !builder.matches_device(&snapshot.core) {
            return Err(MobileRegistrationError::BindingMismatch);
        }
        let binding = snapshot
            .mobile_session
            .as_deref()
            .cloned()
            .ok_or(MobileRegistrationError::BindingMismatch)?;
        let identity = builder.registration_identity();
        validate_binding(&binding, &environment, identity, builder)?;
        let context = DriverContext::decode(&binding)?;
        let mut workflow = decode(&binding, &environment, identity)?;
        if snapshot.pn.is_none()
            && matches!(
                workflow.stage(),
                RegistrationStage::Verified | RegistrationStage::Registered
            )
        {
            return Err(MobileRegistrationError::BindingMismatch);
        }
        if snapshot.pn.is_some() {
            let token = workflow
                .pending_persistence()
                .ok_or(MobileRegistrationError::BindingMismatch)?;
            workflow.confirm_persisted(token)?;
        }
        Ok(Self {
            environment,
            identity,
            store,
            http,
            clock,
            runtime,
            binding,
            context,
            workflow,
            pending: None,
            recovering: false,
            publication: None,
        })
    }

    /// Restore the complete stable profile and IDs from protected device storage.
    /// Host trust/endpoints and platform attestation providers remain explicit.
    pub fn restore_persisted(
        store: Arc<PersistenceManager>,
        environment: MobileEnvironment,
        http: Arc<dyn HttpClient>,
        clock: Arc<dyn Clock>,
        runtime: Arc<dyn Runtime>,
    ) -> Result<(Self, RegistrationMaterial), MobileRegistrationError> {
        let snapshot = store.get_device_snapshot();
        snapshot.validate_mobile_session()?;
        let binding = snapshot
            .mobile_session
            .as_deref()
            .ok_or(MobileRegistrationError::BindingMismatch)?;
        let context = DriverContext::decode(binding)?;
        let material = decode_material(&context)?;
        let client = Self::restore(
            store,
            &material.builder()?,
            environment,
            http,
            clock,
            runtime,
        )?;
        Ok((client, material))
    }

    /// Produce session-construction evidence only after the final identity and
    /// verified checkpoint are durable. Shared metadata comes from stored material.
    pub fn registered_account(&self) -> Result<RegisteredMobileAccount, MobileRegistrationError> {
        self.available()?;
        if self.stage() != RegistrationStage::Registered {
            return Err(RegistrationError::InvalidTransition.into());
        }
        let snapshot = self.store.get_device_snapshot();
        snapshot.validate_mobile_session()?;
        if snapshot.mobile_session.as_deref() != Some(&self.binding) {
            return Err(MobileRegistrationError::BindingMismatch);
        }
        let material = decode_material(&self.context)?;
        if !material.builder()?.matches_device(&snapshot.core) {
            return Err(MobileRegistrationError::BindingMismatch);
        }
        let pn = snapshot
            .pn
            .as_ref()
            .ok_or(MobileRegistrationError::MissingIdentity)?;
        wacore::store::validate_mobile_primary_pn(pn)
            .map_err(|_| MobileRegistrationError::MissingIdentity)?;
        Ok(RegisteredMobileAccount {
            environment: self.environment.clone(),
            identity: self.identity,
            canonical_phone_number: Zeroizing::new(pn.user.to_string()),
            material,
        })
    }

    pub fn registration_material(&self) -> Result<RegistrationMaterial, MobileRegistrationError> {
        decode_material(&self.context)
    }

    pub fn stage(&self) -> RegistrationStage {
        match self.workflow.stage() {
            RegistrationStage::InFlight {
                token, operation, ..
            } if self.recovering => RegistrationStage::OutcomeUnknown { token, operation },
            stage => stage,
        }
    }
    /// A canceled or failed store/HTTP operation must be settled before another operation.
    pub fn recovery_required(&self) -> bool {
        self.recovering
            || matches!(
                self.workflow.stage(),
                RegistrationStage::InFlight { .. } | RegistrationStage::OutcomeUnknown { .. }
            )
    }
    /// Inspect a retained ambiguous response without retrying or publishing state.
    pub fn pending_response(&self) -> Option<&RegistrationResponse> {
        self.pending
            .as_ref()
            .and_then(|pending| pending.response.as_ref())
    }
    pub fn persistence(&self) -> &Arc<PersistenceManager> {
        &self.store
    }

    /// Explicitly accept an OTP requested elsewhere; performs no HTTP operation.
    pub async fn await_external_code(&mut self) -> Result<(), MobileRegistrationError> {
        self.await_external_code_with_delivery(None).await
    }

    /// Explicit external metadata is optional and never causes an OTP lookup.
    pub async fn await_external_code_with_delivery(
        &mut self,
        delivery: Option<RegistrationDeliveryContext>,
    ) -> Result<(), MobileRegistrationError> {
        self.available()?;
        let now = self.now()?;
        if delivery
            .as_ref()
            .is_some_and(|value| value.observed_at_ms > now)
        {
            return Err(MobileRegistrationError::Clock);
        }
        let mut candidate = self.candidate()?;
        candidate.await_external_code(now)?;
        let mut context = self.context.clone();
        context.delivery = delivery;
        self.recovering = true;
        self.persist_candidate(candidate, None, context).await
    }

    pub fn verification_delivery(&self) -> Option<&RegistrationDeliveryContext> {
        self.context.delivery.as_ref()
    }

    /// Pure caller-ID extraction using explicit overrides or the persisted flash
    /// request's hints. Missing length/route never falls back to a guessed profile.
    /// No call log is read and no code is submitted by this helper.
    pub fn extract_flash_code(
        &self,
        caller_id: &str,
        length: Option<usize>,
        filter: Option<&str>,
    ) -> Result<Zeroizing<String>, MobileRegistrationError> {
        let delivery = self
            .context
            .delivery
            .as_ref()
            .filter(|value| value.method == VerificationMethod::Flash)
            .ok_or(RegistrationWireError::MissingMaterial(
                "flash delivery context",
            ))?;
        let length = length
            .or(delivery.hints.code_length().map(usize::from))
            .ok_or(RegistrationWireError::MissingMaterial("flash code length"))?;
        Ok(
            wacore::mobile::registration_wire::flash_code_from_caller_id(
                caller_id,
                length,
                filter.or(delivery.hints.flash_filter()),
            )?,
        )
    }

    pub async fn check_account(
        &mut self,
        builder: &RegistrationRequestBuilder<'_>,
        request_token: Option<&str>,
        ephemeral: EphemeralRegistrationKey,
        policy: AttestationPolicy,
        attestation: &dyn RegistrationAttestation,
    ) -> Result<MobileRegistrationResult, MobileRegistrationError> {
        self.check_builder(builder)?;
        let request = builder.check_account(request_token)?;
        self.execute(
            RegistrationOperation::CheckAccount,
            request,
            builder,
            None,
            ephemeral,
            policy,
            attestation,
        )
        .await
    }

    /// Request code delivery only through the explicitly selected verification method.
    #[allow(clippy::too_many_arguments)]
    pub async fn request_code(
        &mut self,
        builder: &RegistrationRequestBuilder<'_>,
        method: VerificationMethod,
        request_token: Option<&str>,
        extras: &RegistrationForm,
        ephemeral: EphemeralRegistrationKey,
        policy: AttestationPolicy,
        attestation: &dyn RegistrationAttestation,
    ) -> Result<MobileRegistrationResult, MobileRegistrationError> {
        self.check_builder(builder)?;
        if self.now()? < self.context.code_not_before[method_index(method)] {
            return Err(RegistrationError::Cooldown.into());
        }
        let CodeRequestPlan::Request(request) =
            builder.request_code(Some(method), request_token, extras)?
        else {
            return Err(MobileRegistrationError::UnknownResponse);
        };
        self.execute(
            RegistrationOperation::RequestCode,
            request,
            builder,
            Some(method),
            ephemeral,
            policy,
            attestation,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn submit_code(
        &mut self,
        builder: &RegistrationRequestBuilder<'_>,
        code: &str,
        request_token: Option<&str>,
        extras: &RegistrationForm,
        ephemeral: EphemeralRegistrationKey,
        policy: AttestationPolicy,
        attestation: &dyn RegistrationAttestation,
    ) -> Result<MobileRegistrationResult, MobileRegistrationError> {
        self.check_builder(builder)?;
        let request = builder.submit_code(code, request_token, extras)?;
        self.execute(
            RegistrationOperation::SubmitCode,
            request,
            builder,
            None,
            ephemeral,
            policy,
            attestation,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn submit_challenge(
        &mut self,
        builder: &RegistrationRequestBuilder<'_>,
        answer: &str,
        request_token: Option<&str>,
        extras: &RegistrationForm,
        ephemeral: EphemeralRegistrationKey,
        policy: AttestationPolicy,
        attestation: &dyn RegistrationAttestation,
    ) -> Result<MobileRegistrationResult, MobileRegistrationError> {
        self.check_builder(builder)?;
        let request = builder.submit_challenge(answer, request_token, extras)?;
        self.execute(
            RegistrationOperation::SubmitChallenge,
            request,
            builder,
            None,
            ephemeral,
            policy,
            attestation,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn submit_pin(
        &mut self,
        builder: &RegistrationRequestBuilder<'_>,
        pin: &str,
        request_token: Option<&str>,
        extras: &RegistrationForm,
        ephemeral: EphemeralRegistrationKey,
        policy: AttestationPolicy,
        attestation: &dyn RegistrationAttestation,
    ) -> Result<MobileRegistrationResult, MobileRegistrationError> {
        self.check_builder(builder)?;
        let request = builder.submit_pin(pin, request_token, extras)?;
        self.execute(
            RegistrationOperation::SubmitPin,
            request,
            builder,
            None,
            ephemeral,
            policy,
            attestation,
        )
        .await
    }

    fn available(&self) -> Result<(), MobileRegistrationError> {
        if self.recovery_required() || self.pending.is_some() {
            return Err(MobileRegistrationError::DeliveryUnknown);
        }
        Ok(())
    }
    fn check_builder(
        &self,
        builder: &RegistrationRequestBuilder<'_>,
    ) -> Result<(), MobileRegistrationError> {
        self.available()?;
        let snapshot = self.store.get_device_snapshot();
        snapshot.validate_mobile_session()?;
        if snapshot.mobile_session.as_deref() != Some(&self.binding)
            || !builder.matches_device(&snapshot.core)
        {
            return Err(MobileRegistrationError::BindingMismatch);
        }
        validate_binding(&self.binding, &self.environment, self.identity, builder)
    }
    fn now(&self) -> Result<u64, MobileRegistrationError> {
        u64::try_from(self.clock.wall_time_millis()).map_err(|_| MobileRegistrationError::Clock)
    }
    fn candidate(&self) -> Result<RegistrationWorkflow, MobileRegistrationError> {
        Ok(RegistrationWorkflow::restore(
            self.workflow.checkpoint(),
            self.environment.binding(),
            self.identity,
        )?)
    }

    async fn persist_candidate(
        &mut self,
        candidate: RegistrationWorkflow,
        identity: Option<(Jid, Option<Jid>)>,
        context: DriverContext,
    ) -> Result<(), MobileRegistrationError> {
        let mut next = self.binding.clone();
        let checkpoint = encode(&candidate)?;
        let context_bytes = context.encode()?;
        next.registration_checkpoint.zeroize();
        next.registration_context.zeroize();
        next.registration_checkpoint = Some(checkpoint);
        next.registration_context = context_bytes;
        self.publication = Some(next.clone());
        self.store
            .update_mobile_registration(self.binding.clone(), next.clone(), identity)
            .await?;
        self.binding = next;
        self.publication = None;
        self.context = context;
        self.workflow = candidate;
        if let Some(token) = self.workflow.pending_persistence() {
            self.workflow.confirm_persisted(token)?;
        }
        self.recovering = false;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute(
        &mut self,
        operation: RegistrationOperation,
        request: RegistrationRequest,
        builder: &RegistrationRequestBuilder<'_>,
        method: Option<VerificationMethod>,
        ephemeral: EphemeralRegistrationKey,
        policy: AttestationPolicy,
        attestation: &dyn RegistrationAttestation,
    ) -> Result<MobileRegistrationResult, MobileRegistrationError> {
        self.available()?;
        let mut preflight = self.candidate()?;
        let _ = preflight.begin(operation, self.now()?)?;
        // All fallible preparation and provider work precedes attempt reservation.
        let sealed = request.seal(ephemeral, self.environment.trust().registration_key())?;
        let proof = attestation.attest(sealed.attestation_input()).await?;
        self.check_builder(builder)?;
        let wire = sealed.finish(
            policy,
            BodyAttestation {
                body_attestation: proof.body_attestation.as_deref().map(String::as_str),
                authorization: proof.authorization.as_deref().map(String::as_str),
            },
        )?;
        let mut url = format!(
            "{}{}",
            self.environment
                .registration()
                .as_str()
                .trim_end_matches('/'),
            wire.endpoint().as_str()
        );
        if let Some(query) = wire.query() {
            url.push('?');
            url.push_str(query);
        }
        let mut http = HttpRequest::post(url).with_body(wire.body().to_vec());
        for (name, value) in wire.headers() {
            http = http.with_header(name, value);
        }
        let now = self.now()?;
        let mut rollback = self.candidate()?;
        let rollback_token = rollback.begin(operation, now)?;
        rollback.abandon_unsent(rollback_token, now)?;
        let token = self.workflow.begin(operation, now)?;
        let RegistrationStage::InFlight { deadline_ms, .. } = self.workflow.stage() else {
            return Err(MobileRegistrationError::InvalidCheckpoint);
        };
        self.pending = Some(PendingAttempt {
            token,
            endpoint: wire.endpoint(),
            method,
            deadline_ms,
            exchange: None,
            response: None,
            unsent_rollback: Some(rollback.checkpoint()),
        });
        self.recovering = true;
        let mut next = self.binding.clone();
        let checkpoint = encode(&self.workflow)?;
        next.registration_checkpoint.zeroize();
        next.registration_checkpoint = Some(checkpoint);
        self.publication = Some(next.clone());
        self.store
            .update_mobile_registration(self.binding.clone(), next.clone(), None)
            .await?;
        self.binding = next;
        self.publication = None;
        self.workflow.delivery_uncertain(token)?;
        let client = self.http.clone();
        // Creating this future does not dispatch; it is retained across cancellation.
        if let Some(pending) = self.pending.as_mut() {
            pending.unsent_rollback = None;
            pending.exchange = Some(Box::pin(async move { client.execute(http).await }));
        }
        self.finish_pending(false).await
    }

    /// Resume the *same* retained exchange, or settle a canceled store write.
    /// A process restart cannot recover a lost HTTP response and never resends it.
    /// After a timeout this explicitly awaits the original exchange again; the
    /// host may cancel this wait without losing the retained exchange.
    pub async fn resume_pending(
        &mut self,
    ) -> Result<Option<MobileRegistrationResult>, MobileRegistrationError> {
        self.store.flush().await?;
        let snapshot = self.store.get_device_snapshot();
        snapshot.validate_mobile_session()?;
        let actual = snapshot
            .mobile_session
            .as_deref()
            .cloned()
            .ok_or(MobileRegistrationError::BindingMismatch)?;
        if actual != self.binding {
            if self.publication.as_ref() != Some(&actual) {
                return Err(MobileRegistrationError::BindingMismatch);
            }
            if actual.platform != self.binding.platform
                || actual.role != self.binding.role
                || DriverContext::decode(&actual)?.profile_fingerprint
                    != self.context.profile_fingerprint
            {
                return Err(MobileRegistrationError::BindingMismatch);
            }
            let candidate = decode(&actual, &self.environment, self.identity)?;
            if snapshot.pn.is_none()
                && matches!(
                    candidate.stage(),
                    RegistrationStage::Verified | RegistrationStage::Registered
                )
            {
                return Err(MobileRegistrationError::BindingMismatch);
            }
            self.workflow = candidate;
            self.context = DriverContext::decode(&actual)?;
            self.binding = actual;
            self.publication = None;
            if snapshot.pn.is_some() {
                let token = self
                    .workflow
                    .pending_persistence()
                    .ok_or(MobileRegistrationError::BindingMismatch)?;
                self.workflow.confirm_persisted(token)?;
            }
        }
        if let Some(checkpoint) = self
            .pending
            .as_ref()
            .and_then(|p| p.unsent_rollback.clone())
        {
            let candidate = RegistrationWorkflow::restore(
                checkpoint,
                self.environment.binding(),
                self.identity,
            )?;
            self.persist_candidate(candidate, None, self.context.clone())
                .await?;
            self.pending = None;
            return Ok(None);
        }
        if !matches!(
            self.workflow.stage(),
            RegistrationStage::InFlight { .. } | RegistrationStage::OutcomeUnknown { .. }
        ) {
            self.recovering = false;
            self.publication = None;
            if let Some(pending) = self.pending.take()
                && let Some(response) = pending.response
            {
                let decision = response.decision(pending.endpoint);
                return Ok(Some(MobileRegistrationResult {
                    response,
                    decision,
                    stage: self.stage(),
                }));
            }
            return Ok(None);
        }
        if self.pending.is_none() {
            return Err(MobileRegistrationError::DeliveryUnknown);
        }
        self.finish_pending(true).await.map(Some)
    }

    async fn finish_pending(
        &mut self,
        resume: bool,
    ) -> Result<MobileRegistrationResult, MobileRegistrationError> {
        let now = self.now()?;
        let pending = self
            .pending
            .as_mut()
            .ok_or(MobileRegistrationError::DeliveryUnknown)?;
        if pending.response.is_none() {
            let exchange = pending
                .exchange
                .as_mut()
                .ok_or(MobileRegistrationError::DeliveryUnknown)?;
            let raw = if resume {
                match exchange.await {
                    Ok(response) => response,
                    Err(_) => {
                        pending.exchange = None;
                        return Err(MobileRegistrationError::DeliveryUnknown);
                    }
                }
            } else {
                let wait = self.runtime.sleep(Duration::from_millis(
                    pending.deadline_ms.saturating_sub(now),
                ));
                match select(exchange, wait).await {
                    Either::Left((Ok(response), _)) => response,
                    Either::Left((Err(_), _)) => {
                        pending.exchange = None;
                        return Err(MobileRegistrationError::DeliveryUnknown);
                    }
                    Either::Right(_) => return Err(MobileRegistrationError::DeliveryUnknown),
                }
            };
            pending.exchange = None;
            let body = Zeroizing::new(raw.body);
            if !(200..300).contains(&raw.status_code) {
                return Err(MobileRegistrationError::HttpStatus);
            }
            pending.response = Some(RegistrationResponse::parse(&body)?);
        }
        let pending = self
            .pending
            .as_ref()
            .ok_or(MobileRegistrationError::DeliveryUnknown)?;
        let response = pending
            .response
            .as_ref()
            .ok_or(MobileRegistrationError::DeliveryUnknown)?;
        let decision = response.decision(pending.endpoint);
        let preferred_method = pending.method.or_else(|| {
            self.context
                .delivery
                .as_ref()
                .map(RegistrationDeliveryContext::method)
        });
        let outcome = match decision {
            RegistrationResponseDecision::AccountKeysDiffer
            | RegistrationResponseDecision::AccountRecognized => RegistrationOutcome::Ready,
            RegistrationResponseDecision::CodeSent => RegistrationOutcome::NeedsCode,
            RegistrationResponseDecision::ChallengeRequired => RegistrationOutcome::NeedsChallenge,
            RegistrationResponseDecision::PinRequired => RegistrationOutcome::NeedsPin,
            RegistrationResponseDecision::Registered => RegistrationOutcome::Verified,
            RegistrationResponseDecision::RateLimited => RegistrationOutcome::RetryAfter {
                delay_ms: response
                    .wait_hint_seconds(preferred_method)
                    .and_then(|n| n.checked_mul(1000))
                    .ok_or(MobileRegistrationError::UnknownResponse)?,
            },
            RegistrationResponseDecision::NoRoute
            | RegistrationResponseDecision::RequiredInput
            | RegistrationResponseDecision::Rejected => RegistrationOutcome::RetryAfter {
                delay_ms: response
                    .wait_hint_seconds(preferred_method)
                    .unwrap_or(0)
                    .checked_mul(1000)
                    .ok_or(MobileRegistrationError::UnknownResponse)?,
            },
            RegistrationResponseDecision::Blocked => RegistrationOutcome::Rejected,
            _ => return Err(MobileRegistrationError::UnknownResponse),
        };
        let final_identity = if decision == RegistrationResponseDecision::Registered {
            // Both pinned reference drivers retain the submitted phone when a
            // successful response omits login. Present invalid echoes never fall back.
            let submitted = decode_material(&self.context)?.submitted_phone_number();
            let login = response
                .canonical_login()
                .or(Some(submitted.as_str()))
                .filter(|v| {
                    !v.is_empty()
                        && !v.starts_with('0')
                        && v.len() <= 15
                        && v.bytes().all(|b| b.is_ascii_digit())
                })
                .ok_or(MobileRegistrationError::MissingIdentity)?;
            let pn = Jid::new(login, Server::Pn);
            wacore::store::validate_mobile_primary_pn(&pn)
                .map_err(|_| MobileRegistrationError::MissingIdentity)?;
            Some((pn, None))
        } else {
            None
        };
        let mut candidate = self.candidate()?;
        candidate.complete(pending.token, outcome, self.now()?)?;
        let mut next_context = self.context.clone();
        let now = self.now()?;
        if decision == RegistrationResponseDecision::CodeSent {
            let method = pending
                .method
                .ok_or(MobileRegistrationError::UnknownResponse)?;
            next_context.delivery = Some(RegistrationDeliveryContext::new(
                method,
                now,
                response.delivery_hints().cloned().unwrap_or_default(),
            ));
        }
        for method in [
            VerificationMethod::Sms,
            VerificationMethod::Voice,
            VerificationMethod::WhatsApp,
            VerificationMethod::Flash,
            VerificationMethod::Email,
        ] {
            if let Some(seconds) = response.waits().seconds_for(method) {
                let deadline = seconds
                    .checked_mul(1000)
                    .and_then(|delay| now.checked_add(delay))
                    .ok_or(RegistrationError::TimestampOverflow)?;
                next_context.code_not_before[method_index(method)] = deadline;
            }
        }
        self.persist_candidate(candidate, final_identity, next_context)
            .await?;
        let pending = self
            .pending
            .take()
            .ok_or(MobileRegistrationError::DeliveryUnknown)?;
        let response = pending
            .response
            .ok_or(MobileRegistrationError::DeliveryUnknown)?;
        Ok(MobileRegistrationResult {
            response,
            decision,
            stage: self.stage(),
        })
    }
}

fn decode_material(
    context: &DriverContext,
) -> Result<RegistrationMaterial, MobileRegistrationError> {
    let bytes = Zeroizing::new(
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&context.material)
            .map_err(|_| MobileRegistrationError::BindingMismatch)?,
    );
    let material = RegistrationMaterial::decode(&bytes)?;
    if material.builder()?.context_fingerprint()? != context.profile_fingerprint {
        return Err(MobileRegistrationError::BindingMismatch);
    }
    Ok(material)
}
fn stored_platform(
    platform: wacore::mobile::registration_wire::RegistrationPlatform,
) -> StoredMobilePlatform {
    use wacore::mobile::registration_wire::RegistrationPlatform as P;
    match platform {
        P::Android => StoredMobilePlatform::Android,
        P::AndroidBusiness => StoredMobilePlatform::AndroidBusiness,
        P::Ios => StoredMobilePlatform::Ios,
        P::IosBusiness => StoredMobilePlatform::IosBusiness,
    }
}
fn method_index(method: VerificationMethod) -> usize {
    match method {
        VerificationMethod::Sms => 0,
        VerificationMethod::Voice => 1,
        VerificationMethod::WhatsApp => 2,
        VerificationMethod::Flash => 3,
        VerificationMethod::Email => 4,
    }
}
fn validate_binding(
    binding: &MobileSessionBinding,
    environment: &MobileEnvironment,
    identity: RegistrationIdentity,
    builder: &RegistrationRequestBuilder<'_>,
) -> Result<(), MobileRegistrationError> {
    if binding.environment_hash != *environment.binding().as_bytes()
        || binding.identity_hash != *identity.as_bytes()
        || builder.registration_identity() != identity
        || binding.platform != stored_platform(builder.platform())
        || DriverContext::decode(binding)?.profile_fingerprint != builder.context_fingerprint()?
    {
        return Err(MobileRegistrationError::BindingMismatch);
    }
    Ok(())
}
fn encode(workflow: &RegistrationWorkflow) -> Result<Vec<u8>, MobileRegistrationError> {
    serde_json::to_vec(&workflow.checkpoint())
        .map_err(|_| MobileRegistrationError::InvalidCheckpoint)
}
fn decode(
    binding: &MobileSessionBinding,
    environment: &MobileEnvironment,
    identity: RegistrationIdentity,
) -> Result<RegistrationWorkflow, MobileRegistrationError> {
    if binding.environment_hash != *environment.binding().as_bytes()
        || binding.identity_hash != *identity.as_bytes()
    {
        return Err(MobileRegistrationError::BindingMismatch);
    }
    let bytes = binding
        .registration_checkpoint
        .as_ref()
        .ok_or(MobileRegistrationError::InvalidCheckpoint)?;
    let checkpoint =
        serde_json::from_slice(bytes).map_err(|_| MobileRegistrationError::InvalidCheckpoint)?;
    Ok(RegistrationWorkflow::restore(
        checkpoint,
        environment.binding(),
        identity,
    )?)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "registration_tests.rs"]
mod tests;
