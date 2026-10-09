use std::{
    collections::BTreeMap,
    error::Error,
    path::{Path, PathBuf},
};
fn profiles(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            profiles(&path, out)?;
        } else if path.extension().is_some_and(|x| x == "out") {
            out.push(path);
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let mut paths = Vec::new();
    profiles(Path::new(&args[1]), &mut paths)?;
    paths.sort();
    let mut matched = 0usize;
    for path in paths {
        let contents = std::fs::read_to_string(path)?;
        for part in contents.split("part:").skip(1) {
            let Some(name) = part
                .lines()
                .find_map(|x| x.strip_prefix("desc: Trigger: Client Request: "))
            else {
                continue;
            };
            if !name.contains(&args[2]) {
                continue;
            }
            matched += 1;
            let events = part
                .lines()
                .find_map(|x| x.strip_prefix("events: "))
                .ok_or("events")?;
            let index = events
                .split_whitespace()
                .position(|x| x == "Ir")
                .ok_or("Ir")?;
            let total: u64 = part
                .lines()
                .find_map(|x| x.strip_prefix("totals:"))
                .ok_or("totals")?
                .split_whitespace()
                .nth(index)
                .ok_or("total Ir")?
                .parse()?;
            let mut own: BTreeMap<String, u64> = BTreeMap::new();
            let mut callees: BTreeMap<String, (u64, u64)> = BTreeMap::new();
            let (mut function, mut callee, mut calls) = (String::new(), String::new(), None);
            for line in part.lines() {
                if let Some(name) = line.strip_prefix("fn=") {
                    function = name.into();
                } else if let Some(name) = line.strip_prefix("cfn=") {
                    callee = name.into();
                } else if let Some(value) = line.strip_prefix("calls=") {
                    calls = Some(
                        value
                            .split_whitespace()
                            .next()
                            .ok_or("calls")?
                            .parse::<u64>()?,
                    );
                } else if line
                    .starts_with(|c: char| c.is_ascii_digit() || matches!(c, '+' | '-' | '*'))
                {
                    let cost = line
                        .split_whitespace()
                        .nth(index + 1)
                        .ok_or("cost")?
                        .parse::<u64>()?;
                    if let Some(count) = calls.take() {
                        let entry = callees.entry(callee.clone()).or_default();
                        entry.0 += count;
                        entry.1 += cost;
                    } else {
                        *own.entry(function.clone()).or_default() += cost;
                    }
                }
            }
            let sum: u64 = own.values().sum();
            if sum != total {
                return Err(format!("self cost {sum} differs from total {total}").into());
            }
            println!(
                "PROFILE {}\n{name}: {total} Ir\nSelf instructions:",
                path.display()
            );
            let mut rows: Vec<_> = own.into_iter().collect();
            rows.sort_unstable_by_key(|(_, n)| std::cmp::Reverse(*n));
            for (name, cost) in rows.iter().take(30) {
                println!("{cost}\t{name}");
            }
            println!("Called functions: calls / inclusive instructions (overlapping):");
            let mut rows: Vec<_> = callees.into_iter().collect();
            rows.sort_unstable_by_key(|(_, (_, n))| std::cmp::Reverse(*n));
            for (name, (calls, cost)) in rows.iter().take(20) {
                println!("{calls}\t{cost}\t{name}");
            }
        }
    }
    if matched == 0 {
        return Err("no matching instrumented profiles".into());
    }
    println!("Validated {matched} matching profiles: self instructions equal each captured total.");
    Ok(())
}
