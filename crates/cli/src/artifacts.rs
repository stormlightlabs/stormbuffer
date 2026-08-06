use std::collections::BTreeMap;
use std::error::Error;

use clap::Command;
use clap_complete::{Shell, generate};
use clap_mangen::Man;

pub type GeneratedArtifacts = (BTreeMap<String, Vec<u8>>, BTreeMap<String, Vec<u8>>);

pub fn render(root: Command) -> Result<GeneratedArtifacts, Box<dyn Error>> {
    let mut man_pages = BTreeMap::new();
    let root_command = root.clone();
    let mut output = Vec::new();
    Man::new(root_command.clone()).render(&mut output)?;
    man_pages.insert(String::from("stormbuffer.1"), output);

    let subcommands: Vec<_> = root_command
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .cloned()
        .collect();
    for subcommand in subcommands {
        let name = subcommand.get_name().to_owned();
        let mut output = Vec::new();
        Man::new(subcommand.name(format!("stormbuffer-{name}"))).render(&mut output)?;
        man_pages.insert(format!("stormbuffer-{name}.1"), output);
    }

    let mut completions = BTreeMap::new();
    for (shell, extension) in [
        (Shell::Bash, "bash"),
        (Shell::Zsh, "zsh"),
        (Shell::Fish, "fish"),
        (Shell::PowerShell, "ps1"),
    ] {
        let mut output = Vec::new();
        let mut completion_command = root.clone();
        generate(shell, &mut completion_command, "stormbuffer", &mut output);
        completions.insert(format!("stormbuffer.{extension}"), output);
    }

    Ok((man_pages, completions))
}
