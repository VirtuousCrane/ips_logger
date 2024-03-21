use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// MQTT Host URL
    #[arg(long)]
    host: String,
    
    /// MQTT Port
    #[arg(short, long, default_value_t = 1883)]
    port: u32,

    /// MQTT Subscribe Topic
    #[arg(short, long)]
    topic: String,

    #[arg(short, long)]
    verbose: bool,

    /// Output file name
    #[arg(default_value_t = String::from("output.csv"))]
    output: String,
}

fn main() {
    let args = Args::parse();
    
    println!("Host: {:?}", args.host);
    println!("Port: {:?}", args.port);
    println!("Topic: {:?}", args.topic);
    println!("Verbose: {:?}", args.verbose);
    println!("Output: {:?}", args.output)
}
