use clap::Parser;
use payments_system::process;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to CSV file
    path: String,
}

fn main() {
    let args = Args::parse();

    process(args.path).expect("Error processing transactions");
}
