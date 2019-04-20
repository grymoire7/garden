extern crate atty;
extern crate dirs;
extern crate glob;
extern crate yansi;

use ::eval;
use ::syntax;


/// Config files can define a sequence of variables that are
/// iteratively calculated.  Variables can reference other
/// variables in their Tree, Garden, and Configuration scopes.
///
/// The config values can contain either plain values,
/// string ${expressions} that resolve against other Variables,
/// or exec expressions that evaluate to a command whose stdout is
/// captured and placed into the value of the variable.
///
/// An exec expression can use shell-like ${variable} references as which
/// are substituted when evaluating the command, just like a regular
/// string expression.  An exec expression is denoted by using a "$ "
/// (dollar-sign followed by space) before the value.  For example,
/// using "$ echo foo" will place the value "foo" in the variable.

#[derive(Clone, Debug, Default)]
pub struct Variable {
    pub expr: String,
    pub value: Option<String>,
}

impl_display_brief!(Variable);

// Named variables with a single value
#[derive(Clone, Debug)]
pub struct NamedVariable  {
    pub name: String,
    pub expr: String,
    pub value: Option<String>,
}

impl_display_brief!(NamedVariable);

// Simple Name/Value pairs
#[derive(Clone, Debug)]
pub struct NamedValue {
    pub name: String,
    pub value: String,
}

impl_display_brief!(NamedValue);

// Named variables with multiple values
#[derive(Clone, Debug)]
pub struct MultiVariable {
    pub name: String,
    pub values: Vec<Variable>,
}

impl_display!(MultiVariable);

// Trees represent a single worktree
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub commands: Vec<MultiVariable>,
    pub environment: Vec<MultiVariable>,
    pub gitconfig: Vec<NamedVariable>,
    pub is_symlink: bool,
    pub name: String,
    pub path: Variable,
    pub remotes: Vec<NamedVariable>,
    pub symlink: Variable,
    pub templates: Vec<String>,
    pub variables: Vec<NamedVariable>,
}

impl_display!(Tree);

impl Tree {
    pub fn reset_variables(&mut self) {
        // self.path is a variable but it is not reset because
        // the tree path is evaluated once when the configuration
        // is first read, and never again.
        for var in &mut self.variables {
            var.value = None;
        }

        for var in &mut self.gitconfig {
            var.value = None;
        }

        for env in &mut self.environment {
            for var in &mut env.values {
                var.value = None;
            }
        }
        for cmd in &mut self.commands {
            for var in &mut cmd.values {
                var.value = None;
            }
        }
    }
}


#[derive(Clone, Debug, Default)]
pub struct Group {
    pub name: String,
    pub index: GroupIndex,
    pub members: Vec<String>,
}

impl_display!(Group);


#[derive(Clone, Debug, Default)]
pub struct Template {
    pub commands: Vec<MultiVariable>,
    pub environment: Vec<MultiVariable>,
    pub extend: Vec<String>,
    pub gitconfig: Vec<NamedVariable>,
    pub name: String,
    pub remotes: Vec<NamedVariable>,
    pub variables: Vec<NamedVariable>,
}

impl_display!(Template);


// Gardens aggregate trees
#[derive(Clone, Debug, Default)]
pub struct Garden {
    pub name: String,
    pub commands: Vec<MultiVariable>,
    pub environment: Vec<MultiVariable>,
    pub gitconfig: Vec<NamedVariable>,
    pub groups: Vec<String>,
    pub index: GardenIndex,
    pub trees: Vec<String>,
    pub variables: Vec<NamedVariable>,
}

impl_display!(Garden);


// Configuration represents an instantiated garden configuration
#[derive(Clone, Debug, Default)]
pub struct Configuration {
    pub commands: Vec<MultiVariable>,
    pub debug: std::collections::HashSet<String>,
    pub environment: Vec<MultiVariable>,
    pub gardens: Vec<Garden>,
    pub grafts: Vec<NamedVariable>,
    pub groups: Vec<Group>,
    pub path: Option<std::path::PathBuf>,
    pub dirname: Option<std::path::PathBuf>,
    pub root: Variable,
    pub root_path: std::path::PathBuf,
    pub shell: String,
    pub templates: Vec<Template>,
    pub tree_search_path: Vec<std::path::PathBuf>,
    pub trees: Vec<Tree>,
    pub variables: Vec<NamedVariable>,
    pub verbose: bool,
}

impl_display!(Configuration);

impl Configuration {

    /// Create a default Configuration
    pub fn new() -> Self {
        return Configuration {
            shell: "zsh".to_string(),
            ..std::default::Default::default()
        }
    }

    pub fn initialize(&mut self) {
        // Evaluate garden.root
        let expr = self.root.expr.to_string();
        let value = eval::value(self, &expr);
        // Store the resolved garden.root
        self.root_path = std::path::PathBuf::from(&value);
        self.root.value = Some(value.to_string());

        // Resolve tree paths
        self.update_tree_paths();

        // Assign garden.index to each garden
        self.update_indexes();

        // Reset variables
        self.reset();
    }

    pub fn reset(&mut self) {
        // Reset variables to allow for tree-scope evaluation
        self.reset_variables();

        // Add custom variables
        self.reset_builtin_variables()
    }

    fn reset_builtin_variables(&mut self) {
        // Update GARDEN_ROOT at position 0
        if !self.variables.is_empty() && self.variables[0].name == "GARDEN_ROOT" {
            let value = self.root.value.as_ref().unwrap().to_string();
            self.variables[0].expr = value.to_string();
            self.variables[0].value = Some(value.to_string());
        }

        for tree in self.trees.iter_mut() {
            let tree_path = tree.path.value.as_ref().unwrap().to_string();
            if tree.variables.len() >= 2 {
                // Update TREE_NAME at position 0
                if tree.variables[0].name == "TREE_NAME" {
                    tree.variables[0].expr = tree.name.to_string();
                    tree.variables[0].value = Some(tree.name.to_string());
                }
                // Update TREE_PATH at position 1
                if tree.variables[1].name == "TREE_PATH" {
                    tree.variables[1].expr = tree_path.to_string();
                    tree.variables[1].value = Some(tree_path.to_string());
                }
            }
        }
    }

    fn update_indexes(&mut self) {
        for (idx, group) in self.groups.iter_mut().enumerate() {
            group.index = idx as GroupIndex;
        }

        for (idx, garden) in self.gardens.iter_mut().enumerate() {
            garden.index = idx as GardenIndex;
        }
    }

    // Calculate the "path" field for each tree.
    // If specified as a relative path, it will be relative to garden.root.
    // If specified as an asbolute path, it will be left as-is.
    fn update_tree_paths(&mut self) {
        // Gather path and symlink expressions.
        let mut path_values = Vec::new();
        let mut symlink_values = Vec::new();
        for (idx, tree) in self.trees.iter().enumerate() {
            path_values.push(tree.path.expr.to_string());
            if tree.is_symlink {
                symlink_values.push((idx, tree.symlink.expr.to_string()));
            }
        }

        // Evaluate the "path" expression.
        for (idx, value) in path_values.iter().enumerate() {
            let result = Some(self.eval_abspath(value));
            self.trees[idx].path.value = result;
        }

        // Evaluate the "symlink" expression.
        for (idx, value) in &symlink_values {
            let result = Some(self.eval_abspath(value));
            self.trees[*idx].symlink.value = result;
        }
    }

    /// Return a path string relative to the garden root
    fn abspath(&self, path: &str) -> String {
        if path.starts_with("/") {
            // Absolute path, nothing to do
            path.to_string()
        } else {
            // Make path relative to root_path
            let mut path_buf = self.root_path.to_path_buf();
            path_buf.push(path);

            path_buf.to_string_lossy().to_string()
        }
    }

    /// Evaluate the string and return a path string relative to the garden root.
    fn eval_abspath(&mut self, path: &str) -> String {
        let value = eval::value(self, &path);

        self.abspath(&value)
    }

    /// Reset resolved variables
    pub fn reset_variables(&mut self) {
        for var in &mut self.variables {
            var.value = None;
        }
        for env in &mut self.environment {
            for var in &mut env.values {
                var.value = None;
            }
        }
        for cmd in &mut self.commands {
            for var in &mut cmd.values {
                var.value = None;
            }
        }
        for tree in &mut self.trees {
            tree.reset_variables();
        }
    }

    /// Set the config path and the dirname fields
    pub fn set_path(&mut self, path: std::path::PathBuf) {
        let mut dirname = path.to_path_buf();
        dirname.pop();

        self.dirname = Some(dirname);
        self.path = Some(path);
    }
}


/// Tree index into config.trees
pub type TreeIndex = usize;

/// Group index into config.groups
pub type GroupIndex = usize;

/// Garden index into config.gardens
pub type GardenIndex = usize;


#[derive(Clone, Debug)]
pub struct TreeContext {
    pub tree: TreeIndex,
    pub garden: Option<GardenIndex>,
    pub group: Option<GroupIndex>,
}

impl_display_brief!(TreeContext);


#[derive(Debug, Default)]
pub struct TreeQuery {
    pub query: String,
    pub pattern: glob::Pattern,
    pub is_default: bool,
    pub is_garden: bool,
    pub is_group: bool,
    pub is_tree: bool,
    pub include_gardens: bool,
    pub include_groups: bool,
    pub include_trees: bool,
}

impl_display_brief!(TreeQuery);

impl TreeQuery {

    pub fn new(query: &str) -> Self {
        let mut glob_pattern = query.to_string();
        let mut is_default = false;
        let mut is_tree = false;
        let mut is_garden = false;
        let mut is_group = false;
        let mut include_gardens = true;
        let mut include_groups = true;
        let mut include_trees = true;
        let mut trim = false;

        if syntax::is_garden(&glob_pattern) {
            is_garden = true;
            include_groups = false;
            include_trees = false;
            trim = true;
        } else if syntax::is_group(&glob_pattern) {
            is_group = true;
            include_gardens = false;
            include_trees = false;
            trim = true;
        } else if syntax::is_tree(&glob_pattern) {
            is_tree = true;
            include_gardens = false;
            include_groups = false;
            trim = true;
        } else {
            is_default = true;
        }
        if trim {
            glob_pattern = syntax::trim(&glob_pattern);
        }

        let pattern = glob::Pattern::new(glob_pattern.as_ref()).unwrap();

        return TreeQuery {
            query: query.to_string(),
            is_default: is_default,
            is_garden: is_garden,
            is_group: is_group,
            is_tree: is_tree,
            include_gardens: include_gardens,
            include_groups: include_groups,
            include_trees: include_trees,
            pattern: pattern,
        }
    }
}


// Commands
#[derive(Clone, Debug)]
pub enum Command {
    Add,
    Cmd,
    Custom(String),
    Exec,
    Eval,
    Grow,
    Help,
    Init,
    Inspect,
    List,
    Shell,
}

impl std::default::Default for Command {
    fn default() -> Self { Command::Help }
}

impl_display_brief!(Command);

impl std::str::FromStr for Command {
    type Err = ();  // For the FromStr trait

    fn from_str(src: &str) -> Result<Command, ()> {
        return match src {
            "add" => Ok(Command::Add),
            "cmd" => Ok(Command::Cmd),
            "exec" => Ok(Command::Exec),
            "eval" => Ok(Command::Eval),
            "grow" => Ok(Command::Grow),
            "help" => Ok(Command::Help),
            "init" => Ok(Command::Init),
            "inspect" => Ok(Command::Inspect),
            "list" => Ok(Command::List),
            "ls" => Ok(Command::List),
            "sh" => Ok(Command::Shell),
            "shell" => Ok(Command::Shell),
            _ => Ok(Command::Custom(src.to_string())),
        }
    }
}


// Is color enabled?
// --color=<auto,on,off> overrides the default "auto" value.

#[derive(Clone, Debug, PartialEq)]
pub enum ColorMode {
    Auto,  // "auto" enables color when a tty is detected.
    Off,  // disable color
    On,  // enable color
}

impl ColorMode {
    pub fn is_enabled(&self) -> bool {
        match self {
            ColorMode::Auto => atty::is(atty::Stream::Stdout),
            ColorMode::Off => false,
            ColorMode::On => true,
        }
    }

    pub fn names() -> &'static str {
        "auto, true, false, 1, 0, [y]es, [n]o, on, off, always, never"
    }

    pub fn update(&mut self) {
        if *self == ColorMode::Auto {
            // Speedup future calls to is_enabled() by performing the "auto"
            // atty check once and caching the result.
            if self.is_enabled() {
                *self = ColorMode::On;
            } else {
                *self = ColorMode::Off;
            }
        }

        if *self == ColorMode::Off {
            yansi::Paint::disable();
        }
    }
}

impl std::default::Default for ColorMode {
    fn default() -> Self { ColorMode::Auto }
}

impl std::str::FromStr for ColorMode {
    type Err = ();  // For the FromStr trait

    fn from_str(src: &str) -> Result<ColorMode, ()> {
        return match src.to_lowercase().as_ref() {
            "auto" => Ok(ColorMode::Auto),
            "-1" => Ok(ColorMode::Auto),
            "0" => Ok(ColorMode::Off),
            "1" => Ok(ColorMode::On),
            "false" => Ok(ColorMode::Off),
            "true" => Ok(ColorMode::On),
            "never" => Ok(ColorMode::Off),
            "always" => Ok(ColorMode::Off),
            "off" => Ok(ColorMode::Off),
            "on" => Ok(ColorMode::On),
            "n" => Ok(ColorMode::Off),
            "y" => Ok(ColorMode::On),
            "no" => Ok(ColorMode::Off),
            "yes" => Ok(ColorMode::On),
            _ => Err(()),
        }
    }
}


// Color is an alias for yansi::Paint.
pub type Color<T> = yansi::Paint<T>;


pub fn display_missing_tree(tree: &Tree, path: &str, verbose: bool) -> String {
    if verbose {
        format!("{} {}  {} {}",
                Color::black("#").bold(),
                Color::black(&tree.name).bold(),
                Color::black(&path).bold(),
                Color::black("(skipped)").bold())
    } else {
        format!("{} {} {}",
                Color::black("#").bold(),
                Color::black(&tree.name).bold(),
                Color::black("(skipped)").bold())
    }
}


pub fn display_tree(tree: &Tree, path: &str, verbose: bool) -> String {
    if verbose {
        format!("{} {}  {}",
                Color::cyan("#"),
                Color::blue(&tree.name).bold(),
                Color::blue(&path))
    } else {
        format!("{} {}",
                Color::cyan("#"),
                Color::blue(&tree.name).bold())
    }
}


/// Print a tree if it exists, otherwise print a missing tree
pub fn print_tree(tree: &Tree, verbose: bool, quiet: bool) -> bool {
    // Sparse gardens/missing trees are ok -> skip these entries.
    let path = tree.path.value.as_ref().unwrap().to_string();
    if !std::path::PathBuf::from(&path).exists() {
        if !quiet {
            eprintln!("{}", display_missing_tree(&tree, &path, verbose));
        }
        return false;
    }
    print_tree_details(tree, verbose, quiet);

    true
}


/// Print a tree
pub fn print_tree_details(tree: &Tree, verbose: bool, quiet: bool) {
    if !quiet {
        let path = tree.path.value.as_ref().unwrap().to_string();
        eprintln!("{}", display_tree(&tree, &path, verbose));
    }
}


#[derive(Clone, Debug, Default)]
pub struct CommandOptions {
    pub args: Vec<String>,
    pub debug: Vec<String>,
    pub chdir: String,
    pub color_mode: ColorMode,
    pub filename: Option<std::path::PathBuf>,
    pub filename_str: String,
    pub keep_going: bool,
    pub quiet: bool,
    pub root: String,
    pub subcommand: Command,
    pub variables: Vec<String>,
    pub verbose: bool,
}

impl CommandOptions {
    pub fn update(&mut self) {
        // Allow specifying the config file: garden --config <path>
        if !self.filename_str.is_empty() {
            let path = std::path::PathBuf::from(&self.filename_str);
            if path.exists() {
                let canon = path.canonicalize().unwrap_or(path);
                self.filename = Some(canon);
            } else {
                self.filename = Some(path);
            }
        }

        // Override garden.root: garden --root <path>
        if !self.root.is_empty() {
            // Resolve the "--root" option to an absolute path
            let root_path = std::path::PathBuf::from(&self.root);
            self.root = root_path.canonicalize()
                .unwrap_or(root_path).to_string_lossy().to_string();
        }

        // Change directories before searching for conifgs: garden --chdir <path>
        if !self.chdir.is_empty() {
            if let Err(err) = std::env::set_current_dir(&self.chdir) {
                error!("could not chdir to '{}': {}", self.chdir, err);
            }
        }

        self.color_mode.update();
    }

    pub fn is_debug(&self, name: &str) -> bool {
        return self.debug.contains(&name.to_string());
    }
}


#[derive(Clone, Debug, Default)]
pub struct ApplicationContext {
    pub config: Configuration,
    pub options: CommandOptions,
}

impl_display!(ApplicationContext);

impl ApplicationContext {
    pub fn new(config: Configuration, options: CommandOptions) -> Self {
        ApplicationContext {
            config: config,
            options: options,
        }
    }
}
