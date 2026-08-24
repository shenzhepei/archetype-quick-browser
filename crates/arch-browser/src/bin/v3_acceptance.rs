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

const MINIMUM_STABILITY_SECONDS: u64 = 60;
const MAXIMUM_CPU_COST_GROWTH_RATIO: f64 = 1.5;
const MAXIMUM_RSS_GROWTH_BYTES: i64 = 16 * 1024 * 1024;

#[derive(Debug)]
struct Options {
    root: PathBuf,
    output: PathBuf,
    duration: Duration,
    cycle_delay: Duration,
    startup_samples: usize,
    expected_fixtures: usize,
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
    process_cpu_seconds: f64,
    average_cpu_percent: f64,
    first_half_cpu_seconds_per_page: f64,
    second_half_cpu_seconds_per_page: f64,
    cpu_cost_growth_ratio: f64,
    initial_rss_bytes: u64,
    final_rss_bytes: u64,
    rss_growth_bytes: i64,
    peak_rss_bytes: u64,
    acceptance: Acceptance,
}

#[derive(Clone, Copy, Serialize)]
struct Acceptance {
    startup_p95_under_two_seconds: Outcome,
    all_fixtures_completed: Outcome,
    one_minute_stability_completed: Outcome,
    resource_growth_within_limits: Outcome,
}

impl Acceptance {
    const fn passed(self) -> bool {
        self.startup_p95_under_two_seconds.passed()
            && self.all_fixtures_completed.passed()
            && self.one_minute_stability_completed.passed()
            && self.resource_growth_within_limits.passed()
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Pass,
    Fail,
}

impl Outcome {
    const fn passed(self) -> bool {
        matches!(self, Self::Pass)
    }
}

impl From<bool> for Outcome {
    fn from(value: bool) -> Self {
        if value { Self::Pass } else { Self::Fail }
    }
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
    cycle_cpu_seconds: Vec<f64>,
    initial_rss_bytes: u64,
    final_rss_bytes: u64,
    peak_rss_bytes: u64,
    duration: Duration,
}

fn main() -> Result<()> {
    let options = parse_options()?;
    let fixtures = fixture_paths(&options.root, options.expected_fixtures)?;
    let startup = measure_startup(&options.root, options.startup_samples)?;
    let measurements = exercise_fixtures(&fixtures, options.duration, options.cycle_delay)?;
    let first_half_cpu_seconds_per_page =
        half_cpu_seconds_per_page(&measurements.cycle_cpu_seconds, fixtures.len(), false);
    let second_half_cpu_seconds_per_page =
        half_cpu_seconds_per_page(&measurements.cycle_cpu_seconds, fixtures.len(), true);
    let cpu_cost_growth_ratio = if first_half_cpu_seconds_per_page > 0.0 {
        second_half_cpu_seconds_per_page / first_half_cpu_seconds_per_page
    } else {
        0.0
    };
    let rss_growth_bytes =
        signed_difference(measurements.final_rss_bytes, measurements.initial_rss_bytes);
    let process_cpu_seconds = measurements.cycle_cpu_seconds.iter().sum::<f64>();
    let one_minute_stability_completed =
        measurements.duration.as_secs() >= MINIMUM_STABILITY_SECONDS;
    let resource_growth_within_limits = cpu_cost_growth_ratio <= MAXIMUM_CPU_COST_GROWTH_RATIO
        && rss_growth_bytes <= MAXIMUM_RSS_GROWTH_BYTES;
    let acceptance = Acceptance {
        startup_p95_under_two_seconds: (percentile(&startup, 95, 100) < 2_000.0).into(),
        all_fixtures_completed: (measurements.cycles > 0).into(),
        one_minute_stability_completed: one_minute_stability_completed.into(),
        resource_growth_within_limits: resource_growth_within_limits.into(),
    };
    let report = Report {
        schema_version: 2,
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
        process_cpu_seconds,
        average_cpu_percent: process_cpu_seconds / measurements.duration.as_secs_f64() * 100.0,
        first_half_cpu_seconds_per_page,
        second_half_cpu_seconds_per_page,
        cpu_cost_growth_ratio,
        initial_rss_bytes: measurements.initial_rss_bytes,
        final_rss_bytes: measurements.final_rss_bytes,
        rss_growth_bytes,
        peak_rss_bytes: measurements.peak_rss_bytes,
        acceptance,
    };
    if let Some(parent) = options.output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    fs::write(&options.output, serde_json::to_vec_pretty(&report)?)
        .with_context(|| format!("could not write {}", options.output.display()))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.acceptance.passed() {
        bail!("acceptance thresholds were not met");
    }
    Ok(())
}

fn parse_options() -> Result<Options> {
    let mut root = env::current_dir()?;
    let mut output = PathBuf::from("artifacts/v3-acceptance.json");
    let mut duration = Duration::from_secs(60);
    let mut cycle_delay = Duration::from_secs(1);
    let mut startup_samples = 20usize;
    let mut expected_fixtures = 30usize;
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
            "--expected-fixtures" => {
                expected_fixtures = next_value(&mut arguments, "--expected-fixtures")?.parse()?;
            }
            _ => bail!("unknown argument: {argument}"),
        }
    }
    if startup_samples == 0 || expected_fixtures == 0 {
        bail!("sample and fixture counts must be greater than zero");
    }
    Ok(Options {
        root: root
            .canonicalize()
            .context("could not resolve repository root")?,
        output,
        duration,
        cycle_delay,
        startup_samples,
        expected_fixtures,
    })
}

fn next_value(arguments: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    arguments
        .next()
        .with_context(|| format!("missing value for {flag}"))
}

fn fixture_paths(root: &Path, expected_fixtures: usize) -> Result<Vec<PathBuf>> {
    let mut fixtures = fs::read_dir(root.join("fixtures/pages"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("index.html"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    fixtures.sort();
    if fixtures.len() != expected_fixtures {
        bail!(
            "expected {expected_fixtures} fixture pages, found {}",
            fixtures.len()
        );
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
    let mut cycle_cpu_seconds = Vec::new();
    let mut previous_cpu_seconds = process_cpu_seconds();
    let mut rss_samples = Vec::new();
    let mut peak_rss = 0;
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
        let current_cpu_seconds = process_cpu_seconds();
        cycle_cpu_seconds.push((current_cpu_seconds - previous_cpu_seconds).max(0.0));
        previous_cpu_seconds = current_cpu_seconds;
        let current_rss = resident_bytes();
        rss_samples.push(current_rss);
        peak_rss = peak_rss.max(current_rss);
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
        cycle_cpu_seconds,
        initial_rss_bytes: rss_samples.first().copied().unwrap_or_default(),
        final_rss_bytes: rss_samples.last().copied().unwrap_or_default(),
        peak_rss_bytes: peak_rss,
        duration: started.elapsed(),
    })
}

fn process_cpu_seconds() -> f64 {
    parse_process_time(&command_output(
        "ps",
        &["-o", "time=", "-p", &std::process::id().to_string()],
    ))
    .unwrap_or_default()
}

fn parse_process_time(value: &str) -> Option<f64> {
    let (days, time) = value
        .trim()
        .split_once('-')
        .map_or((0.0, value.trim()), |(days, time)| {
            (days.parse::<f64>().ok().unwrap_or_default(), time)
        });
    let parts = time.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => (
            0.0,
            minutes.parse::<f64>().ok()?,
            seconds.parse::<f64>().ok()?,
        ),
        [hours, minutes, seconds] => (
            hours.parse::<f64>().ok()?,
            minutes.parse::<f64>().ok()?,
            seconds.parse::<f64>().ok()?,
        ),
        _ => return None,
    };
    Some(days * 86_400.0 + hours * 3_600.0 + minutes * 60.0 + seconds)
}

fn half_cpu_seconds_per_page(values: &[f64], fixture_count: usize, second_half: bool) -> f64 {
    if values.is_empty() || fixture_count == 0 {
        return 0.0;
    }
    let split = values.len().div_ceil(2);
    let half = if second_half {
        &values[split..]
    } else {
        &values[..split]
    };
    if half.is_empty() {
        return 0.0;
    }
    half.iter().sum::<f64>()
        / f64::from(u32::try_from(half.len() * fixture_count).unwrap_or(u32::MAX))
}

fn signed_difference(final_value: u64, initial_value: u64) -> i64 {
    let difference = i128::from(final_value) - i128::from(initial_value);
    i64::try_from(difference).unwrap_or(if difference.is_negative() {
        i64::MIN
    } else {
        i64::MAX
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

    #[test]
    fn parses_process_cpu_time_and_compares_halves() {
        assert_eq!(parse_process_time("1:02.50"), Some(62.5));
        assert_eq!(parse_process_time("2:03:04.25"), Some(7_384.25));
        assert_eq!(parse_process_time("1-02:03:04.25"), Some(93_784.25));
        assert!(parse_process_time("invalid").is_none());

        let samples = [3.0, 3.0, 4.5, 4.5];
        assert!((half_cpu_seconds_per_page(&samples, 30, false) - 0.1).abs() < f64::EPSILON);
        assert!((half_cpu_seconds_per_page(&samples, 30, true) - 0.15).abs() < f64::EPSILON);
    }
}
