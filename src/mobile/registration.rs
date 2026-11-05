//! Portable, durable registration driver. No HTTP operation is retried implicitly.
//!
//! The host retains its artifact-qualified profile and registration material. Its
//! fingerprint is persisted alongside the keys; changing either requires another
//! empty storage namespace. Cancellation retains the exchange inside this driver.
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
        RegistrationEndpointPath, RegistrationForm, RegistrationMaterial,
        RegistrationMaterialPersistence, RegistrationRequest, RegistrationRequestBuilder,
        RegistrationResponse, RegistrationResponseDecision, RegistrationWireError,
        VerificationMethod,
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
    #[error(transparent)]
    Workflow(#[from] RegistrationError),
    #[error(transparent)]
    Wire(#[from] RegistrationWireError),
    #[error(transparent)]
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DriverContext {
    schema_version: u32,
    profile_fingerprint: [u8; 32],
    material: String,
    code_not_before: [u64; 5],
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
        if bytes.len() > 64 * 1024 {
            bytes.zeroize();
            return Err(RegistrationWireError::SizeLimit.into());
        }
        Ok(bytes)
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
        };
        let binding = MobileSessionBinding::new(
            *environment.binding().as_bytes(),
            *identity.as_bytes(),
            platform,
            context.encode()?,
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
        self.available()?;
        let mut candidate = self.candidate()?;
        candidate.await_external_code(self.now()?)?;
        self.recovering = true;
        self.persist_candidate(candidate, None, self.context.clone())
            .await
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
        if self.recovering || self.pending.is_some() {
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
            self.workflow = decode(&actual, &self.environment, self.identity)?;
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
        let outcome = match decision {
            RegistrationResponseDecision::AccountKeysDiffer
            | RegistrationResponseDecision::AccountRecognized => RegistrationOutcome::Ready,
            RegistrationResponseDecision::CodeSent => RegistrationOutcome::NeedsCode,
            RegistrationResponseDecision::ChallengeRequired => RegistrationOutcome::NeedsChallenge,
            RegistrationResponseDecision::PinRequired => RegistrationOutcome::NeedsPin,
            RegistrationResponseDecision::Registered => RegistrationOutcome::Verified,
            RegistrationResponseDecision::RateLimited => RegistrationOutcome::RetryAfter {
                delay_ms: response
                    .wait_hint_seconds(pending.method)
                    .and_then(|n| n.checked_mul(1000))
                    .ok_or(MobileRegistrationError::UnknownResponse)?,
            },
            RegistrationResponseDecision::NoRoute
            | RegistrationResponseDecision::RequiredInput
            | RegistrationResponseDecision::Rejected => RegistrationOutcome::RetryAfter {
                delay_ms: response
                    .wait_hint_seconds(pending.method)
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
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures::channel::oneshot;
    use rand::SeedableRng;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wacore::libsignal::protocol::KeyPair;
    use wacore::mobile::registration_wire::{
        Base64Padding, RegistrationAppVersion, RegistrationCommonFields, RegistrationEncoding,
        RegistrationHeaderProfile, RegistrationIdentityFields, RegistrationPlatform,
    };
    use wacore::mobile::{MobileTrust, RegistrationEndpoint, TcpEndpoint};
    use wacore::runtime::AbortHandle;
    use wacore::store::{InMemoryBackend, traits::DeviceStore};
    use wacore::time::{Instant, VirtualClock};

    struct TestRuntime;
    #[async_trait]
    impl Runtime for TestRuntime {
        fn spawn(&self, _: BoxFuture<'static, ()>) -> AbortHandle {
            panic!("driver must not spawn")
        }
        fn sleep(&self, _: Duration) -> BoxFuture<'static, ()> {
            Box::pin(futures::future::pending())
        }
        fn spawn_blocking(&self, _: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture<'static, ()> {
            panic!("driver must not perform blocking I/O")
        }
        fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
            None
        }
    }
    struct TestProof;
    impl RegistrationAttestation for TestProof {
        fn attest<'a>(
            &'a self,
            encrypted_body: &'a [u8],
        ) -> BoxFuture<'a, Result<RegistrationAttestationValue, MobileRegistrationError>> {
            Box::pin(async move {
                assert!(!encrypted_body.is_empty());
                Ok(RegistrationAttestationValue {
                    body_attestation: Some(Zeroizing::new("synthetic-proof".into())),
                    authorization: None,
                })
            })
        }
    }
    enum Reply {
        Ready(&'static [u8]),
        Deferred(oneshot::Receiver<&'static [u8]>),
        Failure,
    }
    struct MockHttp {
        backend: Arc<InMemoryBackend>,
        replies: Mutex<VecDeque<Reply>>,
        calls: AtomicUsize,
    }
    #[async_trait]
    impl HttpClient for MockHttp {
        async fn execute(&self, request: HttpRequest) -> anyhow::Result<HttpResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let durable = self
                .backend
                .load()
                .await?
                .expect("keys durable before network");
            let checkpoint: RegistrationCheckpoint = serde_json::from_slice(
                durable
                    .mobile_session
                    .as_ref()
                    .unwrap()
                    .registration_checkpoint
                    .as_ref()
                    .unwrap(),
            )
            .unwrap();
            let restored = RegistrationWorkflow::restore(
                checkpoint,
                environment().binding(),
                RegistrationIdentity::from_public_keys(
                    durable
                        .identity_key
                        .public_key
                        .public_key_bytes()
                        .try_into()
                        .unwrap(),
                    durable
                        .noise_key
                        .public_key
                        .public_key_bytes()
                        .try_into()
                        .unwrap(),
                ),
            )?;
            assert!(matches!(
                restored.stage(),
                RegistrationStage::OutcomeUnknown { .. }
            ));
            assert!(request.url.starts_with("http://mock.invalid/mobile/"));
            assert!(request.body.as_ref().unwrap().starts_with(b"ENC="));
            assert_eq!(
                request.headers.get("request_token").map(String::as_str),
                Some(UUID)
            );
            let reply = self.replies.lock().unwrap().pop_front().unwrap();
            let bytes = match reply {
                Reply::Ready(bytes) => bytes,
                Reply::Deferred(receiver) => receiver.await?,
                Reply::Failure => anyhow::bail!("synthetic transport failure"),
            };
            Ok(HttpResponse::new(200, bytes.to_vec()))
        }
    }
    const UUID: &str = "00000000-0000-4000-8000-000000000001";
    const POLICY: AttestationPolicy = AttestationPolicy {
        require_body_attestation: true,
        require_authorization: false,
    };
    fn environment() -> MobileEnvironment {
        MobileEnvironment::new(
            "synthetic",
            RegistrationEndpoint::for_test_http("http://mock.invalid/mobile").unwrap(),
            TcpEndpoint::new("mock.invalid", 5222).unwrap(),
            MobileTrust::new([9; 32], [7; 32]).unwrap(),
        )
        .unwrap()
    }
    struct Fixture {
        keys: MobileDeviceKeys,
        noise: [u8; 32],
        identity: [u8; 32],
        prekey: [u8; 32],
        header: RegistrationHeaderProfile,
        encoding: RegistrationEncoding,
        fields: RegistrationForm,
    }
    impl Fixture {
        fn new() -> Self {
            let mut rng = rand::rngs::StdRng::seed_from_u64(773);
            let identity = KeyPair::generate(&mut rng);
            let noise = KeyPair::generate(&mut rng);
            let prekey = KeyPair::generate(&mut rng);
            let signature = identity
                .calculate_signature(&prekey.public_key.serialize(), &mut rng)
                .unwrap();
            Self {
                noise: noise.public_key.public_key_bytes().try_into().unwrap(),
                identity: identity.public_key.public_key_bytes().try_into().unwrap(),
                prekey: prekey.public_key.public_key_bytes().try_into().unwrap(),
                keys: MobileDeviceKeys {
                    registration_id: 123,
                    noise_key: noise,
                    identity_key: identity,
                    signed_pre_key: prekey,
                    signed_pre_key_id: 1,
                    signed_pre_key_signature: signature,
                    adv_secret_key: [6; 32],
                },
                header: RegistrationHeaderProfile::new(
                    RegistrationPlatform::Android,
                    &RegistrationAppVersion::new(&[2, 26, 1, 1]).unwrap(),
                    "15",
                    "Synthetic",
                    "Test",
                    vec![],
                )
                .unwrap(),
                encoding: RegistrationEncoding {
                    signal_padding: Base64Padding::Padded,
                    token_on_register: true,
                    mirror_code_and_register_query: false,
                    code_order: vec![],
                    register_order: vec![],
                },
                fields: RegistrationForm::new(),
            }
        }
        fn builder(&self) -> RegistrationRequestBuilder<'_> {
            RegistrationRequestBuilder::new(
                &self.header,
                &self.encoding,
                RegistrationCommonFields {
                    country_code: "999",
                    national_number: "01000000",
                    release_channel: 0,
                    language: "en",
                    locale_country: "ZZ",
                    family_device_id: UUID,
                    access_session_id: "synthetic-session",
                    registration_token: Some("synthetic-token"),
                    business_certificate: None,
                    platform_fields: &self.fields,
                    identity: RegistrationIdentityFields {
                        noise_public_key: &self.noise,
                        registration_id: self.keys.registration_id,
                        identity_public_key: &self.identity,
                        signed_prekey_id: 1,
                        signed_prekey_public: &self.prekey,
                        signed_prekey_signature: &self.keys.signed_pre_key_signature,
                        experience_id: &[5; 16],
                        identity_id: b"synthetic-id",
                    },
                },
            )
            .unwrap()
        }
        async fn client(&self, replies: Vec<Reply>) -> (MobileRegistrationClient, Arc<MockHttp>) {
            let backend = Arc::new(InMemoryBackend::new());
            let http = Arc::new(MockHttp {
                backend: backend.clone(),
                replies: Mutex::new(replies.into()),
                calls: AtomicUsize::new(0),
            });
            let client = MobileRegistrationClient::initialize(
                backend,
                self.keys.clone(),
                &self.builder(),
                environment(),
                StoredMobilePlatform::Android,
                RegistrationRunId::from_bytes([12; 32]),
                RegistrationPolicy::new(10, 1000).unwrap(),
                http.clone(),
                Arc::new(VirtualClock::new(1000, Instant::ZERO).unwrap()),
                Arc::new(TestRuntime),
            )
            .await
            .unwrap();
            (client, http)
        }
    }
    #[tokio::test]
    async fn wire_workflow_persists_keys_before_dispatch_and_identity_before_success() {
        {
            let fixture = Fixture::new();
            let (mut client, http) = fixture
                .client(vec![
                    Reply::Ready(br#"{"status":"fail","reason":"incorrect"}"#),
                    Reply::Ready(br#"{"status":"sent","sms_wait":5}"#),
                    Reply::Ready(br#"{"status":"ok","login":"99901000000"}"#),
                ])
                .await;
            let builder = fixture.builder();
            let lookup = client
                .check_account(
                    &builder,
                    Some(UUID),
                    EphemeralRegistrationKey::from_bytes([21; 32]),
                    POLICY,
                    &TestProof,
                )
                .await
                .unwrap();
            assert_eq!(
                lookup.decision,
                RegistrationResponseDecision::AccountKeysDiffer
            );
            assert_eq!(lookup.stage, RegistrationStage::Ready);
            assert_eq!(http.calls.load(Ordering::SeqCst), 1);
            client
                .request_code(
                    &builder,
                    VerificationMethod::Sms,
                    Some(UUID),
                    &fixture.fields,
                    EphemeralRegistrationKey::from_bytes([22; 32]),
                    POLICY,
                    &TestProof,
                )
                .await
                .unwrap();
            assert_eq!(client.stage(), RegistrationStage::AwaitingCode);
            assert!(matches!(
                client
                    .request_code(
                        &builder,
                        VerificationMethod::Sms,
                        Some(UUID),
                        &fixture.fields,
                        EphemeralRegistrationKey::from_bytes([28; 32]),
                        POLICY,
                        &TestProof
                    )
                    .await,
                Err(MobileRegistrationError::Workflow(
                    RegistrationError::Cooldown
                ))
            ));
            assert_eq!(http.calls.load(Ordering::SeqCst), 2);
            let complete = client
                .submit_code(
                    &builder,
                    "123456",
                    Some(UUID),
                    &fixture.fields,
                    EphemeralRegistrationKey::from_bytes([23; 32]),
                    POLICY,
                    &TestProof,
                )
                .await
                .unwrap();
            assert_eq!(complete.stage, RegistrationStage::Registered);
            let account = client.registered_account().unwrap();
            assert_eq!(account.canonical_phone_number(), "99901000000");
            assert_eq!(account.material().login_metadata().family_device_id, UUID);
            assert_eq!(account.environment().binding(), environment().binding());
            let durable = http.backend.load().await.unwrap().unwrap();
            assert_eq!(durable.pn.unwrap().user, "99901000000");
            let restored = MobileRegistrationClient::restore(
                client.store.clone(),
                &builder,
                environment(),
                http.clone(),
                client.clock.clone(),
                Arc::new(TestRuntime),
            )
            .unwrap();
            assert_eq!(restored.stage(), RegistrationStage::Registered);
            let (restored, material) = MobileRegistrationClient::restore_persisted(
                client.store.clone(),
                environment(),
                http.clone(),
                client.clock.clone(),
                Arc::new(TestRuntime),
            )
            .unwrap();
            assert_eq!(restored.stage(), RegistrationStage::Registered);
            assert_eq!(
                material.builder().unwrap().context_fingerprint().unwrap(),
                builder.context_fingerprint().unwrap()
            );
            assert_eq!(http.calls.load(Ordering::SeqCst), 3);
        }
    }
    #[tokio::test]
    async fn canceled_exchange_resumes_without_another_dispatch() {
        {
            let fixture = Fixture::new();
            let (sender, receiver) = oneshot::channel();
            let (mut client, http) = fixture.client(vec![Reply::Deferred(receiver)]).await;
            let builder = fixture.builder();
            let mut attempt = Box::pin(client.request_code(
                &builder,
                VerificationMethod::Sms,
                Some(UUID),
                &fixture.fields,
                EphemeralRegistrationKey::from_bytes([24; 32]),
                POLICY,
                &TestProof,
            ));
            assert!(futures::poll!(&mut attempt).is_pending());
            drop(attempt);
            assert!(matches!(
                client.stage(),
                RegistrationStage::OutcomeUnknown { .. }
            ));
            assert_eq!(http.calls.load(Ordering::SeqCst), 1);
            assert!(matches!(
                client
                    .check_account(
                        &builder,
                        Some(UUID),
                        EphemeralRegistrationKey::from_bytes([25; 32]),
                        POLICY,
                        &TestProof
                    )
                    .await,
                Err(MobileRegistrationError::DeliveryUnknown)
            ));
            sender.send(br#"{"status":"sent"}"#).unwrap();
            assert_eq!(
                client.resume_pending().await.unwrap().unwrap().stage,
                RegistrationStage::AwaitingCode
            );
            assert_eq!(http.calls.load(Ordering::SeqCst), 1);
        }
    }
    #[tokio::test]
    async fn transport_failure_never_requests_an_otp_or_reports_fresh_account() {
        {
            let fixture = Fixture::new();
            let (mut client, http) = fixture.client(vec![Reply::Failure]).await;
            assert!(matches!(
                client
                    .check_account(
                        &fixture.builder(),
                        Some(UUID),
                        EphemeralRegistrationKey::from_bytes([26; 32]),
                        POLICY,
                        &TestProof
                    )
                    .await,
                Err(MobileRegistrationError::DeliveryUnknown)
            ));
            assert_eq!(http.calls.load(Ordering::SeqCst), 1);
            assert!(matches!(
                client.stage(),
                RegistrationStage::OutcomeUnknown { .. }
            ));
            assert!(matches!(
                client.resume_pending().await,
                Err(MobileRegistrationError::DeliveryUnknown)
            ));
        }
    }
    #[tokio::test]
    async fn challenge_and_pin_are_explicit_paths_and_profile_drift_is_rejected() {
        {
            let mut fixture = Fixture::new();
            let (mut client, http) = fixture
                .client(vec![
                    Reply::Ready(br#"{"status":"fail","image_blob":"AQI="}"#),
                    Reply::Ready(br#"{"status":"fail","reason":"2fa_required"}"#),
                    Reply::Ready(br#"{"status":"ok","login":"99901000000"}"#),
                ])
                .await;
            {
                let builder = fixture.builder();
                let result = client
                    .request_code(
                        &builder,
                        VerificationMethod::Voice,
                        Some(UUID),
                        &fixture.fields,
                        EphemeralRegistrationKey::from_bytes([31; 32]),
                        POLICY,
                        &TestProof,
                    )
                    .await
                    .unwrap();
                assert_eq!(result.stage, RegistrationStage::AwaitingChallenge);
                assert_eq!(
                    result.response.challenge().unwrap().image.as_deref(),
                    Some([1, 2].as_slice())
                );
                let result = client
                    .submit_challenge(
                        &builder,
                        "synthetic-answer",
                        Some(UUID),
                        &fixture.fields,
                        EphemeralRegistrationKey::from_bytes([32; 32]),
                        POLICY,
                        &TestProof,
                    )
                    .await
                    .unwrap();
                assert_eq!(result.stage, RegistrationStage::AwaitingPin);
                let result = client
                    .submit_pin(
                        &builder,
                        "123456",
                        Some(UUID),
                        &fixture.fields,
                        EphemeralRegistrationKey::from_bytes([33; 32]),
                        POLICY,
                        &TestProof,
                    )
                    .await
                    .unwrap();
                assert_eq!(result.stage, RegistrationStage::Registered);
            }
            fixture.keys.registration_id += 1;
            assert!(matches!(
                MobileRegistrationClient::restore(
                    client.store.clone(),
                    &fixture.builder(),
                    environment(),
                    http.clone(),
                    client.clock.clone(),
                    Arc::new(TestRuntime)
                ),
                Err(MobileRegistrationError::BindingMismatch)
            ));
            assert_eq!(http.calls.load(Ordering::SeqCst), 3);
        }
    }

    #[tokio::test]
    async fn external_code_and_unsent_attestation_failure_have_no_http_side_effect() {
        struct FailedProof;
        impl RegistrationAttestation for FailedProof {
            fn attest<'a>(
                &'a self,
                _: &'a [u8],
            ) -> BoxFuture<'a, Result<RegistrationAttestationValue, MobileRegistrationError>>
            {
                Box::pin(async { Err(MobileRegistrationError::Attestation) })
            }
        }
        {
            let fixture = Fixture::new();
            let (mut client, http) = fixture.client(vec![]).await;
            assert!(matches!(
                client
                    .check_account(
                        &fixture.builder(),
                        Some(UUID),
                        EphemeralRegistrationKey::from_bytes([34; 32]),
                        POLICY,
                        &FailedProof
                    )
                    .await,
                Err(MobileRegistrationError::Attestation)
            ));
            assert_eq!(client.stage(), RegistrationStage::Ready);
            client.await_external_code().await.unwrap();
            assert_eq!(client.stage(), RegistrationStage::AwaitingCode);
            assert_eq!(http.calls.load(Ordering::SeqCst), 0);
            let restored = MobileRegistrationClient::restore(
                client.store.clone(),
                &fixture.builder(),
                environment(),
                http,
                client.clock.clone(),
                Arc::new(TestRuntime),
            )
            .unwrap();
            assert_eq!(restored.stage(), RegistrationStage::AwaitingCode);
        }
    }
    #[tokio::test]
    async fn invalid_otp_can_be_corrected_but_zero_login_cannot_be_committed() {
        {
            let fixture = Fixture::new();
            let (mut client, http) = fixture
                .client(vec![
                    Reply::Ready(br#"{"status":"fail","reason":"bad_code"}"#),
                    Reply::Ready(br#"{"status":"ok","login":"0"}"#),
                ])
                .await;
            client.await_external_code().await.unwrap();
            let builder = fixture.builder();
            let rejected = client
                .submit_code(
                    &builder,
                    "111111",
                    Some(UUID),
                    &fixture.fields,
                    EphemeralRegistrationKey::from_bytes([35; 32]),
                    POLICY,
                    &TestProof,
                )
                .await
                .unwrap();
            assert_eq!(rejected.decision, RegistrationResponseDecision::Rejected);
            assert_eq!(rejected.stage, RegistrationStage::AwaitingCode);
            assert!(matches!(
                client
                    .submit_code(
                        &builder,
                        "123456",
                        Some(UUID),
                        &fixture.fields,
                        EphemeralRegistrationKey::from_bytes([36; 32]),
                        POLICY,
                        &TestProof
                    )
                    .await,
                Err(MobileRegistrationError::MissingIdentity)
            ));
            assert!(matches!(
                client.stage(),
                RegistrationStage::OutcomeUnknown { .. }
            ));
            assert!(http.backend.load().await.unwrap().unwrap().pn.is_none());
        }
    }
    async fn gate_device_saves(
        client: &mut MobileRegistrationClient,
        backend: Arc<InMemoryBackend>,
    ) -> async_channel::Sender<()> {
        let (mut probe, _) = crate::store::release::tests::ProbeBackend::new();
        let (unpark, gate) = async_channel::bounded(2);
        probe.backend = backend;
        probe.gate = Some(gate);
        probe.gate_method = "save";
        client.store = Arc::new(PersistenceManager::new(Arc::new(probe)).await.unwrap());
        unpark
    }

    #[tokio::test]
    async fn canceled_pre_send_checkpoint_never_dispatches_and_retains_attempt_budget() {
        {
            let fixture = Fixture::new();
            let (mut client, http) = fixture.client(vec![]).await;
            let unpark = gate_device_saves(&mut client, http.backend.clone()).await;
            let builder = fixture.builder();
            let mut request = Box::pin(client.check_account(
                &builder,
                Some(UUID),
                EphemeralRegistrationKey::from_bytes([37; 32]),
                POLICY,
                &TestProof,
            ));
            assert!(futures::poll!(&mut request).is_pending());
            drop(request);
            assert_eq!(http.calls.load(Ordering::SeqCst), 0);
            assert!(client.recovery_required());
            assert!(matches!(
                client.stage(),
                RegistrationStage::OutcomeUnknown { .. }
            ));
            unpark.send(()).await.unwrap();
            unpark.send(()).await.unwrap();
            assert!(client.resume_pending().await.unwrap().is_none());
            assert_eq!(client.stage(), RegistrationStage::Ready);
            let checkpoint = serde_json::to_value(client.workflow.checkpoint()).unwrap();
            assert_eq!(checkpoint["attempts"], 1);
            assert_eq!(http.calls.load(Ordering::SeqCst), 0);
        }
    }

    #[tokio::test]
    async fn canceled_final_commit_cannot_publish_registered_until_retained_save_finishes() {
        {
            let fixture = Fixture::new();
            let (mut client, http) = fixture
                .client(vec![Reply::Ready(
                    br#"{"status":"ok","login":"99901000000"}"#,
                )])
                .await;
            client.await_external_code().await.unwrap();
            let unpark = gate_device_saves(&mut client, http.backend.clone()).await;
            unpark.send(()).await.unwrap(); // Only the pre-send checkpoint may complete.
            let builder = fixture.builder();
            let mut request = Box::pin(client.submit_code(
                &builder,
                "123456",
                Some(UUID),
                &fixture.fields,
                EphemeralRegistrationKey::from_bytes([38; 32]),
                POLICY,
                &TestProof,
            ));
            assert!(futures::poll!(&mut request).is_pending());
            drop(request);
            assert!(matches!(
                client.stage(),
                RegistrationStage::OutcomeUnknown { .. }
            ));
            assert!(http.backend.load().await.unwrap().unwrap().pn.is_none());
            assert_eq!(http.calls.load(Ordering::SeqCst), 1);
            unpark.send(()).await.unwrap();
            let result = client.resume_pending().await.unwrap().unwrap();
            assert_eq!(result.stage, RegistrationStage::Registered);
            assert!(http.backend.load().await.unwrap().unwrap().pn.is_some());
            assert_eq!(http.calls.load(Ordering::SeqCst), 1);
        }
    }
    #[tokio::test]
    async fn verified_without_login_retains_validated_durable_submitted_number() {
        let fixture = Fixture::new();
        let (mut client, http) = fixture
            .client(vec![Reply::Ready(br#"{"status":"verified"}"#)])
            .await;
        client.await_external_code().await.unwrap();
        let result = client
            .submit_code(
                &fixture.builder(),
                "123456",
                Some(UUID),
                &fixture.fields,
                EphemeralRegistrationKey::from_bytes([39; 32]),
                POLICY,
                &TestProof,
            )
            .await
            .unwrap();
        assert_eq!(result.stage, RegistrationStage::Registered);
        assert_eq!(
            http.backend.load().await.unwrap().unwrap().pn.unwrap().user,
            "99901000000"
        );
    }
    #[tokio::test]
    async fn timeout_retains_exchange_and_recovery_rejects_unrelated_publication() {
        struct ElapsedTimer;
        #[async_trait]
        impl Runtime for ElapsedTimer {
            fn spawn(&self, _: BoxFuture<'static, ()>) -> AbortHandle {
                panic!("unexpected spawn")
            }
            fn sleep(&self, _: Duration) -> BoxFuture<'static, ()> {
                Box::pin(async {})
            }
            fn spawn_blocking(
                &self,
                _: Box<dyn FnOnce() + Send + 'static>,
            ) -> BoxFuture<'static, ()> {
                panic!("unexpected I/O")
            }
            fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
                None
            }
        }
        let fixture = Fixture::new();
        let (sender, receiver) = oneshot::channel();
        let (mut client, http) = fixture.client(vec![Reply::Deferred(receiver)]).await;
        client.runtime = Arc::new(ElapsedTimer);
        let builder = fixture.builder();
        assert!(matches!(
            client
                .request_code(
                    &builder,
                    VerificationMethod::Sms,
                    Some(UUID),
                    &fixture.fields,
                    EphemeralRegistrationKey::from_bytes([40; 32]),
                    POLICY,
                    &TestProof
                )
                .await,
            Err(MobileRegistrationError::DeliveryUnknown)
        ));
        sender.send(br#"{"status":"sent"}"#).unwrap();
        assert_eq!(
            client.resume_pending().await.unwrap().unwrap().stage,
            RegistrationStage::AwaitingCode
        );
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);

        let (sender, receiver) = oneshot::channel();
        let (mut client, http) = fixture.client(vec![Reply::Deferred(receiver)]).await;
        let mut request = Box::pin(client.request_code(
            &builder,
            VerificationMethod::Sms,
            Some(UUID),
            &fixture.fields,
            EphemeralRegistrationKey::from_bytes([41; 32]),
            POLICY,
            &TestProof,
        ));
        assert!(futures::poll!(&mut request).is_pending());
        drop(request);
        let unrelated = RegistrationWorkflow::new(
            environment().binding(),
            builder.registration_identity(),
            RegistrationRunId::from_bytes([99; 32]),
            RegistrationPolicy::new(10, 1000).unwrap(),
        );
        let mut next = client.binding.clone();
        next.registration_checkpoint = Some(encode(&unrelated).unwrap());
        client
            .store
            .update_mobile_registration(client.binding.clone(), next, None)
            .await
            .unwrap();
        sender.send(br#"{"status":"sent"}"#).unwrap();
        assert!(matches!(
            client.resume_pending().await,
            Err(MobileRegistrationError::BindingMismatch)
        ));
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn retry_after_only_response_persists_cooldown_without_automatic_retry() {
        let fixture = Fixture::new();
        let (mut client, http) = fixture
            .client(vec![Reply::Ready(
                br#"{"status":"fail","reason":"too_recent","retry_after":2}"#,
            )])
            .await;
        let result = client
            .request_code(
                &fixture.builder(),
                VerificationMethod::Voice,
                Some(UUID),
                &fixture.fields,
                EphemeralRegistrationKey::from_bytes([42; 32]),
                POLICY,
                &TestProof,
            )
            .await
            .unwrap();
        assert_eq!(result.decision, RegistrationResponseDecision::RateLimited);
        assert_eq!(result.stage, RegistrationStage::Ready);
        assert!(matches!(
            client
                .request_code(
                    &fixture.builder(),
                    VerificationMethod::Voice,
                    Some(UUID),
                    &fixture.fields,
                    EphemeralRegistrationKey::from_bytes([43; 32]),
                    POLICY,
                    &TestProof
                )
                .await,
            Err(MobileRegistrationError::Workflow(
                RegistrationError::Cooldown
            ))
        ));
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
    }
}
