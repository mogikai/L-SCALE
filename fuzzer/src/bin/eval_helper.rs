use clap::{
    Args,
    CommandFactory,
    Parser,
    Subcommand,
};
use clap_complete::{
    Shell,
    generate,
};
use std::error::Error;
use std::fs;
use std::path::{
    Path,
    PathBuf,
};
use std::process::Command;
use std::thread::sleep;
use std::time::{
    Duration,
    Instant,
};

fn find_dot_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut dot_files = Vec::new();

    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                dot_files.extend(find_dot_files(&path)?);
            } else if let Some(ext) = path.extension() {
                if ext == "dot" {
                    dot_files.push(path);
                }
            }
        }
    }

    Ok(dot_files)
}

fn convert_to_pdf(
    dot_file: &Path,
    timeout: Duration,
) -> Result<(), Box<dyn Error>> {
    let pdf_path = dot_file.with_extension("pdf");

    println!("Converting {:?} to {:?}", dot_file, pdf_path);
    let mut child = Command::new("dot")
        .arg("-Tpdf")
        .arg(dot_file)
        .arg("-o")
        .arg(&pdf_path)
        .spawn()
        .expect("Failed to execute dot command");
    let start_time = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            if status.success() {
                println!("Converted {:?} to {:?}", dot_file, pdf_path);
                break Ok(());
            } else {
                break Err(format!("Dot command failed with status {:?}", status).into());
            }
        }

        if start_time.elapsed() > timeout {
            child.kill().expect("Failed to kill dot command");
            break Err("Timeout for dot command".to_string().into());
        }
        sleep(Duration::from_micros(100));
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None, name = "eval_helper")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate pdf files for dots
    GeneratePdfs(GenerateArgs),
    Completions {
        #[arg(long)]
        shell: Shell,
    },
}

#[derive(Args, Debug)]
struct GenerateArgs {
    /// The root directory in which to search for dot files.
    /// The pdfs are generated next to the dot files found.
    #[arg[long]]
    root_dir: PathBuf,

    /// The timeout for individual dot commands. If the building of the pdf takes longer than this
    /// time it is terminated.
    #[arg[long, default_value = "30"]]
    timeout: u64,
}

fn main() {
    match Cli::parse().command {
        Commands::GeneratePdfs(args) => match find_dot_files(&args.root_dir) {
            Ok(dot_files) => {
                for dot_file in dot_files {
                    if let Err(e) = convert_to_pdf(&dot_file, Duration::from_secs(args.timeout)) {
                        eprintln!("Error converting {:?}: {}", dot_file, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error finding .dot files: {}", e);
                std::process::exit(1);
            }
        },
        Commands::Completions { shell } => {
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            generate(shell, &mut command, name, &mut std::io::stdout());
        }
    }
}
