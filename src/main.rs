extern crate argparse;
extern crate subprocess;

#[macro_use]
extern crate garden;

#[allow(non_camel_case_types)]
#[derive(Debug)]
enum Command {
    add,
    exec,
    help,
    init,
    shell,
    status,
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::str::FromStr for Command {
    type Err = ();
    fn from_str(src: &str) -> Result<Command, ()> {
        return match src {
            "add" => Ok(Command::add),
            "ex" => Ok(Command::exec),
            "exec" => Ok(Command::exec),
            "help" => Ok(Command::help),
            "init" => Ok(Command::init),
            "sh" => Ok(Command::shell),
            "shell" => Ok(Command::shell),
            "st" => Ok(Command::status),
            "stat" => Ok(Command::status),
            "status" => Ok(Command::status),
            _ => Err(()),
        }
    }
}

fn garden_help(verbose: bool, mut args: Vec<String>) {
    let cmd_path = match std::env::current_exe() {
        Err(_) => std::path::PathBuf::from("garden"),
        Ok(path) => path,
    };
    let mut help_cmd = vec!(cmd_path);

    let mut command = String::new();
    {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("garden help - command documentation");
        ap.stop_on_first_argument(true);

        ap.refer(&mut command)
            .add_argument("command", argparse::Store,
                          "Command help to display");

        args.insert(0, String::from("garden help"));
        ap.parse(args, &mut std::io::stdout(), &mut std::io::stderr())
            .map_err(|c| std::process::exit(c))
            .ok();
    }

    // garden help foo -> garden foo --help
    if !command.is_empty() {
        help_cmd.push(std::path::PathBuf::from(command));
    }

    help_cmd.push(std::path::PathBuf::from("--help"));

    if verbose {
        debug!("help command");
        let mut i: i32 = 0;
        for arg in &help_cmd {
            debug!("help_cmd[{:02}] = {:?}", i, arg);
            i += 1;
        }
    }

    std::process::exit(garden::cmd::get_status(&help_cmd));
}

fn garden_exec(config_file: Option<std::path::PathBuf>,
               verbose: bool, mut args: Vec<String>) {
    args.insert(0, "garden exec".to_string());

    let mut name = String::new();
    let mut command: Vec<String> = Vec::new();

    // Parse arguments
    {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("garden exec - execute commands inside gardens");

        ap.refer(&mut name).required()
            .add_argument("name", argparse::Store, "Garden to enter");

        ap.refer(&mut command).required()
            .add_argument("command", argparse::List, "Command to run");

        ap.stop_on_first_argument(true);
        if let Err(err) = ap.parse(args,
                                   &mut std::io::stdout(),
                                   &mut std::io::stderr()) {
            std::process::exit(err);
        }
    }

    // Resolve garden and tree names into a set of trees
    let config = garden::config::new(config_file, verbose);

    // Execute commands for each tree
    if verbose {
        debug!("subcommand: exec");
        debug!("name: {}", name);
        debug!("exec arguments:");
        let mut i: i32 = 0;
        for arg in &command {
            debug!("\targs[{:02}] = {}", i, arg);
            i += 1;
        }
    }
}

fn main() {
    let mut verbose = false;
    let mut subcommand = Command::help;
    let mut config_file_str = String::new();
    let mut args = Vec::new();
    {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("garden - git tree organizer");

        ap.refer(&mut verbose)
            .add_option(&["-v", "--verbose"],
                        argparse::StoreTrue, "Be verbose");

        ap.refer(&mut config_file_str)
            .add_option(&["-c", "--config"], argparse::Store, "Be verbose");

        ap.refer(&mut subcommand).required()
            .add_argument("command", argparse::Store,
                "Command to run {add, exec, help, init, shell, status}");

        ap.refer(&mut args)
            .add_argument("arguments", argparse::List,
                "Arguments for sub-command");

        ap.stop_on_first_argument(true);
        ap.parse_args_or_exit();
    }

    // Process arguments
    let mut config_file: Option<std::path::PathBuf> = None;
    if config_file_str.len() > 1 {
        config_file = Some(std::path::PathBuf::from(config_file_str));
    }

    match subcommand {
        Command::add => garden_help(verbose, args),
        Command::help => garden_help(verbose, args),
        Command::exec => garden_exec(config_file, verbose, args),
        Command::init => garden_help(verbose, args),
        Command::status => garden_help(verbose, args),
        Command::shell => garden_help(verbose, args),
    }
}