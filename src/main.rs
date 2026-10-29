use prec_bsl::cli::{self, CliCommand, HelpTopic};

fn main() {
    let exit_code = match cli::parse_env() {
        Ok(CliCommand::Help(topic)) => {
            println!("{}", cli::help(topic));
            0
        }
        Ok(CliCommand::PrekHook(args)) => prec_bsl::app::run_prek_hook(&args),
        Ok(CliCommand::ExecRules(args)) => prec_bsl::app::run_exec_rules(&args),
        Ok(CliCommand::Check(args)) => prec_bsl::app::run_check(&args),
        Ok(CliCommand::Fix(args)) => prec_bsl::app::run_fix(&args),
        Err(error) => {
            eprintln!("ошибка: {}", error.message());
            eprintln!();
            eprintln!("{}", cli::help(HelpTopic::General));
            2
        }
    };

    std::process::exit(exit_code);
}
