use anyhow::Result;
use std::path::Path;

pub fn run(root: &Path, slow: bool) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    super::derive_mlow::fetch(root, None)?;
    xtask_support::run(
        std::process::Command::new(&cargo)
            .args(["run", "-p", "whatspec-codegen", "--", "--check"])
            .current_dir(root),
    )?;
    xtask_support::run(
        std::process::Command::new(&cargo)
            .args([
                "test",
                "--release",
                "--locked",
                "-p",
                "oracle-core",
                "--lib",
                "--tests",
                "--",
                "--nocapture",
            ])
            .current_dir(root),
    )?;
    super::mlow::run(
        root,
        super::mlow::Task::Regenerate {
            out: None,
            from_derived: None,
            check: true,
        },
    )?;
    for filter in ["voip::", "iq::"] {
        xtask_support::run(
            std::process::Command::new(&cargo)
                .args([
                    "test",
                    "--locked",
                    "-p",
                    "wacore",
                    "--features",
                    "voip-mlow",
                    "--lib",
                    filter,
                ])
                .current_dir(root),
        )?;
    }
    if slow {
        for target in ["signaling", "video_offer"] {
            xtask_support::run(
                std::process::Command::new(&cargo)
                    .args([
                        "test",
                        "--release",
                        "--locked",
                        "-p",
                        "oracle-core",
                        "--test",
                        target,
                        "--",
                        "--ignored",
                        "--nocapture",
                        "--test-threads=1",
                    ])
                    .current_dir(root),
            )?;
        }
    }
    println!("VoIP conformance gates passed");
    Ok(())
}
