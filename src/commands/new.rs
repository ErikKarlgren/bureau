//! `bureau new dossier`.

use std::fs;
use std::io::{self, BufRead, Write};

use chrono::Local;

use crate::cli::DossierArgs;
use crate::dossier;
use crate::git;
use crate::template;
use crate::Result;

/// Create a dossier named after the arguments, then commit it.
///
/// # Errors
///
/// Fails when the current directory is not in a git repository, when a dossier
/// with the same name already exists, or when the dossier cannot be written.
pub fn dossier(args: &DossierArgs) -> Result<()> {
    let name = args.name.join(" ");
    let dossiers_dir = git::toplevel()?.join("dossiers");
    let path = dossiers_dir.join(dossier::name_to_filename(&name));

    if path.exists() {
        return Err(format!("a dossier already exists at path '{}'", path.display()).into());
    }

    let creation_date = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let description = prompt_for_description()?;
    let link = prompt_for_link()?;

    let contents = template::render(
        template::DOSSIER,
        &[
            ("DOSSIER_NAME", name.as_str()),
            ("CREATION_DATE", creation_date.as_str()),
            ("DESCRIPTION", description.as_str()),
            ("DOSSIER_LINK", link.as_str()),
        ],
    );

    fs::create_dir_all(&dossiers_dir)?;
    fs::write(&path, contents)?;

    // The dossier is on disk and useful either way, so a git failure is a
    // warning rather than an error.
    if let Err(error) = git::commit(&path, &format!("New dossier {name}")) {
        eprintln!("warning: {error}");
        eprintln!(
            "warning: the dossier was created at '{}' but is not committed",
            path.display()
        );
    }

    Ok(())
}

/// Read the description from stdin, ending at a line containing only a dot.
fn prompt_for_description() -> Result<String> {
    println!("Enter dossier's description. When done, write a line containing only '.'");

    let mut lines = Vec::new();
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line == "." {
            break;
        }
        lines.push(line);
    }

    Ok(lines.join("\n"))
}

/// Read the optional link to the task this dossier is about.
fn prompt_for_link() -> Result<String> {
    print!("Link to issue (e.g. Jira). If none, press Enter: ");
    io::stdout().flush()?;

    let mut link = String::new();
    if io::stdin().read_line(&mut link)? == 0 {
        // stdin ended, which means no link.
        return Ok(String::new());
    }

    let link = link.trim_end_matches(['\r', '\n']);
    if link.is_empty() {
        return Ok(String::new());
    }

    Ok(format!("- [Link to task]({link})\n"))
}
