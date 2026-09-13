use anyhow::{Context, Result};
use std::{path::PathBuf, sync::atomic::AtomicBool};
use subtake_native::{
    export::{self, ExportSettings},
    media,
    project::Project,
    render::Scene,
};
slint::include_modules!();
mod app;
mod inspector;
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("benchmark") => {
            let path = PathBuf::from(
                args.get(1)
                    .context("benchmark PROJECT [WIDTH HEIGHT FRAMES]")?,
            );
            let mut p = Project::load(&path)?;
            p.resolve_assets(&path);
            let source = p.source_path(Some(&path));
            let info = media::probe(&source)?;
            let width = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(960);
            let height = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(540);
            let frames: usize = args.get(4).map(|s| s.parse()).transpose()?.unwrap_or(90);
            anyhow::ensure!((2..=600).contains(&frames), "Frames must be 2–600");
            let start = std::time::Instant::now();
            let mut scene = Scene::new(source, info.clone(), width, height)?;
            scene.render(&p, 0.)?;
            let first = start.elapsed().as_secs_f64() * 1000.;
            let mut times = vec![];
            for i in 1..frames {
                let start = std::time::Instant::now();
                scene.render(&p, (i as f64 / 30.).min((info.duration - 0.05).max(0.)))?;
                times.push(start.elapsed().as_secs_f64() * 1000.);
            }
            let total = times.iter().sum::<f64>();
            times.sort_by(f64::total_cmp);
            let mut seeks = vec![];
            for fraction in [0.8, 0.1, 0.65, 0.25, 0.9, 0.4, 0.05, 0.7, 0.2, 0.6] {
                let start = std::time::Instant::now();
                scene.render(&p, info.duration * fraction)?;
                seeks.push(start.elapsed().as_secs_f64() * 1000.);
            }
            seeks.sort_by(f64::total_cmp);
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"backend":scene.backend(),"source":info,"output":[width,height],"frames":frames,"first_frame_ms":first,"sequential_p50_ms":times[times.len()/2],"sequential_p95_ms":times[(times.len()*95/100).min(times.len()-1)],"sequential_fps":(frames-1) as f64*1000./total,"seek_p50_ms":seeks[5],"seek_p95_ms":seeks[9],"pipeline":"decode + native composition + RGBA readback; excludes UI presentation and audio","electron_comparison":false})
                )?
            );
        }
        Some("transcribe") => {
            let source = PathBuf::from(args.get(1).context("transcribe VIDEO MODEL [LANGUAGE]")?);
            let model = PathBuf::from(args.get(2).context("Missing model")?);
            let cues = subtake_native::transcription::transcribe(
                &source,
                &media::binary("whisper-cli")?,
                &model,
                args.get(3).map(String::as_str).unwrap_or("auto"),
                &AtomicBool::new(false),
                |s| eprintln!("{s}"),
            )?;
            println!("{}", serde_json::to_string_pretty(&cues)?);
        }
        Some("download-model") => {
            let path = subtake_native::models::download(&AtomicBool::new(false), |progress| {
                eprintln!("{:.0}%", progress * 100.)
            })?;
            println!("{}", path.display());
        }
        Some("probe") => {
            let info = media::probe(&PathBuf::from(args.get(1).context("probe VIDEO")?))?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        Some("validate") => {
            let project = Project::load(&PathBuf::from(args.get(1).context("validate PROJECT")?))?;
            project.validate()?;
            println!("Valid project, version {}", project.version);
        }
        Some("render") => {
            let path = PathBuf::from(
                args.get(1)
                    .context("render PROJECT OUTPUT.png TIME [WIDTH HEIGHT]")?,
            );
            let mut p = Project::load(&path)?;
            p.resolve_assets(&path);
            let source = p.source_path(Some(&path));
            let info = media::probe(&source)?;
            let time = args.get(3).context("Missing time")?.parse()?;
            let w = args.get(4).map(|v| v.parse()).transpose()?.unwrap_or(960);
            let h = args.get(5).map(|v| v.parse()).transpose()?.unwrap_or(540);
            let pixels = Scene::new(source, info, w, h)?.render(&p, time)?;
            image::save_buffer(
                args.get(2).context("Missing output")?,
                &pixels,
                w,
                h,
                image::ColorType::Rgba8,
            )?;
        }
        Some("export") => {
            let path = PathBuf::from(
                args.get(1)
                    .context("export PROJECT OUTPUT [WIDTH HEIGHT FPS]")?,
            );
            let mut p = Project::load(&path)?;
            p.resolve_assets(&path);
            let source = p.source_path(Some(&path));
            let output = PathBuf::from(args.get(2).context("Missing output")?);
            let defaults = ExportSettings::for_media(&p, &media::probe(&source)?);
            let settings = ExportSettings {
                width: args
                    .get(3)
                    .map(|v| v.parse())
                    .transpose()?
                    .unwrap_or(defaults.width),
                height: args
                    .get(4)
                    .map(|v| v.parse())
                    .transpose()?
                    .unwrap_or(defaults.height),
                fps: args
                    .get(5)
                    .map(|v| v.parse())
                    .transpose()?
                    .unwrap_or(defaults.fps),
                gif: output.extension().is_some_and(|e| e == "gif"),
                ..defaults
            };
            export::export(
                &p,
                &source,
                &settings,
                &output,
                &AtomicBool::new(false),
                |v| eprintln!("{:.0}%", v * 100.),
            )?;
        }
        Some("sources") => println!(
            "{}",
            serde_json::to_string_pretty(&subtake_native::platform::sources()?)?
        ),
        Some("--help") => println!(
            "SubTake native\n  subtake-native [VIDEO|PROJECT]\n  subtake-native probe VIDEO\n  subtake-native validate PROJECT\n  subtake-native render PROJECT OUTPUT.png TIME [WIDTH HEIGHT]\n  subtake-native export PROJECT OUTPUT [WIDTH HEIGHT FPS]\n  subtake-native benchmark PROJECT [WIDTH HEIGHT FRAMES]\n  subtake-native sources\n  subtake-native download-model\n  subtake-native transcribe VIDEO MODEL [LANGUAGE]"
        ),
        _ => app::run(args.first().map(PathBuf::from))?,
    }
    Ok(())
}
