use wilayah::pipeline::Pipeline;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut pipeline = Pipeline::new();

    // Parse simple flags (not using clap for minimalism)
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    pipeline = pipeline.output(std::path::Path::new(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("error: --output requires a path");
                    std::process::exit(1);
                }
            }
            "--cache-dir" => {
                if i + 1 < args.len() {
                    pipeline = pipeline.cache_dir(std::path::Path::new(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("error: --cache-dir requires a path");
                    std::process::exit(1);
                }
            }
            "--pdf-url" => {
                if i + 1 < args.len() {
                    pipeline = pipeline.pdf_url(&args[i + 1]);
                    i += 2;
                } else {
                    eprintln!("error: --pdf-url requires a URL");
                    std::process::exit(1);
                }
            }
            "--big-api-url" => {
                if i + 1 < args.len() {
                    pipeline = pipeline.big_api_url(&args[i + 1]);
                    i += 2;
                } else {
                    eprintln!("error: --big-api-url requires a URL");
                    std::process::exit(1);
                }
            }
            "--force-refresh-big" => {
                pipeline = pipeline.force_refresh_big(true);
                i += 1;
            }
            _ => {
                eprintln!("warning: unknown argument: {}", args[i]);
                i += 1;
            }
        }
    }

    match pipeline.run() {
        Ok(output) => {
            println!("Pipeline completed successfully.");
            println!("Database: {}", output.db_path.display());
            println!("Villages: {}", output.village_count);
            println!("SHA-256: {}", output.sha256);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Pipeline failed: {}", e);
            std::process::exit(1);
        }
    }
}
