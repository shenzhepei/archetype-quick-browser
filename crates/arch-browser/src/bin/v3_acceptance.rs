use std::{
    env, fs,
    io::{BufRead as _, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use arch_browser::{render_url, snapshot::SnapshotRenderer};
use arch_net::Loader;
use serde::Serialize;
use url::Url;
use uuid::Uuid;

const TWO_HOURS_SECONDS: u64 = 7_200;

#[derive(Debug)]
struct Options {
    root: PathBuf,
    output: PathBuf,
    duration: Duration,
    cycle_delay: Duration,
    startup_samples: usize,
}

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    generated_at_utc: String,
    git_commit: String,
    machine: Machine,
    fixture_count: usize,
    completed_cycles: u64,
    completed_page_loads: u64,
    requested_duration_seconds: u64,
    cycle_delay_milliseconds: u64,
    measured_duration_seconds: f64,
    startup_to_window_ms: Statistics,
    page_pipeline_ms: Statistics,
    reference_frame_raster_ms: Statistics,
    peak_rss_bytes: u64,
    startup_p95_under_two_seconds: bool,
    all_fixtures_completed: bool,
    two_hour_stability_completed: bool,
}

#[derive(Serialize)]
struct Machine {
    operating_system: String,
    architecture: String,
    model: String,
    processor: String,
    memory_bytes: u64,
}

#[derive(Serialize)]
struct Statistics {
    samples: usize,
    minimum: f64,
    median: f64,
    p95: f64,
    maximum: f64,
    mean: f64,
}

struct StabilityMeasurements {
    pipeline_ms: Vec<f64>,
    frame_ms: Vec<f64>,
    cycles: u64,
    peak_rss_bytes: u64,
    duration: Duration,
}

fn main() -> Result<()> {
    let options = parse_options()?;
    let fixtures = fixture_paths(&options.root)?;
    let startup = measure_startup(&options.root, options.startup_samples)?;
    let measurements = exercise_fixtures(&fixtures, options.duration, options.cycle_delay)?;
    let report = Report {
        schema_version: 1,
        generated_at_utc: command_output("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"]),
        git_commit: command_output("git", &["rev-parse", "HEAD"]),
        machine: machine(),
        fixture_count: fixtures.len(),
        completed_cycles: measurements.cycles,
        completed_page_loads: u64::try_from(measurements.pipeline_ms.len()).unwrap_or(u64::MAX),
        requested_duration_seconds: options.duration.as_secs(),
        cycle_delay_milliseconds: u64::try_from(options.cycle_delay.as_millis())
            .unwrap_or(u64::MAX),
        measured_duration_seconds: measurements.duration.as_secs_f64(),
        startup_to_window_ms: statistics(&startup),
        page_pipeline_ms: statistics(&measurements.pipeline_ms),
        reference_frame_raster_ms: statistics(&measurements.frame_ms),
        peak_rss_bytes: measurements.peak_rss_bytes,
        startup_p95_under_two_seconds: percentile(&startup, 95, 100) < 2_000.0,
        all_fixtures_completed: measurements.cycles > 0,
        two_hour_stability_completed: measurements.duration.as_secs() >= TWO_HOURS_SECONDS,
    };
    if let Some(parent) = options.output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    fs::write(&options.output, serde_json::to_vec_pretty(&report)?)
        .with_context(|| format!("could not write {}", options.output.display()))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn parse_options() -> Result<Options> {
    let mut root = env::current_dir()?;
    let mut output = PathBuf::from("artifacts/v3-acceptance.json");
    let mut duration = Duration::from_secs(60);
    let mut cycle_delay = Duration::from_secs(1);
    let mut startup_samples = 20usize;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--root" => root = PathBuf::from(next_value(&mut arguments, "--root")?),
            "--output" => output = PathBuf::from(next_value(&mut arguments, "--output")?),
            "--duration-seconds" => {
                duration =
                    Duration::from_secs(next_value(&mut arguments, "--duration-seconds")?.parse()?);
            }
            "--cycle-delay-milliseconds" => {
                cycle_delay = Duration::from_millis(
                    next_value(&mut arguments, "--cycle-delay-milliseconds")?.parse()?,
                );
            }
            "--startup-samples" => {
                startup_samples = next_value(&mut arguments, "--startup-samples")?.parse()?;
            }
            _ => bail!("unknown argument: {argument}"),
        }
    }
    if startup_samples == 0 {
        bail!("--startup-samples must be greater than zero");
    }
    Ok(Options {
        root: root
            .canonicalize()
            .context("could not resolve repository root")?,
        output,
        duration,
        cycle_delay,
        startup_samples,
    })
}

fn next_value(arguments: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    arguments
        .next()
        .with_context(|| format!("missing value for {flag}"))
}

fn fixture_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let mut fixtures = fs::read_dir(root.join("fixtures/pages"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("index.html"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    fixtures.sort();
    if fixtures.len() != 30 {
        bail!("expected 30 fixture pages, found {}", fixtures.len());
    }
    Ok(fixtures)
}

fn measure_startup(root: &Path, samples: usize) -> Result<Vec<f64>> {
    let executable = env::current_exe()?
        .parent()
        .context("acceptance executable has no parent directory")?
        .join("arch-browser");
    if !executable.is_file() {
        bail!(
            "{} is missing; build all release binaries first",
            executable.display()
        );
    }
    let profile_root = env::temp_dir().join(format!("archetype-startup-{}", Uuid::now_v7()));
    fs::create_dir_all(&profile_root)?;
    let mut durations = Vec::with_capacity(samples);
    for sample in 0..samples {
        let profile = profile_root.join(sample.to_string());
        let started = Instant::now();
        let mut child = Command::new(&executable)
            .current_dir(root)
            .env("ARCHETYPE_DATA_DIR", &profile)
            .env("ARCHETYPE_STARTUP_PROBE", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("could not launch desktop startup probe")?;
        let stdout = child.stdout.take().context("startup probe has no stdout")?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
            let _ = sender.send(result);
        });
        let ready = match receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(result) => result?,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error).context("desktop startup probe timed out");
            }
        };
        let elapsed = started.elapsed();
        let _ = child.kill();
        let _ = child.wait();
        if ready.trim() != "ARCHETYPE_READY" {
            bail!("desktop startup probe exited before its window became ready");
        }
        durations.push(elapsed.as_secs_f64() * 1_000.0);
    }
    fs::remove_dir_all(profile_root)?;
    Ok(durations)
}

fn exercise_fixtures(
    fixtures: &[PathBuf],
    requested_duration: Duration,
    cycle_delay: Duration,
) -> Result<StabilityMeasurements> {
    let loader = Loader::new()?;
    let mut renderer = SnapshotRenderer::default();
    let started = Instant::now();
    let mut pipeline = Vec::new();
    let mut frame = Vec::new();
    let mut cycles = 0u64;
    let mut peak_rss = resident_bytes();
    loop {
        for path in fixtures {
            let url = Url::from_file_path(path)
                .map_err(|()| anyhow::anyhow!("invalid fixture path: {}", path.display()))?;
            let load_started = Instant::now();
            let page = render_url(&loader, &url, 1280.0)
                .with_context(|| format!("could not render {}", path.display()))?;
            pipeline.push(load_started.elapsed().as_secs_f64() * 1_000.0);
            let frame_started = Instant::now();
            let image = renderer.render(&page);
            frame.push(frame_started.elapsed().as_secs_f64() * 1_000.0);
            if image.width() != 1280 || image.height() != 800 {
                bail!("fixture frame has unexpected dimensions");
            }
        }
        cycles = cycles.saturating_add(1);
        peak_rss = peak_rss.max(resident_bytes());
        let elapsed = started.elapsed();
        if elapsed >= requested_duration && cycles > 0 {
            break;
        }
        thread::sleep(cycle_delay.min(requested_duration.saturating_sub(elapsed)));
    }
    Ok(StabilityMeasurements {
        pipeline_ms: pipeline,
        frame_ms: frame,
        cycles,
        peak_rss_bytes: peak_rss,
        duration: started.elapsed(),
    })
}

fn resident_bytes() -> u64 {
    command_output("ps", &["-o", "rss=", "-p", &std::process::id().to_string()])
        .parse::<u64>()
        .unwrap_or_default()
        .saturating_mul(1_024)
}

fn machine() -> Machine {
    Machine {
        operating_system: format!("macOS {}", command_output("sw_vers", &["-productVersion"])),
        architecture: env::consts::ARCH.to_owned(),
        model: command_output("sysctl", &["-n", "hw.model"]),
        processor: command_output("sysctl", &["-n", "machdep.cpu.brand_string"]),
        memory_bytes: command_output("sysctl", &["-n", "hw.memsize"])
            .parse()
            .unwrap_or_default(),
    }
}

fn command_output(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_default()
}

fn statistics(values: &[f64]) -> Statistics {
    let sum = values.iter().sum::<f64>();
    Statistics {
        samples: values.len(),
        minimum: values.iter().copied().reduce(f64::min).unwrap_or_default(),
        median: percentile(values, 50, 100),
        p95: percentile(values, 95, 100),
        maximum: values.iter().copied().reduce(f64::max).unwrap_or_default(),
        mean: if values.is_empty() {
            0.0
        } else {
            sum / f64::from(u32::try_from(values.len()).unwrap_or(u32::MAX))
        },
    }
}

fn percentile(values: &[f64], numerator: usize, denominator: usize) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let scaled = (sorted.len() - 1).saturating_mul(numerator);
    let index = scaled.saturating_add(denominator - 1) / denominator;
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_report_nearest_rank_percentiles() {
        let values = (1..=20).map(f64::from).collect::<Vec<_>>();
        let summary = statistics(&values);
        assert_eq!(summary.samples, 20);
        assert!((summary.median - 11.0).abs() < f64::EPSILON);
        assert!((summary.p95 - 20.0).abs() < f64::EPSILON);
        assert!((summary.mean - 10.5).abs() < f64::EPSILON);
    }
}
