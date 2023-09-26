use crate::{AppConfig, RadioStation};
use duct::cmd;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Container {
    pub id: String,
    pub name: String,
}

pub fn get_running_containers() -> anyhow::Result<Vec<Container>> {
    let output = cmd!("docker", "ps", "--format", "{{.ID}}\n{{.Names}}").read()?;
    Ok(output
        .lines()
        .collect::<Vec<&str>>()
        .chunks(2)
        .map(|v| Container {
            id: v[0].trim().to_string(),
            name: v[1].trim().to_string(),
        })
        .collect())
}

pub fn remove_container(name: &str) -> anyhow::Result<Vec<String>> {
    let mut messages = Vec::new();
    for command in ["stop", "rm"] {
        let output = cmd!("docker", command, name).stderr_to_stdout().read()?;
        messages.push(output);
    }
    Ok(messages)
}

pub fn restart_container(name: &str) -> anyhow::Result<String> {
    let output = cmd!("docker", "restart", name).stderr_to_stdout().read()?;
    Ok(output)
}

pub fn restart_all_containers() -> anyhow::Result<Vec<String>> {
    let output = cmd!("bash", "-c", "docker restart $(docker ps -a -q)")
        .stderr_to_stdout()
        .read()?;
    Ok(output.lines().map(String::from).collect())
}

pub fn create_new_container(station: &RadioStation, config: &AppConfig) -> anyhow::Result<()> {
    cmd!(
        "docker",
        "run",
        "--restart=always",
        "-d",
        "--init",
        "--name",
        station.name.replace(' ', "_"),
        config.image.to_string(),
        "--debug",
        "--station",
        station.url.to_string(),
        "--interval",
        config.interval.to_string(),
        "--endpoint",
        config.endpoint.to_string()
    )
    .run()?;
    Ok(())
}

pub fn pull_image(name: &str) -> anyhow::Result<()> {
    cmd!("docker", "pull", name).run()?;
    Ok(())
}
