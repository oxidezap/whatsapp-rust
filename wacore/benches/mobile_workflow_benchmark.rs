//! Synthetic CPU/allocation baselines for the portable registration policy.
//!
//! These rows perform no network, storage, SMS or APK execution. Input setup is
//! outside each timed operation; restore rows measure context validation and
//! hashing, while the acknowledgement row measures only the local publication.

use divan::{Bencher, black_box};
use wacore::mobile::{
    EnvironmentBinding, MobileEnvironment, MobileTrust, RegistrationCheckpoint,
    RegistrationEndpoint, RegistrationIdentity, RegistrationOperation, RegistrationOutcome,
    RegistrationPolicy, RegistrationRunId, RegistrationWorkflow, TcpEndpoint,
};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

struct Fixture {
    environment: EnvironmentBinding,
    identity: RegistrationIdentity,
    run_id: RegistrationRunId,
    policy: RegistrationPolicy,
}

impl Fixture {
    fn new() -> Self {
        let environment = MobileEnvironment::new(
            "synthetic-policy-benchmark",
            RegistrationEndpoint::new("https://registration.example.invalid/v2")
                .expect("synthetic endpoint"),
            TcpEndpoint::new("mobile.example.invalid", 443).expect("synthetic destination"),
            MobileTrust::new([1; 32], [2; 32]).expect("synthetic public trust"),
        )
        .expect("synthetic environment");
        Self {
            environment: environment.binding(),
            identity: RegistrationIdentity::from_public_keys(&[3; 32], &[4; 32]),
            run_id: RegistrationRunId::from_bytes([5; 32]),
            policy: RegistrationPolicy::new(4, 1_000).expect("bounded policy"),
        }
    }

    fn workflow(&self) -> RegistrationWorkflow {
        RegistrationWorkflow::new(self.environment, self.identity, self.run_id, self.policy)
    }

    fn verified(&self) -> RegistrationWorkflow {
        let mut workflow = self.workflow();
        let token = workflow
            .begin(RegistrationOperation::RequestCode, 10)
            .expect("reserve synthetic attempt");
        workflow
            .complete(token, RegistrationOutcome::Verified, 20)
            .expect("supplied synthetic response");
        workflow
    }

    fn restore(&self, checkpoint: RegistrationCheckpoint) -> RegistrationWorkflow {
        RegistrationWorkflow::restore(checkpoint, self.environment, self.identity)
            .expect("matching synthetic checkpoint")
    }
}

#[divan::bench]
fn policy_transitions(bencher: Bencher) {
    let fixture = Fixture::new();
    bencher
        .with_inputs(|| fixture.workflow())
        .bench_values(|workflow| {
            let mut workflow = black_box(workflow);
            let request = workflow
                .begin(black_box(RegistrationOperation::RequestCode), black_box(10))
                .expect("reserve request");
            workflow
                .complete(
                    black_box(request),
                    black_box(RegistrationOutcome::NeedsCode),
                    black_box(20),
                )
                .expect("supplied code requirement");
            let submission = workflow
                .begin(black_box(RegistrationOperation::SubmitCode), black_box(30))
                .expect("reserve submission");
            workflow
                .complete(
                    black_box(submission),
                    black_box(RegistrationOutcome::Verified),
                    black_box(40),
                )
                .expect("supplied verification");
            let commit = workflow.pending_persistence().expect("verified checkpoint");
            workflow
                .confirm_persisted(black_box(commit))
                .expect("synthetic host acknowledgement");
            black_box(workflow)
        });
}

#[divan::bench]
fn restore_inflight_checkpoint(bencher: Bencher) {
    let fixture = Fixture::new();
    let mut workflow = fixture.workflow();
    workflow
        .begin(RegistrationOperation::RequestCode, 10)
        .expect("reserve synthetic attempt");
    let checkpoint = workflow.checkpoint();
    bencher
        .with_inputs(|| checkpoint.clone())
        .bench_values(|checkpoint| black_box(fixture.restore(black_box(checkpoint))));
}

#[divan::bench]
fn restore_verified_checkpoint(bencher: Bencher) {
    let fixture = Fixture::new();
    let checkpoint = fixture.verified().checkpoint();
    bencher
        .with_inputs(|| checkpoint.clone())
        .bench_values(|checkpoint| black_box(fixture.restore(black_box(checkpoint))));
}

#[divan::bench]
fn confirm_durable_checkpoint(bencher: Bencher) {
    let fixture = Fixture::new();
    bencher
        .with_inputs(|| {
            let workflow = fixture.verified();
            let token = workflow.pending_persistence().expect("verified checkpoint");
            (workflow, token)
        })
        .bench_values(|(workflow, token)| {
            let mut workflow = black_box(workflow);
            workflow
                .confirm_persisted(black_box(token))
                .expect("synthetic host acknowledgement");
            black_box(workflow)
        });
}
