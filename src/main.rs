mod cli;
#[cfg(feature = "gui")]
mod gui;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        #[cfg(feature = "gui")]
        {
            if let Err(e) = gui::run() {
                eprintln!("{e}");
                std::process::exit(1);
            }
            return;
        }
        #[cfg(not(feature = "gui"))]
        {
            print!("{}", cli::USAGE);
            return;
        }
    }
    if let Err(e) = cli::run(args) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
