extern crate dirs;
use std::convert::AsRef;

use super::model;
use super::config_yaml;

#[derive(Clone, Copy)]
enum FileFormat {
    JSON,
    YAML,
    UNKNOWN,
}

// Search for configuration in the following locations:
//  .
//  ./garden
//  ./etc/garden
//  ~/.config/garden
//  ~/etc/garden
//  /etc/garden

fn search_path() -> Vec<std::path::PathBuf> {
    // Result: Vec<PathBufs> in priority order
    let mut paths: Vec<std::path::PathBuf> = Vec::new();

    let current_dir = std::env::current_dir().unwrap();
    let home_dir = dirs::home_dir().unwrap();

    // . Current directory
    paths.push(current_dir.to_path_buf());

    // ./garden
    let mut current_garden_dir  = current_dir.to_path_buf();
    current_garden_dir.push("garden");
    if current_garden_dir.exists() {
        paths.push(current_garden_dir);
    }

    // ./etc/garden
    let mut current_etc_garden_dir = current_dir.to_path_buf();
    current_etc_garden_dir.push("etc");
    current_etc_garden_dir.push("garden");
    if current_etc_garden_dir.exists() {
        paths.push(current_etc_garden_dir);
    }

    // ~/.config/garden
    let mut home_config_dir = home_dir.to_path_buf();
    home_config_dir.push(".config");
    home_config_dir.push("garden");
    if home_config_dir.exists() {
        paths.push(home_config_dir);
    }

    // ~/etc/garden
    let mut home_etc_dir = home_dir.to_path_buf();
    home_etc_dir.push("etc");
    home_etc_dir.push("garden");
    if home_etc_dir.exists() {
        paths.push(home_etc_dir);
    }

    // /etc/garden
    let etc_garden = std::path::PathBuf::from("/etc/garden");
    if etc_garden.exists() {
        paths.push(etc_garden);
    }

    return paths;
}


pub fn new(config: Option<std::path::PathBuf>,
           verbose: bool) -> model::Configuration
{
    let mut file_format = FileFormat::UNKNOWN;
    let mut path: Option<std::path::PathBuf> = None;
    let shell = std::path::PathBuf::from("/bin/sh");
    let variables = Vec::new();
    let environment = Vec::new();
    let commands = Vec::new();
    let gardens = Vec::new();
    let groups = Vec::new();
    let tree_search_path = Vec::new();
    let trees = Vec::new();
    let root_path = std::path::PathBuf::new();

    // Find garden.yaml in the search path
    let mut found = false;
    if let Some(config_path) = config {
        if config_path.is_file() && config_path.extension().is_some() {
            let ref ext = config_path.extension().unwrap()
                .to_string_lossy().to_lowercase();
            match ext.as_ref() {
                "json" => {
                    path = Some(config_path);
                    file_format = FileFormat::JSON;
                    found = true;
                }
                "yaml" => {
                    path = Some(config_path);
                    file_format = FileFormat::YAML;
                    found = true;
                }
                _ => { error!("unrecognized config file format: {}", ext); }
            }
        }
    }

    if !found {
        for entry in search_path() {
            let formats = vec!(
                (FileFormat::JSON, "json"),
                (FileFormat::YAML, "yaml"),
            );
            for fmt in formats {
                let (fmt_format, fmt_ext) = fmt;
                let mut candidate = entry.to_path_buf();
                candidate.push(String::from("garden.") + &fmt_ext);
                if candidate.exists() {
                    file_format = fmt_format;
                    path = Some(candidate);
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
    }

    if verbose {
        debug!("config path is {}{}",
               path.as_ref().unwrap().to_str().unwrap(),
               match found {
                   true => "",
                   false => " (NOT FOUND)",
               });
    }

    let mut cfg = model::Configuration{
        path: path,
        shell: shell,
        variables: variables,
        environment: environment,
        commands: commands,
        gardens: gardens,
        trees: trees,
        groups: groups,
        root_path: root_path,
        tree_search_path: tree_search_path,
        verbose: verbose,
    };

    if found {
        // parse yaml
        match file_format {
            FileFormat::YAML => {
                if verbose {
                    debug!("file format: yaml");
                }
                config_yaml::read(&mut cfg, verbose);
            }
            FileFormat::JSON => {
                if verbose {
                    debug!("file format: json");
                }
                error!("json support is currently unimplemented");
            }
            _ => {
                error!("unsupported config file format");
            }
        }
    }

    // Execute commands for each tree
    if verbose {
        debug!("configuration:\n{}", cfg);
    }
    return cfg;
}