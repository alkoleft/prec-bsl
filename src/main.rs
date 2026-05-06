use prec_bsl::cli::{self, CliCommand, HelpTopic};

fn main() {
    let exit_code = match cli::parse_env() {
        Ok(CliCommand::Help(topic)) => {
            println!("{}", cli::help(topic));
            0
        }
        Ok(CliCommand::PrekHook(args)) => prec_bsl::app::run_prek_hook(&args),
        Ok(CliCommand::ExecRules(args)) => prec_bsl::app::run_exec_rules(&args),
        Err(error) => {
            eprintln!("error: {}", error.message());
            eprintln!();
            eprintln!("{}", cli::help(HelpTopic::General));
            2
        }
    };

    std::process::exit(exit_code);
}
