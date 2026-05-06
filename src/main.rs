use prec_bsl::cli::{self, CliCommand, HelpTopic};

fn main() {
    let exit_code = match cli::parse_env() {
        Ok(CliCommand::Help(topic)) => {
            println!("{}", cli::help(topic));
            0
        }
        Ok(CliCommand::PrekHook(_args)) => {
            println!("prec-bsl prek-hook: command contract accepted");
            0
        }
        Ok(CliCommand::ExecRules(_args)) => {
            println!("prec-bsl exec-rules: command contract accepted");
            0
        }
        Err(error) => {
            eprintln!("error: {}", error.message());
            eprintln!();
            eprintln!("{}", cli::help(HelpTopic::General));
            2
        }
    };

    std::process::exit(exit_code);
}
